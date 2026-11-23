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
W, H = 1560, 1080  # twice the page's portrait column (780): the same scale, the town either side
# for the page's FULL SCREEN (2026-11-23); the column cuts the middle as before
COLUMN_W = 780
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
    if size == "x":
        corners(rows, depth, top, seat, facade, metal, base, roof_c)
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


def corners(rows, depth, top, seat, facade, metal, base, roof_c):
    """A world-class ground closed all round (a friend's idea, 2026-11-23): each corner filled with the same
    rows as the stands, an L of treads between the long stand's and the end stand's rows, seats in
    the mosaic, a back wall and a roof over the outer rows."""
    sx0, sy0 = (PL + 8) / 2, (PW + 4) / 2  # the stands' ends
    fx, fy = PL / 2 + GAP, PW / 2 + GAP  # the end stands' and the long stands' front rows
    front = depth * ROOF_FRONT
    for qx in (-1, 1):
        for qy in (-1, 1):
            P = lambda x, y, z: (qx * x, qy * y, z)
            for k, (u0, u1, z, t) in enumerate(rows):
                bm = bmesh.new()
                floor = 0.0 if t == 0 else z - 2.5
                # (along the long stand's row, then down the end stand's)
                add_box(bm, P(sx0, fy + u0, floor), P(fx + u1, fy + u1, z))
                add_box(bm, P(fx + u0, sy0, floor), P(fx + u1, fy + u0, z))
                add_box(bm, P(sx0, fy + u1 - 0.32, z), P(fx + u1 - 0.32, fy + u1 - 0.12, z + 0.42))
                add_box(bm, P(fx + u1 - 0.32, sy0, z), P(fx + u1 - 0.12, fy + u1 - 0.32, z + 0.42))
                band = k * BANDS // len(rows)
                obj(f"corner{qx}{qy}_{k}", bm, seat, base, id_colour(1 + band % 2, 0 if qy > 0 else 1, 0))
            bm = bmesh.new()  # the back wall
            add_box(bm, P(sx0 - 1, fy + depth, 0), P(fx + depth + 1.4, fy + depth + 1.4, top + 2.2))
            add_box(bm, P(fx + depth, sy0 - 1, 0), P(fx + depth + 1.4, fy + depth, top + 2.2))
            obj(f"cornerback{qx}{qy}", bm, facade, base)
            bm = bmesh.new()  # the roof, level with the stands' at its middle
            z = top + 5.0 + 4 * 0.35
            add_box(bm, P(sx0 - 1, fy + front, z), P(fx + depth + 1.6, fy + depth + 1.6, z + 0.5))
            add_box(bm, P(fx + front, sy0 - 1, z), P(fx + depth + 1.6, fy + front, z + 0.5))
            obj(f"cornerroof{qx}{qy}", bm, metal, roof_c)


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
            if xb - xa > 2 and yb - ya > 2 and LEVEL < 3:  # (a huge ground's corners are seated, `corners`)
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
            hgt, z0 = top + 26, 0.0
            if LEVEL == 3:  # (seated corners: the mast stands on the corner roof's inner edge)
                f = (ex - 2.5 - PL / 2 - GAP) * ROOF_FRONT
                x, y, z0, hgt = sx * (PL / 2 + GAP + f), sy * (PW / 2 + GAP + f), top + 6.9, top + 18
            legs = lambda z: [(x + dx * (1.3 - 0.8 * z / hgt), y + dy * (1.3 - 0.8 * z / hgt), z) for dx, dy in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
            for k in range(4):
                strut(pole, legs(z0)[k], legs(hgt)[k], 0.32)
            z, step = z0, 3.2
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
            spot_data.energy = 200000 if LEVEL < 3 else 90000  # (on the roof: closer, it glared)
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
    town(coll, box, ex, ey, reach)


def town(coll, box, ex, ey, reach):
    """The ground in its town again (the user's friend, 2026-11-23: the old pictures' car parks,
    roads, grass and kiosks): grass to the picture's edges, a ring road round the plinth, the main
    road in front, car parks east and west, kiosks on the plinth, trees, low buildings and a pub."""
    rnd = random.Random(23)
    g = -1.62  # (the grass' top: below the plinth and its dark path)
    bm = bmesh.new()
    add_box(bm, (-reach * 3, -reach * 3, g - 0.3), (reach * 3, reach * 3, g))
    obj("grass", bm, mat("Grass2", (0.035, 0.085, 0.028), 0.95), coll)
    # the ring road: a rounded rectangle round the plinth's path, a dashed middle line
    x0, x1, y0, y1 = box[0] - 7, box[1] + 7, box[2] - 7, box[3] + 7
    cy, hx, hy, rad = (y0 + y1) / 2, (x1 - x0) / 2 + 11, (y1 - y0) / 2 + 11, 26.0
    road, dashes = bmesh.new(), bmesh.new()
    def ring_point(t):
        sx, sy = hx - rad, hy - rad
        legs = [2 * sx, 2 * sy, 2 * sx, 2 * sy]
        arc = math.pi * rad / 2
        d = t * (sum(legs) + 4 * arc)
        corners = [(sx, -sy, -math.pi / 2), (sx, sy, 0.0), (-sx, sy, math.pi / 2), (-sx, -sy, math.pi)]
        starts = [(-sx, -(sy + rad), 1, 0), (sx + rad, -sy, 0, 1), (sx, sy + rad, -1, 0), (-(sx + rad), sy, 0, -1)]
        for k in range(4):
            px, py, dx, dy = starts[k]
            if d <= legs[k]:
                return px + dx * d, cy + py + dy * d
            d -= legs[k]
            if d <= arc:
                ccx, ccy, a0 = corners[k]
                return ccx + rad * math.cos(a0 + d / rad), cy + ccy + rad * math.sin(a0 + d / rad)
            d -= arc
        return starts[0][0], cy + starts[0][1]
    steps = 240
    for k in range(steps):
        (ax, ay), (bx, by) = ring_point(k / steps), ring_point((k + 1) / steps)
        ang, seg = math.atan2(by - ay, bx - ax), math.hypot(bx - ax, by - ay) + 0.6
        at = Matrix.Translation(((ax + bx) / 2, (ay + by) / 2, g + 0.05)) @ Matrix.Rotation(ang, 4, "Z")
        bmesh.ops.create_cube(road, size=1.0, matrix=at @ Matrix.Diagonal((seg, 9.0, 0.14, 1.0)))
        if k % 3 == 0:
            bmesh.ops.create_cube(dashes, size=1.0, matrix=Matrix.Translation((0, 0, 0.08)) @ at @ Matrix.Diagonal((seg * 1.4, 0.3, 0.02, 1.0)))
    ring_s, ring_n, ring_w, ring_e = cy - hy, cy + hy, -hx, hx
    main = ring_s - 26  # the main road in front, its middle
    add_box(road, (-reach * 3, main - 6, g), (reach * 3, main + 6, g + 0.12))
    x = -reach * 3
    while x < reach * 3:
        add_box(dashes, (x, main - 0.15, g + 0.12), (x + 3.0, main + 0.15, g + 0.14))
        x += 7.0
    for rx in (-30.0, 34.0):  # two roads in from the main road
        add_box(road, (rx - 4.5, main, g), (rx + 4.5, ring_s + 2, g + 0.12))
    # car parks east and west of the ring road, a lane into each
    # (the town grows with the ground, a friend's idea 2026-11-23: an amateur ground one small car park,
    # two kiosks, no fan zone; a huge one big car parks, every stall, a crowd)
    lv = LEVEL
    parks = [
        [(ring_e + 8, ring_e + 48, cy - 22, cy + 18)],
        [(ring_e + 8, ring_e + 64, cy - 36, cy + 30), (ring_w - 44, ring_w - 8, cy - 22, cy + 26)],
        [(ring_e + 8, ring_e + 74, cy - 46, cy + 40), (ring_w - 60, ring_w - 8, cy - 30, cy + 36)],
        [(ring_e + 8, ring_e + 116, cy - 60, cy + 52), (ring_w - 96, ring_w - 8, cy - 46, cy + 50)],
    ][lv]
    cars = [bmesh.new() for _ in range(6)]
    bays = bmesh.new()
    for px0, px1, py0, py1 in parks:
        add_box(road, (px0, py0, g), (px1, py1, g + 0.12))
        lane = (ring_e - 2, px0 + 1) if px0 > 0 else (px1 - 1, ring_w + 2)
        add_box(road, (min(lane), cy - 5, g), (max(lane), cy + 5, g + 0.12))
        x = px0 + 5
        while x + 15 < px1:
            y = py0 + 3
            while y < py1 - 3:
                add_box(bays, (x - 0.08, y, g + 0.12), (x + 5.2, y + 0.12, g + 0.14))
                add_box(bays, (x + 9.8, y, g + 0.12), (x + 15, y + 0.12, g + 0.14))
                for xx in (x + 0.4, x + 10.2):
                    if rnd.random() < 0.72:
                        c = cars[rnd.randrange(6)]
                        add_box(c, (xx, y + 0.45, g + 0.12), (xx + 4.4, y + 2.25, g + 1.1))
                        add_box(c, (xx + 1.1, y + 0.6, g + 1.1), (xx + 3.4, y + 2.1, g + 1.6))
                y += 2.7
            x += 21
    obj("roads", road, mat("Asphalt", (0.028, 0.029, 0.033), 0.85), coll)
    obj("roadlines", dashes, mat("Paint", (0.75, 0.75, 0.72), 0.7), coll)
    obj("bays", bays, mat("Paint", (0.75, 0.75, 0.72), 0.7), coll)
    for k, c in enumerate([(0.6, 0.6, 0.62), (0.06, 0.06, 0.07), (0.5, 0.05, 0.04), (0.05, 0.12, 0.4), (0.85, 0.85, 0.85), (0.6, 0.45, 0.08)]):
        obj(f"cars{k}", cars[k], mat(f"Car{k}", c, 0.3, 0.4), coll)
    # food and drink, big enough to read (the user: "food stalls etc. (properly shown)"): kiosks
    # with striped awnings round the plinth; in front of the near stand, beside the megastore, a
    # fan zone: food trucks, a beer tent, picnic tables under parasols, fans in the club's colours
    body, lit, people, tables = bmesh.new(), bmesh.new(), bmesh.new(), bmesh.new()
    stripes = [bmesh.new() for _ in range(5)]  # red, yellow, blue, green, white
    def awning(x0, y0, x1, y1, z, a, b, along_x=True):
        """A striped awning: stripes 0.8 m wide, colours a and b by turns."""
        n = max(2, int(((x1 - x0) if along_x else (y1 - y0)) / 0.8))
        for k in range(n):
            if along_x:
                xa = x0 + (x1 - x0) * k / n
                add_box(stripes[a if k % 2 == 0 else b], (xa, y0, z), (xa + (x1 - x0) / n, y1, z + 0.35))
            else:
                ya = y0 + (y1 - y0) * k / n
                add_box(stripes[a if k % 2 == 0 else b], (x0, ya, z), (x1, ya + (y1 - y0) / n, z + 0.35))
    spots = [(x, ey + 4.0, True) for x in range(-56, 57, 16)] + [(-ex - 4.5, y, False) for y in (-26, -9, 9, 26)] + [(ex + 4.5, y, False) for y in (-26, -9, 9, 26)]
    spots = [[(-8, ey + 4.0, True), (8, ey + 4.0, True)], spots[:8] + [spots[9], spots[10], spots[13], spots[14]], spots, spots][lv]
    for k, (x, y, along_x) in enumerate(spots):
        w, d = (4.6, 3.2) if along_x else (3.2, 4.6)
        add_box(body, (x - w / 2, y - d / 2, 0), (x + w / 2, y + d / 2, 2.8))
        add_box(lit, (x - w / 2 + 0.4, y - d / 2 - 0.06, 1.0), (x + w / 2 - 0.4, y - d / 2 + 0.1, 2.2)) if along_x else add_box(lit, (x - w / 2 - 0.06, y - d / 2 + 0.4, 1.0), (x - w / 2 + 0.1, y + d / 2 - 0.4, 2.2))
        awning(x - w / 2 - 0.5, y - d / 2 - 1.0, x + w / 2 + 0.5, y + d / 2 + 0.3, 2.8, k % 4, 4, along_x)
    # the fan zone: from the megastore's east side to near the east end, the near stand's back to
    # the plinth's edge
    fx0, fx1, fy0, fy1 = -PL / 2 + 21, PL / 2 + 2, -ey - 33, -ey - 6
    trucks = [(), (fx0 + 2, fx0 + 14), (fx0 + 2, fx0 + 14, fx0 + 26, fx0 + 38), (fx0 + 2, fx0 + 14, fx0 + 26, fx0 + 38, fx0 + 50)][lv]
    for k, x in enumerate(trucks):  # food trucks, hatches south
        y = fy1 - 4
        add_box(body, (x, y - 1.3, 0.4), (x + 6.5, y + 1.3, 3.2))
        add_box(body, (x + 6.5, y - 1.3, 0.4), (x + 8.6, y + 1.3, 2.5))  # the cab
        add_box(lit, (x + 0.8, y - 1.36, 1.5), (x + 5.7, y - 1.2, 2.7))
        awning(x + 0.4, y - 2.6, x + 6.1, y - 1.3, 2.8, k % 4, 4)
        add_box(tables, (x + 1, y - 1.3, 0), (x + 7.6, y + 1.3, 0.4))  # the wheels' shadow line
    tx0 = fx1 - 18  # the beer tent: white, a pitched roof (large grounds and up)
    if lv >= 2:
        add_box(body, (tx0, fy0 + 8, 0), (tx0 + 14, fy0 + 18, 2.6))
        tent = bmesh.new()
        v = [tent.verts.new(p) for p in [(tx0 - 0.3, fy0 + 7.7, 2.6), (tx0 + 14.3, fy0 + 7.7, 2.6), (tx0 + 14.3, fy0 + 18.3, 2.6), (tx0 - 0.3, fy0 + 18.3, 2.6), (tx0 - 0.3, fy0 + 13, 4.6), (tx0 + 14.3, fy0 + 13, 4.6)]]
        for f in ((0, 1, 5, 4), (3, 4, 5, 2), (0, 4, 3), (1, 2, 5)):
            tent.faces.new([v[i] for i in f])
        obj("beertent", tent, mat("Tent", (0.85, 0.85, 0.83), 0.6), coll)
        add_box(lit, (tx0 + 2, fy0 + 7.94, 0.6), (tx0 + 12, fy0 + 8.05, 2.2))
    # picnic tables under parasols
    paras = [bmesh.new() for _ in range(3)]
    for i in range([0, 3, 5, 5][lv]):
        for j in range(2):
            x, y = fx0 + 4 + i * 7.5, fy0 + 6 + j * 8
            add_box(tables, (x - 1.2, y - 0.45, 0.7), (x + 1.2, y + 0.45, 0.8))
            add_box(tables, (x - 1.2, y - 1.1, 0.4), (x + 1.2, y - 0.8, 0.5))
            add_box(tables, (x - 1.2, y + 0.8, 0.4), (x + 1.2, y + 1.1, 0.5))
            add_box(tables, (x - 0.06, y - 0.06, 0.8), (x + 0.06, y + 0.06, 2.4))
            bmesh.ops.create_cone(paras[(i + j) % 3], cap_ends=True, segments=12, radius1=1.7, radius2=0.05, depth=0.7, matrix=Matrix.Translation((x, y, 2.6)))
    for k, c in enumerate([(0.7, 0.08, 0.05), (0.9, 0.75, 0.1), (0.1, 0.3, 0.65)]):
        obj(f"parasols{k}", paras[k], mat(f"Parasol{k}", c, 0.6), coll)
    # fans: in the club's colours (most) or dark coats, in the fan zone and on the way in
    fans = bmesh.new()
    def person(bm, x, y):
        add_box(bm, (x - 0.25, y - 0.2, 0), (x + 0.25, y + 0.2, 1.4))
        add_box(bm, (x - 0.14, y - 0.14, 1.4), (x + 0.14, y + 0.14, 1.75))
    for k in range([0, 90, 260, 420][lv]):
        x, y = rnd.uniform(fx0, fx1 if lv > 1 else fx0 + 34), rnd.uniform(fy0 + 1, fy1 - 1)
        person(fans if rnd.random() < 0.65 else people, x, y)
    for k in range([20, 50, 90, 160][lv]):  # (on the plinth round the stands: west and north)
        x, y = (rnd.uniform(-ex - 8, -ex - 1), rnd.uniform(-35, 35)) if k % 2 else (rnd.uniform(-60, 60), rnd.uniform(ey + 1, ey + 8))
        person(fans if rnd.random() < 0.6 else people, x, y)
    obj("stalls", body, mat("Stall", (0.75, 0.75, 0.73), 0.6), coll)
    obj("hatches", lit, mat("Hatch", (1.0, 0.82, 0.55), 0.2, 0.0, 1.6), coll)
    obj("tables", tables, mat("Wood", (0.2, 0.12, 0.06), 0.8), coll)
    obj("people", people, mat("Coats", (0.05, 0.055, 0.07), 0.8), coll)
    obj("fans", fans, mat("Trim", (0.85, 0.85, 0.85), 0.4), coll, id_colour(3))
    for k, c in enumerate([(0.7, 0.06, 0.04), (0.9, 0.7, 0.05), (0.06, 0.2, 0.6), (0.06, 0.4, 0.12), (0.9, 0.9, 0.88)]):
        obj(f"stripes{k}", stripes[k], mat(f"Stripe{k}", c, 0.6), coll)
    # two training pitches in the fields, either side (the full screen view)
    # (placed by the camera: left and right in the part of the picture the page's FULL SCREEN
    # shows; at the frame's corners the page cut them off, the user 2026-11-23)
    cam = bpy.context.scene.camera
    def ground_at(u, v):
        s = cam.data.ortho_scale
        o = cam.matrix_world @ Vector(((u - 0.5) * s, (0.5 - v) * s * H / W, 0))
        d = cam.matrix_world.to_3x3() @ Vector((0, 0, -1))
        t = (g - o.z) / d.z
        return o.x + d.x * t, o.y + d.y * t
    def clear_of_parks(px, py, away):
        # (pushed away from the ground until it is off the car parks and the ring road)
        for _ in range(40):
            x0, x1, y0, y1 = px - 35, px + 35, py - 22, py + 22
            hit = any(p[0] - 6 < x1 and x0 < p[1] + 6 and p[2] - 6 < y1 and y0 < p[3] + 6 for p in parks)
            ring = abs(px) - 35 < hx + 8 and abs(py - cy) - 22 < hy + 8
            if not hit and not ring:
                return (x0, x1, y0, y1)
            px, py = px + away[0] * 4, py + away[1] * 4
        return (x0, x1, y0, y1)
    spots_t = [ground_at(0.16, 0.6), ground_at(0.84, 0.4)]
    tpitches = [clear_of_parks(x, y, (math.copysign(1, x), 0)) for x, y in spots_t]
    tpitches = [[], tpitches[1:], tpitches, tpitches][lv]
    turf, lines = bmesh.new(), bmesh.new()
    for x0, x1, y0, y1 in tpitches:
        add_box(turf, (x0 - 3, y0 - 3, g), (x1 + 3, y1 + 3, g + 0.06))
        for a, b in [((x0, y0), (x1, y0 + 0.3)), ((x0, y1 - 0.3), (x1, y1)), ((x0, y0), (x0 + 0.3, y1)), ((x1 - 0.3, y0), (x1, y1)), (((x0 + x1) / 2 - 0.15, y0), ((x0 + x1) / 2 + 0.15, y1))]:
            add_box(lines, (*a, g + 0.06), (*b, g + 0.09))
        for s in (x0, x1 - 0.4):  # the goals
            add_box(lines, (s, (y0 + y1) / 2 - 3.6, g), (s + 0.4, (y0 + y1) / 2 + 3.6, g + 2.4))
        n = 32
        mx, my = (x0 + x1) / 2, (y0 + y1) / 2
        for k in range(n):
            a0 = 2 * math.pi * k / n
            add_box(lines, (mx + 7 * math.cos(a0) - 0.2, my + 7 * math.sin(a0) - 0.2, g + 0.06), (mx + 7 * math.cos(a0) + 0.2, my + 7 * math.sin(a0) + 0.2, g + 0.09))
    # (the user: "the high fence and some more details"): a 5 m ball-stop fence of posts, rails
    # and see-through green netting, a gate; four light masts; a shelter with benches, a
    # changing hut and a path to it; mini goals, poles and a line of cones
    posts, net, kit, hut, path, cones = (bmesh.new() for _ in range(6))
    for x0, x1, y0, y1 in tpitches:
        fx0, fx1, fy0, fy1, fh = x0 - 4, x1 + 4, y0 - 4, y1 + 4, 5.0
        for (ax, ay), (bx, by) in (((fx0, fy0), (fx1, fy0)), ((fx1, fy0), (fx1, fy1)), ((fx1, fy1), (fx0, fy1)), ((fx0, fy1), (fx0, fy0))):
            n = max(1, int(math.hypot(bx - ax, by - ay) / 4))
            for k in range(n + 1):
                px, py = ax + (bx - ax) * k / n, ay + (by - ay) * k / n
                add_box(posts, (px - 0.12, py - 0.12, g), (px + 0.12, py + 0.12, g + fh))
            for z in (1.2, fh - 0.1):
                strut(posts, (ax, ay, g + z), (bx, by, g + z), 0.1)
            gate = (ax == fx0 and ay == fy0)  # (a gap on the near side: the way in)
            mx = (ax + bx) / 2
            if gate:
                add_box(net, (ax, ay - 0.03, g + 0.05), (mx - 2, ay + 0.03, g + fh))
                add_box(net, (mx + 2, ay - 0.03, g + 0.05), (bx, ay + 0.03, g + fh))
            else:
                lo, hi = (min(ax, bx), min(ay, by)), (max(ax, bx), max(ay, by))
                add_box(net, (lo[0] - 0.03, lo[1] - 0.03, g + 0.05), (hi[0] + 0.03, hi[1] + 0.03, g + fh))
        for cx_, cy_ in ((fx0, fy0), (fx1, fy0), (fx1, fy1), (fx0, fy1)):  # light masts
            add_box(posts, (cx_ - 0.25, cy_ - 0.25, g), (cx_ + 0.25, cy_ + 0.25, g + 12))
            add_box(kit, (cx_ - 1.0, cy_ - 1.0, g + 12), (cx_ + 1.0, cy_ + 1.0, g + 13.2))
        sy = y1 + 1.2  # the shelter on the far long side, benches under it
        add_box(hut, ((x0 + x1) / 2 - 6, sy, g), ((x0 + x1) / 2 + 6, sy + 1.8, g + 2.4))
        add_box(kit, ((x0 + x1) / 2 - 5.5, sy + 0.2, g + 0.4), ((x0 + x1) / 2 + 5.5, sy + 1.0, g + 0.9))
        hx_ = x1 + 10 if x0 > 0 else x0 - 22  # the changing hut beside the pitch, a path to the gate
        add_box(hut, (hx_, y0 + 2, g), (hx_ + 12, y0 + 12, g + 3.6))
        add_box(kit, (hx_ - 0.4, y0 + 1.6, g + 3.6), (hx_ + 12.4, y0 + 12.4, g + 4.1))
        add_box(path, (min(hx_ + 6, (fx0 + fx1) / 2), fy0 - 3, g), (max(hx_ + 6, (fx0 + fx1) / 2), fy0 - 1, g + 0.07))
        add_box(path, (hx_ + 5, fy0 - 3, g), (hx_ + 7, y0 + 2, g + 0.07))
        for gx in (x0 + 18, x1 - 18):  # mini goals across the pitch
            add_box(lines, (gx - 0.1, (y0 + y1) / 2 - 12, g), (gx + 0.1, (y0 + y1) / 2 - 10, g + 1.2))
            add_box(lines, (gx - 0.1, (y0 + y1) / 2 + 10, g), (gx + 0.1, (y0 + y1) / 2 + 12, g + 1.2))
        for k in range(8):  # a line of cones and two rows of poles for the drills
            cx_ = x0 + 8 + k * 2.5
            bmesh.ops.create_cone(cones, cap_ends=True, segments=8, radius1=0.35, radius2=0.05, depth=0.6, matrix=Matrix.Translation((cx_, y0 + 6, g + 0.36)))
            add_box(cones, (x1 - 8 - k * 2.5 - 0.06, y1 - 6 - 0.06, g), (x1 - 8 - k * 2.5 + 0.06, y1 - 6 + 0.06, g + 1.6))
    tm = grass_mat()  # (mown stripes, as the main pitch)
    obj("trainingturf", turf, tm, coll)
    obj("traininglines", lines, mat("Lines", (0.95, 0.95, 0.95), 0.6), coll)
    obj("fenceposts", posts, mat("FencePost", (0.12, 0.13, 0.12), 0.5, 0.5), coll)
    netting = mat("Netting", (0.05, 0.12, 0.06), 0.8)
    netting.node_tree.nodes["Principled BSDF"].inputs["Alpha"].default_value = 0.4
    obj("netting", net, netting, coll)
    obj("trainingkit", kit, mat("Kit", (0.15, 0.16, 0.18), 0.5), coll)
    obj("traininghut", hut, mat("Hut", (0.55, 0.52, 0.47), 0.8), coll)
    obj("trainingpath", path, mat("Path2", (0.22, 0.2, 0.17), 0.9), coll)
    obj("cones", cones, mat("Cone", (0.95, 0.45, 0.05), 0.6), coll)
    # low buildings beyond the main road and north of the ring road; a pub by the east car park
    walls = [mat(f"House{k}", c, 0.8) for k, c in enumerate([(0.22, 0.12, 0.08), (0.3, 0.28, 0.25), (0.16, 0.17, 0.19)])]
    houses, roofs, lit = [bmesh.new() for _ in range(3)], bmesh.new(), bmesh.new()
    rows = [(main - 22, -1), (ring_n + 22, 1)]
    for y, s in rows:
        x = -reach * 1.2
        while x < reach * 1.2:
            w, d, hgt = rnd.uniform(9, 16), rnd.uniform(8, 12), rnd.uniform(5, 11)
            yy = y + s * d / 2
            if not any(p[0] - 6 < x + w and x < p[1] + 6 and p[2] - 6 < yy < p[3] + 6 for p in parks + tpitches):
                add_box(houses[rnd.randrange(3)], (x, yy - d / 2, g), (x + w, yy + d / 2, g + hgt))
                add_box(roofs, (x - 0.4, yy - d / 2 - 0.4, g + hgt), (x + w + 0.4, yy + d / 2 + 0.4, g + hgt + 0.6))
                for fl in range(int(hgt // 3)):
                    if rnd.random() < 0.5:
                        add_box(lit, (x + 1, yy - s * (d / 2 + 0.05), g + 1.2 + fl * 3), (x + w - 1, yy - s * (d / 2 - 0.1), g + 2.2 + fl * 3))
            x += w + rnd.uniform(3, 9)
    px0, _, py0, _ = parks[0]
    add_box(houses[0], (px0 + 4, py0 - 20, g), (px0 + 24, py0 - 6, g + 6))  # the pub
    add_box(roofs, (px0 + 3.5, py0 - 20.5, g + 6), (px0 + 24.5, py0 - 5.5, g + 6.8))
    add_box(lit, (px0 + 6, py0 - 6.05, g + 1), (px0 + 22, py0 - 5.9, g + 3.5))
    for k in range(3):
        obj(f"houses{k}", houses[k], walls[k], coll)
    obj("houseroofs", roofs, mat("HouseRoof", (0.07, 0.07, 0.08), 0.7), coll)
    obj("windows", lit, mat("Windows", (1.0, 0.8, 0.5), 0.2, 0.0, 1.0), coll)
    # trees in the grass: rows along the ring road, many more in the fields; never on a road,
    # a car park, a building or the plinth
    crowns, trunks = [bmesh.new() for _ in range(3)], bmesh.new()
    def free(x, y):
        if box[0] - 9 < x < box[1] + 9 and box[2] - 9 < y < box[3] + 9:
            return False
        if abs(y - main) < 10 or any(p[0] - 3 < x < p[1] + 3 and p[2] - 3 < y < p[3] + 3 for p in parks):
            return False
        if abs(y - (main - 22)) < 16 or abs(y - (ring_n + 22)) < 16:
            return False
        if abs(x) < hx + 7 and abs(y - cy) < hy + 7 and not (abs(x) < hx - 7 and abs(y - cy) < hy - 7):
            return False  # (the ring road)
        if any(p[0] - 8 < x < p[1] + 8 and p[2] - 8 < y < p[3] + 8 for p in tpitches):
            return False
        return not any(abs(x - rx) < 7 and main < y < ring_s + 3 for rx in (-30.0, 34.0))
    placed = []
    for k in range(2800):
        x, y = rnd.uniform(-reach * 1.4, reach * 1.4), rnd.uniform(-reach * 1.4, reach * 1.4)
        if not free(x, y) or any((x - a) ** 2 + (y - b) ** 2 < 20 for a, b in placed[-60:]):
            continue
        placed.append((x, y))
        r, h = rnd.uniform(2.2, 3.8), rnd.uniform(2.5, 4.5)
        add_box(trunks, (x - 0.3, y - 0.3, g), (x + 0.3, y + 0.3, g + h))
        bmesh.ops.create_icosphere(crowns[rnd.randrange(3)], subdivisions=2, radius=r, matrix=Matrix.Translation((x, y, g + h + r * 0.8)))
    obj("fieldtrunks", trunks, mat("Bark", (0.1, 0.07, 0.045), 0.9), coll)
    for k, c in enumerate([(0.025, 0.08, 0.022), (0.04, 0.11, 0.03), (0.022, 0.065, 0.03)]):
        obj(f"fieldtrees{k}", crowns[k], mat(f"FieldLeaves{k}", c, 0.85), coll)


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
    global LEVEL
    LEVEL = "smlx".index(size)  # (the town's size, `town`)
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
    cam_data.ortho_scale = max(2 * half_w, 2 * half_h * COLUMN_W / H) * 1.06 * W / COLUMN_W
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
    text.color = id_colour(1)  # (the club's text colour on its main one, the band: class 3)
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
