"""The stadium page's 3D ground (user 2026-11-23: from the flat drawing to a rendered 3/4 view).

Run with Blender 5.x (no window needed):
    blender -b --factory-startup -P build_stadium.py -- <out folder>
It builds the ground in four sizes (s, m, l, x: the page picks one by capacity), renders each from
the same 3/4 angle and writes the pack the DLL reads, <out>/cmui_stadium.bin (copied to Data\\ by
install.py). Change the sizes below and run it again.

Per size four pictures:
  base   the whole ground without roof or works (Eevee)
  id     the same in flat colours (Workbench, no smoothing): what each pixel is, for the DLL
         R class (0 as drawn, 1 seat colour A, 2 seat colour B, 3 club colour trim),
         G the stand (0 north, 1 south, 2 west, 3 east), B the segment along it (0..15)
  roof   the roofs alone, the rest held out (drawn over the base when the ground has a roof)
  works  crane and scaffolding alone (drawn over while places are being built)
The seats are rendered white: the DLL paints them in the club's two colours (a mosaic) or grey
where the ground has standing places, by its own figures.
"""
import bpy, bmesh, math, random, sys, os, struct, json
import numpy as np
from mathutils import Vector, Matrix

OUT = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else os.path.join(os.path.dirname(os.path.abspath(__file__)), "out")
W, H = 780, 1080  # portrait: the page's left column (the mock's picture)
# tiers per size: (rows, row depth m, row rise m)
SIZES = {
    "s": [(12, 0.85, 0.42)],
    "m": [(22, 0.85, 0.45)],
    "l": [(20, 0.85, 0.45), (14, 0.9, 0.62)],
    "x": [(26, 0.85, 0.45), (22, 0.9, 0.66)],
}
PL, PW = 105.0, 68.0  # the pitch
GAP = 7.0  # touchline to the first row
ROOF_FRONT = 0.58  # the roofs reach this far from the front (share of the stand's depth): the reference's seats show
SEGS, BANDS = 16, 3
VIEW = Vector((-1.0, -0.55, 1.25)).normalized()  # camera looks along -VIEW: from the west end, high
SUN = Vector((0.35, -0.75, 0.7)).normalized()


def srgb_lin(v):
    v = v / 255.0
    return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4


# ---------------------------------------------------------------- materials
MATS = {}


def mat(name, color, rough=0.7, metal=0.0, emit=0.0):
    if name in MATS:
        return MATS[name]
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    p = m.node_tree.nodes.get("Principled BSDF")
    p.inputs["Base Color"].default_value = (*color, 1)
    p.inputs["Roughness"].default_value = rough
    p.inputs["Metallic"].default_value = metal
    if emit:
        p.inputs["Emission Color"].default_value = (*color, 1)
        p.inputs["Emission Strength"].default_value = emit
    elif name not in ("Seat", "Trim", "Lines", "Paint"):
        # a slight variation of colour and roughness, so it does not look plastic (the user's
        # brief); the seats and the club-colour trim stay even (the DLL paints them by their light)
        nt = m.node_tree
        tc = nt.nodes.new("ShaderNodeTexCoord")
        nz = nt.nodes.new("ShaderNodeTexNoise")
        nz.inputs["Scale"].default_value = 0.45
        nz.inputs["Detail"].default_value = 6.0
        nt.links.new(tc.outputs["Object"], nz.inputs["Vector"])
        k = nt.nodes.new("ShaderNodeMath"); k.operation = "MULTIPLY"; k.inputs[1].default_value = 0.6
        nt.links.new(nz.outputs["Fac"], k.inputs[0])
        mix = nt.nodes.new("ShaderNodeMix"); mix.data_type = "RGBA"
        # (a faint variation: the reference's materials are clean, 2026-11-23)
        mix.inputs[6].default_value = (*[c * 1.05 for c in color], 1)
        mix.inputs[7].default_value = (*[c * 0.93 for c in color], 1)
        nt.links.new(k.outputs[0], mix.inputs[0])
        nt.links.new(mix.outputs[2], p.inputs["Base Color"])
        r = nt.nodes.new("ShaderNodeMapRange")
        r.inputs["To Min"].default_value = max(rough - 0.15, 0.0)
        r.inputs["To Max"].default_value = min(rough + 0.15, 1.0)
        nt.links.new(nz.outputs["Fac"], r.inputs["Value"])
        nt.links.new(r.outputs["Result"], p.inputs["Roughness"])
    MATS[name] = m
    return m


def grass_mat():
    m = bpy.data.materials.new("Grass")
    m.use_nodes = True
    nt = m.node_tree
    p = nt.nodes.get("Principled BSDF")
    p.inputs["Roughness"].default_value = 0.9
    tc = nt.nodes.new("ShaderNodeTexCoord")
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    l = nt.links
    l.new(tc.outputs["Object"], sep.inputs[0])
    # mowing stripes across the pitch (strong) and along it (faint): the reference's lawn
    mods = []
    for axis, size in ((0, PL / 14), (1, PW / 6)):
        div = nt.nodes.new("ShaderNodeMath"); div.operation = "DIVIDE"; div.inputs[1].default_value = size
        fl = nt.nodes.new("ShaderNodeMath"); fl.operation = "FLOOR"
        mod = nt.nodes.new("ShaderNodeMath"); mod.operation = "FLOORED_MODULO"; mod.inputs[1].default_value = 2
        l.new(sep.outputs[axis], div.inputs[0])
        l.new(div.outputs[0], fl.inputs[0])
        l.new(fl.outputs[0], mod.inputs[0])
        mods.append(mod)
    k = nt.nodes.new("ShaderNodeMath"); k.operation = "MULTIPLY_ADD"
    k.inputs[1].default_value = 0.28
    l.new(mods[1].outputs[0], k.inputs[0])
    l.new(mods[0].outputs[0], k.inputs[2])
    mix = nt.nodes.new("ShaderNodeMix"); mix.data_type = "RGBA"
    mix.inputs[6].default_value = (0.055, 0.25, 0.03, 1)
    mix.inputs[7].default_value = (0.1, 0.38, 0.05, 1)
    l.new(k.outputs[0], mix.inputs[0])
    l.new(mix.outputs[2], p.inputs["Base Color"])
    return m


def strut(bm, p0, p1, t):
    """A square bar of side t from p0 to p1 (a lattice member)."""
    p0, p1 = Vector(p0), Vector(p1)
    d = p1 - p0
    m = Matrix.Translation((p0 + p1) / 2) @ d.to_track_quat("Z", "Y").to_matrix().to_4x4() @ Matrix.Diagonal((t, t, d.length, 1.0))
    bmesh.ops.create_cube(bm, size=1.0, matrix=m)


# ---------------------------------------------------------------- geometry
def add_box(bm, a, b):
    """An axis-aligned box between corners a and b (world)."""
    x0, x1 = sorted((a[0], b[0])); y0, y1 = sorted((a[1], b[1])); z0, z1 = sorted((a[2], b[2]))
    v = [bm.verts.new(p) for p in [(x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0), (x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)]]
    for f in [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]:
        bm.faces.new([v[i] for i in f])


def obj(name, bm, material, coll, color=(0, 0, 0, 1)):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    o.data.materials.append(material)
    o.color = color
    coll.objects.link(o)
    return o


def id_colour(cls, stand=0, seg=0):
    return (srgb_lin(cls * 80), srgb_lin(stand * 80), srgb_lin(seg * 16), 1)


# a stand's own frame: a along it, u outward from its front, z up
def frame(side):
    f = (PW / 2 if side < 2 else PL / 2) + GAP
    length = PL + 8 if side < 2 else PW + 4
    def w(a, u, z):
        return {0: (a, f + u, z), 1: (a, -(f + u), z), 2: (-(f + u), a, z), 3: (f + u, a, z)}[side]
    return w, length


def profile(tiers):
    """Each row: (u front, u back, z of its tread, tier)."""
    rows, u, z = [], 0.0, 0.9
    for t, (n, d, rise) in enumerate(tiers):
        if t > 0:  # the upper tier hangs over the lower's back third
            u -= 0.3 * sum(r[1] - r[0] for r in rows if r[3] == t - 1)
            z += 4.0
        for _ in range(n):
            z += rise
            rows.append((u, u + d, z, t))
            u += d
    return rows


def build(size, colls):
    tiers = SIZES[size]
    rows = profile(tiers)
    depth = max(r[1] for r in rows)
    top = max(r[2] for r in rows)
    base, roof_c, works = colls[:3]
    seat = mat("Seat", (0.82, 0.82, 0.82), 0.45)
    concrete = mat("Concrete", (0.3, 0.305, 0.31), 0.85)
    facade = mat("Facade", (0.055, 0.058, 0.064), 0.6)
    trim = mat("Trim", (0.85, 0.85, 0.85), 0.4)
    metal = mat("Roof", (0.13, 0.135, 0.145), 0.4, 0.4)
    dark = mat("Fascia", (0.07, 0.075, 0.085), 0.5)
    tunnel = mat("Tunnel", (0.008, 0.008, 0.01), 0.9)
    for side in range(4):
        w, length = frame(side)
        n_rows = len(rows)
        # aisles: a concrete stair between the seat blocks, as real stands (and the mock) have
        aisles = bmesh.new()
        aisle = 1.1
        for seg in range(SEGS):
            a0 = -length / 2 + seg * length / SEGS
            a1 = a0 + length / SEGS - (aisle if seg < SEGS - 1 else 0.0)
            if seg < SEGS - 1:
                for u0, u1, z, t in rows:
                    add_box(aisles, w(a1, u0, 0.0 if t == 0 else z - 2.5), w(a1 + aisle, u1, z - 0.05))
            for band in range(BANDS):
                bm = bmesh.new()
                for k in range(band * n_rows // BANDS, (band + 1) * n_rows // BANDS):
                    u0, u1, z, t = rows[k]
                    floor = 0.0 if t == 0 else z - 2.5
                    add_box(bm, w(a0, u0, floor), w(a1, u1, z))
                    add_box(bm, w(a0 + 0.05, u1 - 0.32, z), w(a1 - 0.05, u1 - 0.12, z + 0.42))  # the seat backs
                cls = 1 + (seg // 2 + band) % 2  # (blocks two segments wide: the mock's mosaic)
                obj(f"seats{side}_{seg}_{band}", bm, seat, base, id_colour(cls, side, seg))
        obj(f"aisles{side}", aisles, concrete, base)
        # vomitories: a tunnel mouth in the lower tier at every other aisle (the reference's main stand)
        lower = [r for r in rows if r[3] == 0]
        k0, k1 = len(lower) * 35 // 100, len(lower) * 55 // 100
        walls, mouths = bmesh.new(), bmesh.new()
        for seg in range(1, SEGS - 1, 2):
            a = -length / 2 + seg * length / SEGS - aisle / 2
            u0, u1, z = lower[k0][0], lower[k1][1], lower[k1][2] + 0.5
            add_box(walls, w(a - 1.6, u0, 0.0), w(a + 1.6, u1, z))
            add_box(mouths, w(a - 1.25, u0 - 0.02, lower[k0][2] - 0.2), w(a + 1.25, u1 - 0.3, z + 0.02))
        obj(f"vomwalls{side}", walls, concrete, base)
        obj(f"vom{side}", mouths, tunnel, base)
        # the front wall between the pitch surround and the first row
        bm = bmesh.new()
        add_box(bm, w(-length / 2, -0.35, 0), w(length / 2, 0.0, rows[0][2] + 0.3))
        obj(f"frontwall{side}", bm, concrete, base)
        # the back: facade from the ground to above the last row; pitch-side boards in club colour
        bm = bmesh.new()
        add_box(bm, w(-length / 2, depth, 0), w(length / 2, depth + 1.4, top + 2.2))
        obj(f"back{side}", bm, facade, base)
        bm = bmesh.new()
        add_box(bm, w(-length / 2, depth - 0.2, top + 2.2), w(length / 2, depth + 1.6, top + 2.9))
        obj(f"backtrim{side}", bm, trim, base, id_colour(3))
        bm = bmesh.new()
        for a in (-length / 2 - 0.8, length / 2):  # the end walls: a step for every row
            for u0, u1, z, tier in rows:
                add_box(bm, w(a, u0, 0.0 if tier == 0 else z - 2.7), w(a + 0.8, u1, z + 0.55))
            add_box(bm, w(a, depth, 0), w(a + 0.8, depth + 1.4, top + 2.2))
        obj(f"ends{side}", bm, facade, base)
        # the back: ribs every 6 m and a band of glass (the concourse)
        bm, gl = bmesh.new(), bmesh.new()
        for k in range(int(length // 6) + 1):
            a = -length / 2 + k * 6
            add_box(bm, w(a, depth + 1.4, 0), w(a + 0.5, depth + 1.75, top + 2.2))
        add_box(gl, w(-length / 2, depth + 1.4, 3.0), w(length / 2, depth + 1.5, 5.2))
        if top > 14:
            add_box(gl, w(-length / 2, depth + 1.4, top * 0.55), w(length / 2, depth + 1.5, top * 0.55 + 2.2))
        obj(f"ribs{side}", bm, mat("Ribs", (0.1, 0.104, 0.112), 0.6), base)
        # the entrance in the middle of the back: a dark doorway, a canopy in club colour,
        # turnstiles and a stair flight each side (the reference's main stand)
        door, canopy, gates = bmesh.new(), bmesh.new(), bmesh.new()
        bk = depth + 1.75
        add_box(door, w(-6, bk - 0.4, 0), w(6, bk + 0.05, 3.2))
        add_box(canopy, w(-7, bk, 3.3), w(7, bk + 1.6, 3.9))
        for k in range(5):
            add_box(gates, w(-4 + k * 2, bk + 0.6, 0), w(-3.6 + k * 2, bk + 1.4, 1.1))
        for s in (-1, 1):
            for k in range(6):
                a0, a1 = sorted((s * 8, s * 11))
                add_box(gates, w(a0, bk, 0), w(a1, bk + 4.2 - k * 0.7, 0.45 * (k + 1)))
        obj(f"door{side}", door, tunnel, base)
        obj(f"canopy{side}", canopy, trim, base, id_colour(3))
        obj(f"gates{side}", gates, concrete, base)
        # (lit from inside: the mock's evening, a warm band along each stand's back)
        obj(f"glass{side}", gl, mat("Concourse", (0.03, 0.035, 0.045), 0.1, 0.3), base)
        bm = bmesh.new()
        add_box(bm, w(-length / 2 + 4, -GAP + 2.0, 0), w(length / 2 - 4, -GAP + 2.3, 0.95))
        obj(f"boards{side}", bm, trim, base, id_colour(3))
        # the roof: from behind the facade over two thirds of the rows, rising toward the pitch
        bm, ribs = bmesh.new(), bmesh.new()
        front = depth * ROOF_FRONT
        for k in range(8):
            u0 = depth + 1.6 - k * (depth + 1.6 - front) / 8
            u1 = depth + 1.6 - (k + 1) * (depth + 1.6 - front) / 8
            z = top + 5.0 + k * 0.35
            add_box(bm, w(-length / 2 - 1, u1, z), w(length / 2 + 1, u0, z + 0.5))
            # corrugation: a low rib every 2.5 m down the slope
            a = -length / 2 - 0.5
            while a < length / 2 + 0.5:
                add_box(ribs, w(a, u1, z + 0.5), w(a + 0.3, u0, z + 0.68))
                a += 2.5
        obj(f"roof{side}", bm, metal, roof_c)
        # (the ribs a lighter steel: the reference's roofs read as ribbed sheet)
        obj(f"roofribs{side}", ribs, mat("RoofRibs", (0.3, 0.31, 0.33), 0.35, 0.5), roof_c)
        bm, edge = bmesh.new(), bmesh.new()
        add_box(edge, w(-length / 2 - 1, front - 0.4, top + 5.0 + 8 * 0.35 - 0.6), w(length / 2 + 1, front, top + 5.0 + 8 * 0.35 + 0.9))
        for k in range(9):  # columns at the back
            a = -length / 2 + k * length / 8
            add_box(bm, w(a - 0.3, depth + 1.4, top + 2.9), w(a + 0.3, depth + 1.9, top + 5.5))
        # the truss under the roof's front edge: two chords and a zigzag (the reference's girders)
        zf = top + 5.0 + 8 * 0.35 - 0.6
        a, step = -length / 2 - 1, 3.0
        strut(bm, w(-length / 2 - 1, front - 0.2, zf - 2.6), w(length / 2 + 1, front - 0.2, zf - 2.6), 0.28)
        up = True
        while a < length / 2 + 1 - step / 2:
            z0, z1 = (zf - 2.6, zf) if up else (zf, zf - 2.6)
            strut(bm, w(a, front - 0.2, z0), w(a + step, front - 0.2, z1), 0.2)
            a += step
            up = not up
        obj(f"fascia{side}", bm, dark, roof_c)
        obj(f"roofedge{side}", edge, mat("RoofEdge", (0.55, 0.57, 0.6), 0.4, 0.3), roof_c)
        # works: scaffolding on the back, a grid of poles
        bm = bmesh.new()
        for k in range(int(length // 6) + 1):
            a = -length / 2 + k * 6
            add_box(bm, w(a, depth + 1.6, 0), w(a + 0.35, depth + 1.95, top + 4))
        z = 0.0
        while z < top + 4:
            add_box(bm, w(-length / 2, depth + 1.6, z), w(length / 2, depth + 1.95, z + 0.3))
            z += 3.0
        obj(f"scaffold{side}", bm, mat("Scaffold", (0.9, 0.45, 0.05), 0.5), works)
    # the crane at the north-east corner
    cx, cy = PL / 2 + GAP + depth * 0.6, PW / 2 + GAP + depth + 8
    tall = top + 34
    bm = bmesh.new()
    add_box(bm, (cx - 1.2, cy - 1.2, 0), (cx + 1.2, cy + 1.2, tall))
    add_box(bm, (cx - 45, cy - 0.8, tall), (cx + 14, cy + 0.8, tall + 1.8))
    add_box(bm, (cx + 6, cy - 1.6, tall - 1.5), (cx + 13, cy + 1.6, tall + 0.5))
    add_box(bm, (cx - 0.25, cy - 0.25, tall - 18), (cx + 0.25, cy + 0.25, tall))
    obj("crane", bm, mat("Crane", (0.95, 0.72, 0.05), 0.45), works)
    bm = bmesh.new()
    add_box(bm, (cx - 30, cy - 0.12, tall - 20), (cx - 29.7, cy + 0.12, tall))
    add_box(bm, (cx - 31.5, cy - 1.5, tall - 21.5), (cx - 28.2, cy + 1.5, tall - 20))
    obj("hook", bm, mat("Hook", (0.1, 0.1, 0.1)), works)
    return depth, top


def pitch(coll):
    bm = bmesh.new()
    add_box(bm, (-PL / 2 - 3.5, -PW / 2 - 3.5, -0.1), (PL / 2 + 3.5, PW / 2 + 3.5, 0.02))
    obj("pitch", bm, grass_mat(), coll)
    bm = bmesh.new()  # the dark surround between the grass and the stands
    add_box(bm, (-PL / 2 - GAP - 0.5, -PW / 2 - GAP - 0.5, -0.15), (PL / 2 + GAP + 0.5, PW / 2 + GAP + 0.5, -0.01))
    obj("surround", bm, mat("Surround", (0.045, 0.048, 0.052), 0.8), coll)
    white = mat("Lines", (0.95, 0.95, 0.95), 0.6)
    bm = bmesh.new()
    t, z = 0.25, 0.05
    def line(x0, y0, x1, y1):
        add_box(bm, (min(x0, x1) - t / 2, min(y0, y1) - t / 2, 0.02), (max(x0, x1) + t / 2, max(y0, y1) + t / 2, z))
    hl, hw = PL / 2, PW / 2
    for y in (-hw, hw):
        line(-hl, y, hl, y)
    for x in (-hl, 0, hl):
        line(x, -hw, x, hw)
    for s in (-1, 1):
        for depth, half in ((16.5, 20.16), (5.5, 9.16)):
            x = s * (hl - depth)
            line(s * hl, -half, x, -half); line(s * hl, half, x, half); line(x, -half, x, half)
    n = 96
    for k in range(n):
        a0, a1 = 2 * math.pi * k / n, 2 * math.pi * (k + 1) / n
        line(9.15 * math.cos(a0), 9.15 * math.sin(a0), 9.15 * math.cos(a1), 9.15 * math.sin(a1))
    # the penalty arcs outside the boxes, the spots, the corner arcs
    for s in (-1, 1):
        spot = s * (hl - 11.0)
        add_box(bm, (spot - 0.2, -0.2, 0.02), (spot + 0.2, 0.2, z))
        add_box(bm, (-0.2, -0.2, 0.02), (0.2, 0.2, z))
        for k in range(48):
            a0, a1 = math.pi * k / 48 - math.pi / 2, math.pi * (k + 1) / 48 - math.pi / 2
            x0, x1 = spot - s * 9.15 * math.cos(a0), spot - s * 9.15 * math.cos(a1)
            if abs(x0) < hl - 16.5 and abs(x1) < hl - 16.5:
                line(x0, 9.15 * math.sin(a0), x1, 9.15 * math.sin(a1))
        for cy in (-hw, hw):
            for k in range(8):
                a0, a1 = math.pi / 2 * k / 8, math.pi / 2 * (k + 1) / 8
                sx, sy = -s, -1 if cy > 0 else 1
                line(s * hl + sx * math.cos(a0), cy + sy * math.sin(a0), s * hl + sx * math.cos(a1), cy + sy * math.sin(a1))
    obj("lines", bm, white, coll)
    bm = bmesh.new()
    for s in (-1, 1):  # the goals
        x = s * hl
        add_box(bm, (x, -3.66, 0), (x + s * 0.15, -3.5, 2.44))
        add_box(bm, (x, 3.5, 0), (x + s * 0.15, 3.66, 2.44))
        add_box(bm, (x, -3.66, 2.3), (x + s * 0.15, 3.66, 2.44))
        add_box(bm, (x + s * 2.0, -3.66, 0), (x + s * 2.1, 3.66, 1.4))
    obj("goals", bm, white, coll)


def oct_ring(x0, x1, y0, y1, cut):
    return [(x0 + cut, y0), (x1 - cut, y0), (x1, y0 + cut), (x1, y1 - cut), (x1 - cut, y1), (x0 + cut, y1), (x0, y1 - cut), (x0, y0 + cut)]


def octagon(bm, box, cut, z0, z1):
    """A prism over the octagon in box (x0, x1, y0, y1), its corners cut by cut."""
    ring = oct_ring(*box, cut)
    lo = [bm.verts.new((x, y, z0)) for x, y in ring]
    hi = [bm.verts.new((x, y, z1)) for x, y in ring]
    bm.faces.new(hi)
    bm.faces.new(lo[::-1])
    for k in range(8):
        j = (k + 1) % 8
        bm.faces.new([lo[k], lo[j], hi[j], hi[k]])


def paving():
    """Light concrete slabs (the reference's plinth), a faint grid of joints."""
    m = bpy.data.materials.new("Paving")
    m.use_nodes = True
    nt = m.node_tree
    p = nt.nodes.get("Principled BSDF")
    p.inputs["Roughness"].default_value = 0.85
    tc = nt.nodes.new("ShaderNodeTexCoord")
    br = nt.nodes.new("ShaderNodeTexBrick")
    br.inputs["Scale"].default_value = 0.4
    br.inputs["Mortar Size"].default_value = 0.012
    br.inputs["Color1"].default_value = (0.075, 0.077, 0.081, 1)
    br.inputs["Color2"].default_value = (0.066, 0.068, 0.072, 1)
    br.inputs["Mortar"].default_value = (0.045, 0.046, 0.049, 1)
    br.offset = 0.0
    nt.links.new(tc.outputs["Object"], br.inputs["Vector"])
    nt.links.new(br.outputs["Color"], p.inputs["Base Color"])
    return m


def surroundings(coll, ex, ey, top, reach):
    """The ground on a paved octagonal plinth over the page's dark (the reference's diorama look):
    a kerb, a dark path round it, small trees along the edge, lattice floodlight towers."""
    rnd = random.Random(7)
    # (tight round the stands; deeper on the south side, the megastore's)
    box, cut = (-ex - 9, ex + 9, -ey - 37, ey + 9), 18
    bm = bmesh.new()
    octagon(bm, box, cut, -0.9, -0.05)
    obj("plaza", bm, paving(), coll)
    # a light kerb along the plinth's edge (the reference's low wall)
    bm = bmesh.new()
    outer, inner = oct_ring(*box, cut), oct_ring(box[0] + 0.9, box[1] - 0.9, box[2] + 0.9, box[3] - 0.9, cut - 0.37)
    for k in range(8):
        j = (k + 1) % 8
        q = [bm.verts.new((*p, z)) for z in (-0.06, 0.5) for p in (outer[k], outer[j], inner[j], inner[k])]
        for f in ((4, 5, 6, 7), (0, 1, 5, 4), (3, 2, 6, 7)[::-1]):
            bm.faces.new([q[i] for i in f])
    obj("kerb", bm, mat("Kerb", (0.2, 0.205, 0.215), 0.75), coll)
    bm = bmesh.new()
    octagon(bm, (box[0] - 7, box[1] + 7, box[2] - 7, box[3] + 7), cut + 2.9, -1.6, -1.25)
    obj("path", bm, mat("Path", (0.03, 0.031, 0.034), 0.85), coll)
    # the corners between the stands: low blocks, so the ground reads as one bowl
    walls = mat("Walls", (0.06, 0.063, 0.07), 0.7)
    bm, tops = bmesh.new(), bmesh.new()
    cxa, cya = (PL + 8) / 2 + 0.9, (PW + 4) / 2 + 0.9
    for sx in (-1, 1):
        for sy in (-1, 1):
            xa, xb = sorted((sx * cxa, sx * (ex - 1.0)))
            ya, yb = sorted((sy * cya, sy * (ey - 1.0)))
            if xb - xa > 2 and yb - ya > 2:
                add_box(bm, (xa, ya, 0), (xb, yb, top * 0.62 - 0.4))
                add_box(tops, (xa, ya, top * 0.62 - 0.4), (xb, yb, top * 0.62))
    obj("buildings", bm, walls, coll)
    obj("cornertops", tops, mat("CornerTop", (0.06, 0.065, 0.07), 0.7), coll)
    # floodlights: lattice towers in the corners, a bank of lamps tilted at the centre spot
    pole, lamp = bmesh.new(), bmesh.new()
    for sx in (-1, 1):
        for sy in (-1, 1):
            # (in the corners between the stands, as the reference's: at the ground's outer corners
            # the page's crop cut the left and right ones off, a friend's "no floodlights")
            x, y = sx * (cxa + 4), sy * (cya + 4)
            hgt = top + 26
            legs = lambda z: [(x + dx * (1.3 - 0.8 * z / hgt), y + dy * (1.3 - 0.8 * z / hgt), z) for dx, dy in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
            for k in range(4):
                strut(pole, legs(0)[k], legs(hgt)[k], 0.32)
            z, step = 0.0, 3.2
            while z < hgt - 0.1:
                z1 = min(z + step, hgt)
                lo, hi = legs(z), legs(z1)
                for k in range(4):
                    j = (k + 1) % 4
                    strut(pole, hi[k], hi[j], 0.18)
                    strut(pole, lo[k], hi[j], 0.12)
                z = z1
            # (half toward the pitch, half toward the camera: facing the pitch alone the left and
            # right heads were edge-on, as the reference's all face the viewer)
            face = Vector((-x, -y, 0)).normalized() + Vector((VIEW.x, VIEW.y, 0)).normalized()
            turn = math.atan2(face.y, face.x)
            head = Matrix.Translation((x, y, hgt + 2.2)) @ Matrix.Rotation(turn, 4, "Z") @ Matrix.Rotation(math.radians(-30), 4, "Y")
            bmesh.ops.create_cube(pole, size=1.0, matrix=head @ Matrix.Diagonal((0.6, 8.4, 5.4, 1.0)))
            for row in range(3):
                for col in range(4):
                    # (through the frame: lit seen from either side, the reference's four bright heads)
                    off = Matrix.Translation((0.0, -2.85 + col * 1.9, -1.7 + row * 1.7))
                    bmesh.ops.create_cube(lamp, size=1.0, matrix=head @ off @ Matrix.Diagonal((0.9, 1.25, 1.05, 1.0)))
            spot_data = bpy.data.lights.new(f"flood{sx}{sy}", "SPOT")
            # (closer since they stand in the corners: weaker, a wide soft cone, no bright patches)
            spot_data.energy = 200000
            spot_data.spot_size = math.radians(120)
            spot_data.spot_blend = 1.0
            spot_data.use_shadow = False  # (the stands' shadows from the corners cut the pitch into triangles)
            spot_data.color = (1.0, 0.97, 0.9)
            spot = bpy.data.objects.new(f"flood{sx}{sy}", spot_data)
            spot.location = (x, y, hgt + 1)
            spot.rotation_euler = (Vector((0, 0, 0)) - Vector((x, y, hgt))).to_track_quat("-Z", "Y").to_euler()
            coll.objects.link(spot)
    obj("poles", pole, mat("Pole", (0.16, 0.165, 0.175), 0.5, 0.6), coll)
    obj("lamps", lamp, mat("Lamp", (1.0, 0.98, 0.92), 0.3, 0.0, 2.2), coll)
    # small round trees and lamp posts along the plinth's edge
    crowns = [bmesh.new() for _ in range(2)]
    trunks, posts = bmesh.new(), bmesh.new()
    ring = oct_ring(box[0] + 3.5, box[1] - 3.5, box[2] + 3.5, box[3] - 3.5, cut - 1.5)
    for k in range(8):
        (x0, y0), (x1, y1) = ring[k], ring[(k + 1) % 8]
        n = max(1, int(math.hypot(x1 - x0, y1 - y0) // 15))
        for i in range(n):
            t = (i + 0.5) / n
            x, y = x0 + (x1 - x0) * t, y0 + (y1 - y0) * t
            if -PL / 2 - 14 < x < -PL / 2 + 26 and y < -ey + 2:  # (the megastore's corner)
                continue
            if (i + k) % 3:
                h = rnd.uniform(2.2, 3.0)
                add_box(trunks, (x - 0.3, y - 0.3, 0), (x + 0.3, y + 0.3, h))
                bmesh.ops.create_icosphere(crowns[rnd.randrange(2)], subdivisions=3, radius=rnd.uniform(2.6, 3.2), matrix=Matrix.Translation((x, y, h + 2.2)) @ Matrix.Diagonal((0.9, 0.9, 1.3, 1)))
            else:
                add_box(posts, (x - 0.12, y - 0.12, 0), (x + 0.12, y + 0.12, 6.0))
                add_box(posts, (x - 0.35, y - 0.35, 6.0), (x + 0.35, y + 0.35, 6.25))
    obj("trunks", trunks, mat("Bark", (0.1, 0.07, 0.045), 0.9), coll)
    obj("posts", posts, mat("Pole", (0.16, 0.165, 0.175), 0.5, 0.6), coll)
    for k, c in enumerate([(0.03, 0.11, 0.025), (0.045, 0.14, 0.03)]):
        obj(f"trees{k}", crowns[k], mat(f"Leaves{k}", c, 0.8), coll)


# ---------------------------------------------------------------- scene, camera, renders
def scene_for(size):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    MATS.clear()
    sc = bpy.context.scene
    colls = []
    for name in ("Base", "Roof", "Works", "Shop", "Mega"):
        c = bpy.data.collections.new(name)
        sc.collection.children.link(c)
        colls.append(c)
    depth, top = build(size, colls)
    pitch(colls[0])
    ex, ey = PL / 2 + GAP + depth + 2.5, PW / 2 + GAP + depth + 2.5
    club_shop(colls[3], ey)
    club_shop(colls[4], ey, mega=True)
    # the camera: orthographic, from the south-south-west, the ground filling the picture
    cam_data = bpy.data.cameras.new("cam")
    cam_data.type = "ORTHO"
    cam = bpy.data.objects.new("cam", cam_data)
    sc.collection.objects.link(cam)
    cam.location = VIEW * 600
    cam.rotation_euler = (-VIEW).to_track_quat("-Z", "Y").to_euler()
    cam_data.clip_end = 2000
    sc.camera = cam
    bpy.context.view_layer.update()
    inv = cam.matrix_world.inverted()
    corners = [inv @ Vector((x, y, z)) for x in (-ex, ex) for y in (-ey, ey) for z in (0, top + 8)]
    half_w = max(abs(c.x) for c in corners)
    half_h = max(abs(c.y) for c in corners)
    # (ortho_scale is the picture's width: a portrait picture's would be its height, and the
    # ground came out cut at the sides, night 11)
    cam_data.sensor_fit = "HORIZONTAL"
    cam_data.ortho_scale = max(2 * half_w, 2 * half_h * W / H) * 1.06
    reach = cam_data.ortho_scale
    surroundings(colls[0], ex, ey, top, reach)
    sun_data = bpy.data.lights.new("sun", "SUN")
    sun_data.energy = 2.4
    sun_data.angle = math.radians(3)
    sun = bpy.data.objects.new("sun", sun_data)
    sun.rotation_euler = SUN.to_track_quat("Z", "Y").to_euler()
    sc.collection.objects.link(sun)
    world = bpy.data.worlds.new("w")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs[0].default_value = (0.5, 0.53, 0.58, 1)
    world.node_tree.nodes["Background"].inputs[1].default_value = 0.4
    sc.world = world
    r = sc.render
    r.resolution_x, r.resolution_y, r.resolution_percentage = W, H, 100
    r.film_transparent = True
    r.image_settings.file_format = "PNG"
    r.image_settings.color_mode = "RGBA"
    sc.view_settings.view_transform = "Standard"
    from bpy_extras.object_utils import world_to_camera_view
    def at(p):
        v = world_to_camera_view(sc, cam, Vector(p))
        return (round(v.x * W), round((1 - v.y) * H))
    mid = lambda side: frame(side)[0](0, depth / 2, top * 0.55)
    points = [at((0, 0, 0))] + [at(mid(s)) for s in range(4)]
    edge = max(math.dist(at((0, 0, 0)), at(p)) for p in [(9.15, 0, 0), (0, 9.15, 0)])
    points.append((round(edge), 0))
    # the pitch's own axes on the picture (the centre circle's edge along x and along y): the DLL
    # lays the crest flat on the grass with them (the mock's crest)
    points += [at((9.15, 0, 0.05)), at((0, 9.15, 0.05))]
    # the two long roofs' tops as the screen has them (top left, top right, bottom left): the DLL
    # writes the club's name on the far one and the ground's on the near one (the mock's HERACLES,
    # ALMELO); far = stand 0 (north), near = stand 1 (south)
    def roof_corners(side):
        w, length = frame(side)
        front = depth * ROOF_FRONT
        zb, zf = top + 5.5, top + 5.0 + 8 * 0.35 + 0.5
        pts = [at(w(-length / 2 - 1, depth + 1.6, zb)), at(w(length / 2 + 1, depth + 1.6, zb)), at(w(length / 2 + 1, front, zf)), at(w(-length / 2 - 1, front, zf))]
        ends = [(pts[0], pts[3]), (pts[1], pts[2])]  # (back, front) at each end of the stand
        back_top = (pts[0][1] + pts[1][1]) < (pts[2][1] + pts[3][1])
        tops = [e[0] if back_top else e[1] for e in ends]
        bots = [e[1] if back_top else e[0] for e in ends]
        i = 0 if tops[0][0] < tops[1][0] else 1
        return [tops[i], tops[1 - i], bots[i]]
    global ROOF_POINTS, EXTENT
    ROOF_POINTS = roof_corners(0) + roof_corners(1) + [(0, 0), (0, 0)]
    EXTENT = (ex, ey)
    return sc, colls, points


def club_shop(coll, ey, mega=False):
    """The club shop / megastore (user 2026-11-23, after the isolated megastore picture): a white
    block beside the south stand's near (west) end, its glass front and sign facing west (the
    camera), pillars, sign band and a roof panel in the club's first colour (id class 3), the name
    in its second (class 2), a roof unit, planters. Its own layer, drawn once the club has one."""
    wx, wy, h = (24.0, 32.0, 12.0) if mega else (15.0, 20.0, 8.0)
    x0 = -PL / 2 - 7.5
    x1, y1 = x0 + wx, -ey - 3.0
    y0 = y1 - wy
    club = id_colour(3)
    walls, red, glass, frames, darks, greens, roof = (bmesh.new() for _ in range(7))
    add_box(darks, (x0 - 2.5, y0 - 1.2, -0.05), (x1 + 1.2, y1 + 0.5, 0.3))  # the base slab
    add_box(walls, (x0, y0, 0.3), (x1, y1, h))
    p = 1.3  # pillar width
    # pillars on the corners the camera sees, the sign band over the front
    for x, y in ((x0, y0), (x0, y1 - p), (x1 - p, y0)):
        add_box(red, (x - 0.3, y - 0.3 if y == y0 else y, 0.3), (x + p, y + p + (0.3 if y != y0 else 0), h + 0.1))
    band = (h * 0.56, h * 0.95)
    add_box(red, (x0 - 0.35, y0 + p, band[0]), (x0, y1 - p, band[1]))
    add_box(red, (x0 + p, y0 - 0.3, h * 0.62), (x1 - p, y0, h * 0.68))  # a stripe along the side
    add_box(red, (x0 + p, y0 - 0.3, 0.3), (x1 - p, y0, 0.75))
    # the shop windows: lit inside, dark frames
    g_top = band[0] - 0.35
    add_box(glass, (x0 - 0.1, y0 + p + 0.2, 0.75), (x0 + 0.05, y1 - p - 0.2, g_top))
    add_box(glass, (x0 + p + 0.3, y0 - 0.1, 0.95), (x1 - p - 4.0, y0 + 0.05, h * 0.58))
    add_box(frames, (x0 - 0.25, y0 + p, g_top - 0.05), (x0, y1 - p, g_top + 0.3))
    add_box(frames, (x0 - 0.25, y0 + p, 0.3), (x0, y1 - p, 0.75))
    add_box(frames, (x0 - 0.22, y0 + p, h * 0.38), (x0, y1 - p, h * 0.38 + 0.14))
    y = y0 + p + 2.2
    while y < y1 - p - 0.5:
        add_box(frames, (x0 - 0.22, y - 0.08, 0.75), (x0, y + 0.08, g_top))
        y += 2.2
    x = x0 + p + 0.3 + 2.0
    while x < x1 - p - 4.0:
        add_box(frames, (x - 0.08, y0 - 0.2, 0.95), (x + 0.08, y0, h * 0.58))
        x += 2.0
    # a door at the side's far end, in club colour; the front's glass doors in the middle
    add_box(red, (x1 - p - 3.2, y0 - 0.15, 0.3), (x1 - p - 1.4, y0 + 0.05, 2.9))
    ym = (y0 + y1) / 2
    add_box(frames, (x0 - 0.24, ym - 0.06, 0.75), (x0, ym + 0.06, g_top))  # the door's middle
    # planters with shrubs along the front
    for yy in (y0 + p + 3.0, y1 - p - 3.0):
        add_box(darks, (x0 - 2.2, yy - 1.6, 0.3), (x0 - 0.5, yy + 1.6, 1.0))
        for k in range(3):
            bmesh.ops.create_icosphere(greens, subdivisions=2, radius=0.6, matrix=Matrix.Translation((x0 - 1.35, yy - 1.0 + k, 1.25)) @ Matrix.Diagonal((1, 1, 0.75, 1)))
    # the roof: a parapet, a panel in club colour, an air unit and a vent
    add_box(roof, (x0 - 0.3, y0 - 0.3, h), (x1 + 0.3, y1 + 0.3, h + 0.15))
    for a, b in [((x0 - 0.3, y0 - 0.3), (x1 + 0.3, y0 + 0.4)), ((x0 - 0.3, y1 - 0.4), (x1 + 0.3, y1 + 0.3)),
                 ((x0 - 0.3, y0 - 0.3), (x0 + 0.4, y1 + 0.3)), ((x1 - 0.4, y0 - 0.3), (x1 + 0.3, y1 + 0.3))]:
        add_box(walls, (*a, h), (*b, h + 0.7))
    add_box(red, (x0 + wx * 0.3, y0 + wy * 0.25, h + 0.15), (x1 - 1.5, y1 - wy * 0.2, h + 0.3))
    add_box(frames, (x0 + wx * 0.42, y0 + wy * 0.3, h + 0.3), (x1 - 2.5, y0 + wy * 0.3 + 1.0, h + 0.75))
    add_box(walls, (x1 - wx * 0.35, y1 - wy * 0.38, h + 0.15), (x1 - 1.6, y1 - 1.8, h + 1.8))
    add_box(frames, (x1 - wx * 0.35 + 0.4, y1 - wy * 0.38 - 0.05, h + 0.5), (x1 - 2.0, y1 - wy * 0.38, h + 1.5))
    add_box(frames, (x0 + 2.0, y1 - 3.4, h + 0.15), (x0 + 2.7, y1 - 2.7, h + 1.1))
    obj("shopwalls", walls, mat("ShopWalls", (0.62, 0.62, 0.63), 0.6), coll)
    obj("shopred", red, mat("Trim", (0.85, 0.85, 0.85), 0.4), coll, club)
    # (dark glass with the shop lit behind it, as the reference's)
    obj("shopglass", glass, mat("ShopGlass", (0.75, 0.6, 0.42), 0.1, 0.0, 0.28), coll)
    obj("shopframes", frames, mat("ShopFrames", (0.03, 0.032, 0.036), 0.5), coll)
    obj("shopbase", darks, mat("ShopBase", (0.06, 0.062, 0.068), 0.8), coll)
    obj("shopgreen", greens, mat("Leaves1", (0.045, 0.14, 0.03), 0.8), coll)
    obj("shoproof", roof, mat("ShopRoof", (0.09, 0.092, 0.1), 0.7), coll)
    # the name on the band, facing west, in the club's second colour
    cu = bpy.data.curves.new("shopsign", "FONT")
    cu.body = "MEGASTORE" if mega else "CLUB SHOP"
    cu.size = (band[1] - band[0]) * 0.62
    cu.align_x = "CENTER"
    cu.align_y = "CENTER"
    cu.extrude = 0.06
    # (a bold sans, the reference's lettering; Blender's own font where Windows has none)
    for path in (r"C:\Windows\Fonts\arialbd.ttf", r"C:\Windows\Fonts\segoeuib.ttf"):
        if os.path.exists(path):
            cu.font = bpy.data.fonts.load(path, check_existing=True)
            break
    text = bpy.data.objects.new("shopsign", cu)
    text.location = (x0 - 0.45, ym, (band[0] + band[1]) / 2)
    text.rotation_euler = (math.radians(90), 0, math.radians(-90))
    text.data.materials.append(mat("Seat", (0.82, 0.82, 0.82), 0.45))
    text.color = id_colour(2)
    coll.objects.link(text)
    # (the band's length: the name no wider than it)
    bpy.context.view_layer.update()
    span = (y1 - p) - (y0 + p) - 1.6
    if text.dimensions.x > span:
        cu.size *= span / text.dimensions.x


def show(sc, colls, base, roof, works, shop="off", mega="off"):
    """Each collection: 'on', 'hold' (held out: cuts what is behind) or 'off'."""
    lc = bpy.context.view_layer.layer_collection.children
    for c, how in zip(colls, (base, roof, works, shop, mega)):
        lc[c.name].exclude = how == "off"
        if how != "off":
            lc[c.name].holdout = how == "hold"


def render(sc, path, workbench=False):
    if workbench:
        sc.render.engine = "BLENDER_WORKBENCH"
        sh = sc.display.shading
        sh.light = "FLAT"
        sh.color_type = "OBJECT"
        sh.show_shadows = False
        sh.show_cavity = False
        sh.show_object_outline = False
        sh.show_specular_highlight = False
        sc.display.render_aa = "OFF"
    else:
        sc.render.engine = "BLENDER_EEVEE"
        sc.eevee.taa_render_samples = 64
        # (ray-traced light: soft contact shadows under the roofs and between the stands, the
        # mock's look; where this Blender has the settings)
        for k, v in (("use_raytracing", True), ("use_shadows", True), ("fast_gi_method", "GLOBAL_ILLUMINATION")):
            try:
                setattr(sc.eevee, k, v)
            except Exception:
                pass
    sc.render.filepath = path
    bpy.ops.render.render(write_still=True)


def pixels(path):
    img = bpy.data.images.load(path)
    a = np.empty(img.size[0] * img.size[1] * 4, dtype=np.float32)
    img.pixels.foreach_get(a)
    a = (a.reshape(img.size[1], img.size[0], 4)[::-1] * 255 + 0.5).astype(np.uint8)
    bpy.data.images.remove(img)
    return a


def runs(data):
    """4-byte pixels as runs: (count 1..255, pixel), the pack's 'CMS2'."""
    a = np.frombuffer(data, dtype=np.uint32)
    if len(a) == 0:
        return b""
    starts = np.flatnonzero(np.concatenate(([True], a[1:] != a[:-1])))
    lens = np.diff(np.concatenate((starts, [len(a)])))
    out = bytearray()
    for s, n in zip(starts.tolist(), lens.tolist()):
        v = a[s].tobytes()
        while n > 0:
            k = min(n, 255)
            out.append(k)
            out += v
            n -= k
    return bytes(out)


def packed(rgba, ids=None, crop=False):
    """(x, y, w, h, bytes): RGB565 little-endian, alpha, id per pixel; cropped to what is drawn."""
    y0, x0, y1, x1 = 0, 0, rgba.shape[0], rgba.shape[1]
    if crop:
        ys, xs = np.nonzero(rgba[:, :, 3])
        if len(ys) == 0:
            return (0, 0, 0, 0, b"")
        y0, y1, x0, x1 = ys.min(), ys.max() + 1, xs.min(), xs.max() + 1
    p = rgba[y0:y1, x0:x1].astype(np.uint16)
    c565 = ((p[:, :, 0] >> 3) << 11) | ((p[:, :, 1] >> 2) << 5) | (p[:, :, 2] >> 3)
    out = np.zeros((y1 - y0, x1 - x0, 4), dtype=np.uint8)
    out[:, :, 0] = c565 & 0xFF
    out[:, :, 1] = c565 >> 8
    out[:, :, 2] = rgba[y0:y1, x0:x1, 3]
    if ids is not None:
        i = ids[y0:y1, x0:x1].astype(np.int32)
        cls = np.clip((i[:, :, 0] + 40) // 80, 0, 3)
        stand = np.clip((i[:, :, 1] + 40) // 80, 0, 3)
        seg = np.clip((i[:, :, 2] + 8) // 16, 0, 15)
        out[:, :, 3] = np.where(i[:, :, 3] > 0, cls | (stand << 2) | (seg << 4), 0)
    return (int(x0), int(y0), int(x1 - x0), int(y1 - y0), out.tobytes())


def main():
    os.makedirs(OUT, exist_ok=True)
    entries = []
    info = {}
    only = os.environ.get("STADIUM_ONLY")  # e.g. "m": one size, for a quick look
    for size in SIZES:
        if only and size not in only:
            continue
        sc, colls, points = scene_for(size)
        info[size] = points
        f = lambda kind: os.path.join(OUT, f"{kind}_{size}.png")
        show(sc, colls, "on", "off", "off"); render(sc, f("base"))
        render(sc, f("id"), workbench=True)
        show(sc, colls, "hold", "on", "off"); render(sc, f("roof"))
        show(sc, colls, "hold", "off", "on"); render(sc, f("works"))
        show(sc, colls, "hold", "off", "off", "on"); render(sc, f("shop"))
        show(sc, colls, "on", "off", "off", "on"); render(sc, f("idshop"), workbench=True)
        show(sc, colls, "hold", "off", "off", "off", "on"); render(sc, f("mega"))
        show(sc, colls, "on", "off", "off", "off", "on"); render(sc, f("idmega"), workbench=True)
        # the clean top-down view (the brief's second render; the page uses the 3/4 one), the
        # long side upright in the portrait picture
        ex, ey = EXTENT
        sc.camera.location = (0, 0, 600)
        sc.camera.rotation_euler = (0, 0, math.pi / 2)
        sc.camera.data.ortho_scale = max(2 * (ey + 14), 2 * (ex + 14) * W / H) * 1.04
        show(sc, colls, "on", "off", "off"); render(sc, f("top"))
        entries.append((f"base_{size}", points, packed(pixels(f("base")), pixels(f("id")))))
        entries.append((f"roof_{size}", ROOF_POINTS, packed(pixels(f("roof")), crop=True)))
        entries.append((f"works_{size}", points, packed(pixels(f("works")), crop=True)))
        # (with ids: the pillars, band and name take the club's colours, as the seats do)
        entries.append((f"shop_{size}", points, packed(pixels(f("shop")), pixels(f("idshop")), crop=True)))
        entries.append((f"mega_{size}", points, packed(pixels(f("mega")), pixels(f("idmega")), crop=True)))
    # 'CMS2', count; entries of 64 bytes: name[16], full w, full h, x, y, w, h (u16), offset (u32),
    # eight points (i16 x, y: pitch centre, the N S W E stands' middles, centre circle radius, 0, 0;
    # a roof's: its two long tops' corners); each picture's pixels as runs (count 1..255, pixel)
    # ('CMST' was the pixels as they are: 32 MB for the friends' zip, night 11)
    head = 8 + 64 * len(entries)
    body, table = b"", b""
    for name, points, (x, y, w, h, data) in entries:
        pts = (list(points) + [(0, 0)] * 8)[:8]
        table += struct.pack("<16s6HI16h", name.encode(), W, H, x, y, w, h, head + len(body), *[v for p in pts for v in p])
        body += runs(data)
    with open(os.path.join(OUT, "cmui_stadium.bin"), "wb") as fh:
        fh.write(b"CMS2" + struct.pack("<I", len(entries)) + table + body)
    with open(os.path.join(OUT, "points.json"), "w") as fh:
        json.dump(info, fh, indent=1)
    print("written", len(entries), "pictures,", len(body) // 1024, "KB")


main()
