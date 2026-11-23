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
ROOF_FRONT = 0.45  # the roofs reach this far from the front (share of the stand's depth): the mock's seats show
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
        mix.inputs[6].default_value = (*[c * 1.12 for c in color], 1)
        mix.inputs[7].default_value = (*[c * 0.8 for c in color], 1)
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
    floors = []
    for axis, size in ((0, PL / 10), (1, PW / 6)):
        div = nt.nodes.new("ShaderNodeMath"); div.operation = "DIVIDE"; div.inputs[1].default_value = size
        fl = nt.nodes.new("ShaderNodeMath"); fl.operation = "FLOOR"
        l.new(sep.outputs[axis], div.inputs[0])
        l.new(div.outputs[0], fl.inputs[0])
        floors.append(fl)
    add = nt.nodes.new("ShaderNodeMath"); add.operation = "ADD"
    l.new(floors[0].outputs[0], add.inputs[0]); l.new(floors[1].outputs[0], add.inputs[1])
    mod = nt.nodes.new("ShaderNodeMath"); mod.operation = "FLOORED_MODULO"; mod.inputs[1].default_value = 2
    l.new(add.outputs[0], mod.inputs[0])
    mix = nt.nodes.new("ShaderNodeMix"); mix.data_type = "RGBA"
    mix.inputs[6].default_value = (0.05, 0.25, 0.045, 1)
    mix.inputs[7].default_value = (0.08, 0.35, 0.065, 1)
    l.new(mod.outputs[0], mix.inputs[0])
    l.new(mix.outputs[2], p.inputs["Base Color"])
    return m


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
    concrete = mat("Concrete", (0.16, 0.165, 0.17), 0.9)
    facade = mat("Facade", (0.075, 0.08, 0.09), 0.6)
    trim = mat("Trim", (0.85, 0.85, 0.85), 0.4)
    metal = mat("Roof", (0.17, 0.18, 0.19), 0.45, 0.5)
    dark = mat("Fascia", (0.08, 0.085, 0.095), 0.5)
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
        obj(f"ribs{side}", bm, concrete, base)
        # (lit from inside: the mock's evening, a warm band along each stand's back)
        obj(f"glass{side}", gl, mat("Concourse", (1.0, 0.78, 0.45), 0.15, 0.0, 1.6), base)
        bm = bmesh.new()
        add_box(bm, w(-length / 2 + 4, -GAP + 2.0, 0), w(length / 2 - 4, -GAP + 2.3, 0.95))
        obj(f"boards{side}", bm, trim, base, id_colour(3))
        # the roof: from behind the facade over two thirds of the rows, rising toward the pitch
        bm = bmesh.new()
        front = depth * ROOF_FRONT
        for k in range(8):
            u0 = depth + 1.6 - k * (depth + 1.6 - front) / 8
            u1 = depth + 1.6 - (k + 1) * (depth + 1.6 - front) / 8
            z = top + 5.0 + k * 0.35
            add_box(bm, w(-length / 2 - 1, u1, z), w(length / 2 + 1, u0, z + 0.5))
            # corrugation: a low rib every 2.5 m down the slope
            a = -length / 2 - 0.5
            while a < length / 2 + 0.5:
                add_box(bm, w(a, u1, z + 0.5), w(a + 0.3, u0, z + 0.68))
                a += 2.5
        obj(f"roof{side}", bm, metal, roof_c)
        bm, edge = bmesh.new(), bmesh.new()
        add_box(edge, w(-length / 2 - 1, front - 0.4, top + 5.0 + 8 * 0.35 - 0.6), w(length / 2 + 1, front, top + 5.0 + 8 * 0.35 + 0.9))
        for k in range(9):  # columns at the back
            a = -length / 2 + k * length / 8
            add_box(bm, w(a - 0.3, depth + 1.4, top + 2.9), w(a + 0.3, depth + 1.9, top + 5.5))
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
    add_box(bm, (-PL / 2 - GAP + 0.5, -PW / 2 - GAP + 0.5, -0.1), (PL / 2 + GAP - 0.5, PW / 2 + GAP - 0.5, 0.02))
    obj("pitch", bm, grass_mat(), coll)
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


def surroundings(coll, ex, ey, top, reach):
    rnd = random.Random(7)
    bm = bmesh.new()
    add_box(bm, (-reach * 3, -reach * 3, -0.4), (reach * 3, reach * 3, -0.2))
    obj("ground", bm, mat("Field", (0.012, 0.028, 0.014), 0.95), coll)
    px, py = ex + 12, ey + 12  # the plaza round the ground
    bm = bmesh.new()
    add_box(bm, (-px, -py, -0.2), (px, py, -0.05))
    obj("plaza", bm, mat("Plaza", (0.045, 0.047, 0.052), 0.9), coll)
    road = mat("Asphalt", (0.022, 0.023, 0.026), 0.85)
    bm = bmesh.new()
    rx, ry = px + 9, py + 9
    # the ring road with rounded corners (the mock's curving road), a dashed white middle line
    dashes = bmesh.new()
    rad = 22.0
    def ring_point(t):
        """A point along the rounded rectangle (half sides rx - 4, ry - 4, corner rad), t in 0..1."""
        hx, hy = rx - 4 - rad, ry - 4 - rad
        straight = [2 * hx, 2 * hy, 2 * hx, 2 * hy]
        arc = math.pi * rad / 2
        total = sum(straight) + 4 * arc
        d = t * total
        corners = [(hx, -hy, -math.pi / 2), (hx, hy, 0.0), (-hx, hy, math.pi / 2), (-hx, -hy, math.pi)]
        starts = [(-hx, -(hy + rad), 1, 0), (hx + rad, -hy, 0, 1), (hx, hy + rad, -1, 0), (-(hx + rad), hy, 0, -1)]
        for k in range(4):
            sx, sy, dx, dy = starts[k]
            if d <= straight[k]:
                return sx + dx * d, sy + dy * d
            d -= straight[k]
            if d <= arc:
                cx, cy, a0 = corners[k]
                a = a0 + d / rad
                return cx + rad * math.cos(a), cy + rad * math.sin(a)
            d -= arc
        return starts[0][0], starts[0][1]
    steps = 220
    for k in range(steps):
        x0, y0 = ring_point(k / steps)
        x1, y1 = ring_point((k + 1) / steps)
        mx, my = (x0 + x1) / 2, (y0 + y1) / 2
        ang = math.atan2(y1 - y0, x1 - x0)
        seg = math.hypot(x1 - x0, y1 - y0) + 0.6
        m = Matrix.Translation((mx, my, -0.11)) @ Matrix.Rotation(ang, 4, "Z") @ Matrix.Diagonal((seg, 8.0, 0.14, 1.0))
        bmesh.ops.create_cube(bm, size=1.0, matrix=m)
        if k % 3 == 0:
            m = Matrix.Translation((mx, my, -0.035)) @ Matrix.Rotation(ang, 4, "Z") @ Matrix.Diagonal((seg * 1.4, 0.25, 0.02, 1.0))
            bmesh.ops.create_cube(dashes, size=1.0, matrix=m)
    add_box(bm, (-rx - 200, -ry - 24, -0.18), (rx + 200, -ry - 14, -0.04))  # the main road in front
    x = -rx - 200.0
    while x < rx + 200:
        add_box(dashes, (x, -ry - 19.12, -0.04), (x + 3.0, -ry - 18.88, -0.02))
        x += 7.0
    add_box(bm, (-4, -ry - 16, -0.18), (4, -ry + 4, -0.04))  # in from the main road
    # the car park east of the ground
    cx0, cx1, cy0, cy1 = rx + 6, rx + 70, -ry + 4, ry - 20
    add_box(bm, (cx0, cy0, -0.18), (cx1, cy1, -0.04))
    add_box(bm, (rx, -6, -0.18), (cx0, 6, -0.04))
    obj("roads", bm, road, coll)
    obj("roadlines", dashes, mat("Paint", (0.8, 0.8, 0.8), 0.7), coll)
    bm = bmesh.new()
    cars = [bmesh.new() for _ in range(5)]
    for col in range(3):
        x = cx0 + 6 + col * 21
        y = cy0 + 3
        while y < cy1 - 3:
            add_box(bm, (x - 0.08, y, -0.03), (x + 5.2, y + 0.12, -0.01))
            add_box(bm, (x + 9.8, y, -0.03), (x + 15, y + 0.12, -0.01))
            for xx in (x + 0.4, x + 10.2):
                if rnd.random() < 0.7:
                    c = cars[rnd.randrange(5)]
                    add_box(c, (xx, y + 0.45, 0), (xx + 4.4, y + 2.25, 1.0))
                    add_box(c, (xx + 1.1, y + 0.6, 1.0), (xx + 3.4, y + 2.1, 1.5))
            y += 2.7
    obj("bays", bm, mat("Paint", (0.8, 0.8, 0.8), 0.7), coll)
    for k, c in enumerate([(0.6, 0.6, 0.62), (0.08, 0.08, 0.09), (0.45, 0.05, 0.04), (0.06, 0.12, 0.35), (0.85, 0.85, 0.85)]):
        obj(f"cars{k}", cars[k], mat(f"Car{k}", c, 0.3, 0.4), coll)
    # buildings on the plaza in front: a restaurant, the club shop (its roof in club colour), kiosks
    walls = mat("Walls", (0.16, 0.16, 0.17), 0.8)
    glass = mat("Glass", (1.0, 0.8, 0.5), 0.1, 0.0, 1.2)  # (shop and restaurant windows lit)
    bm, gm, tm = bmesh.new(), bmesh.new(), bmesh.new()
    def house(x0, y0, x1, y1, h, club=False):
        add_box(bm, (x0, y0, 0), (x1, y1, h))
        add_box(gm, (x0 + 0.6, y0 - 0.1, 0.5), (x1 - 0.6, y0 + 0.2, h - 0.8))
        add_box(tm if club else bm, (x0 - 0.5, y0 - 0.5, h), (x1 + 0.5, y1 + 0.5, h + 0.7))
    house(-38, -py + 1.5, -18, -py + 9, 4.5)
    for x in (-8, -3, 2, 7):
        house(x, -py + 2.5, x + 3, -py + 5, 2.6, club=(x % 2 == 0))
    # the corners between the stands: low blocks with a band of glass, so the ground reads as one
    # bowl (the mock's corners are built up too)
    cxa, cya = (PL + 8) / 2 + 0.9, (PW + 4) / 2 + 0.9
    tops = bmesh.new()
    for sx in (-1, 1):
        for sy in (-1, 1):
            xa, xb = sorted((sx * cxa, sx * (ex - 1.0)))
            ya, yb = sorted((sy * cya, sy * (ey - 1.0)))
            if xb - xa > 2 and yb - ya > 2:
                add_box(bm, (xa, ya, 0), (xb, yb, top * 0.62 - 0.4))
                add_box(tops, (xa, ya, top * 0.62 - 0.4), (xb, yb, top * 0.62))
                add_box(gm, (xa - 0.1, ya - 0.1, top * 0.3), (xb + 0.1, yb + 0.1, top * 0.3 + 1.8))
    obj("buildings", bm, walls, coll)
    obj("cornertops", tops, mat("CornerTop", (0.06, 0.065, 0.07), 0.7), coll)
    obj("windows", gm, glass, coll)
    obj("clubroofs", tm, mat("Trim", (0.85, 0.85, 0.85)), coll, id_colour(3))
    # floodlights in the corners
    pole, lamp = bmesh.new(), bmesh.new()
    for sx in (-1, 1):
        for sy in (-1, 1):
            x, y = sx * (ex + 2), sy * (ey + 2)
            hgt = top + 26
            add_box(pole, (x - 0.45, y - 0.45, 0), (x + 0.45, y + 0.45, hgt))
            # a lamp bank tilted at the centre spot, and a spot light for the evening glow
            turn = math.atan2(-y, -x)
            # a dark frame with two rows of four lamps
            head = Matrix.Translation((x, y, hgt + 1.5)) @ Matrix.Rotation(turn, 4, "Z") @ Matrix.Rotation(math.radians(-35), 4, "Y")
            bmesh.ops.create_cube(pole, size=1.0, matrix=head @ Matrix.Diagonal((0.5, 7.6, 4.4, 1.0)))
            for row in (-1, 1):
                for col in range(4):
                    off = Matrix.Translation((0.3, -2.7 + col * 1.8, row * 1.05))
                    bmesh.ops.create_cube(lamp, size=1.0, matrix=head @ off @ Matrix.Diagonal((0.3, 1.4, 1.5, 1.0)))
            spot_data = bpy.data.lights.new(f"flood{sx}{sy}", "SPOT")
            spot_data.energy = 300000
            spot_data.spot_size = math.radians(70)
            spot_data.spot_blend = 0.6
            spot_data.color = (1.0, 0.93, 0.8)
            spot = bpy.data.objects.new(f"flood{sx}{sy}", spot_data)
            spot.location = (x, y, hgt + 1)
            spot.rotation_euler = (Vector((0, 0, 0)) - Vector((x, y, hgt))).to_track_quat("-Z", "Y").to_euler()
            coll.objects.link(spot)
    obj("poles", pole, mat("Pole", (0.5, 0.5, 0.52), 0.4, 0.7), coll)
    obj("lamps", lamp, mat("Lamp", (1.0, 0.96, 0.85), 0.3, 0.0, 12.0), coll)
    # trees: rows along the ring road, more in the fields, never on a road or the car park
    crowns = [bmesh.new() for _ in range(3)]
    trunks = bmesh.new()
    spots = []
    for k in range(int((rx + ry) * 0.9)):
        spots += [(rnd.uniform(-rx, rx), -ry - 5), (rnd.uniform(-rx, rx), ry + 5), (-rx - 5, rnd.uniform(-ry, ry))]
    for k in range(900):
        spots.append((rnd.uniform(-reach * 1.6, reach * 1.6), rnd.uniform(-reach * 1.6, reach * 1.6)))
    placed = []
    for x, y in spots:
        x, y = x + rnd.uniform(-1.5, 1.5), y + rnd.uniform(-1.5, 1.5)
        if abs(x) < rx + 2 and abs(y) < ry + 2:
            continue
        if cx0 - 2 < x < cx1 + 2 and cy0 - 2 < y < cy1 + 2:
            continue
        if -ry - 26 < y < -ry - 12:
            continue
        if any((x - a) ** 2 + (y - b) ** 2 < 16 for a, b in placed):
            continue
        placed.append((x, y))
        r = rnd.uniform(2.0, 3.6)
        h = rnd.uniform(2.5, 4.5)
        add_box(trunks, (x - 0.3, y - 0.3, 0), (x + 0.3, y + 0.3, h))
        bmesh.ops.create_icosphere(crowns[rnd.randrange(3)], subdivisions=2, radius=r, matrix=Matrix.Translation((x, y, h + r * 0.8)))
    obj("trunks", trunks, mat("Bark", (0.12, 0.08, 0.05), 0.9), coll)
    for k, c in enumerate([(0.02, 0.07, 0.025), (0.035, 0.09, 0.03), (0.018, 0.055, 0.03)]):
        obj(f"trees{k}", crowns[k], mat(f"Leaves{k}", c, 0.85), coll)


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
    shop_ex = PL / 2 + GAP + max(r[1] for r in profile(SIZES[size])) + 2.5
    club_shop(colls[3], shop_ex)
    club_shop(colls[4], shop_ex, mega=True)
    ex, ey = PL / 2 + GAP + depth + 2.5, PW / 2 + GAP + depth + 2.5
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
    sun_data.energy = 2.6
    sun_data.angle = math.radians(3)
    sun = bpy.data.objects.new("sun", sun_data)
    sun.rotation_euler = SUN.to_track_quat("Z", "Y").to_euler()
    sc.collection.objects.link(sun)
    world = bpy.data.worlds.new("w")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs[0].default_value = (0.22, 0.26, 0.34, 1)
    world.node_tree.nodes["Background"].inputs[1].default_value = 0.45
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


def club_shop(coll, ex, mega=False):
    """The club shop (user 2026-11-23, after a picture of one): a big block on the plaza at the west
    end, the end nearest the camera, its long glass front and CLUB SHOP towards it, fans at the
    door. Its own layer: drawn once the club has a shop (every size: a new stadium keeps it)."""
    # (the megastore: longer, two storeys, "MEGASTORE"; its own layer, drawn instead)
    x0, x1 = (-(ex + 15.0), -(ex + 2.0)) if mega else (-(ex + 13.0), -(ex + 2.0))
    y0, y1, h = (-30.0, 30.0, 16.0) if mega else (-18.0, 18.0, 12.0)
    gtop, band = h * 0.63, (h * 0.68, h * 0.97)
    bm, glass, rim, posts, sign_band, people = (bmesh.new() for _ in range(6))
    add_box(bm, (x0, y0, 0), (x1, y1, h))
    # a thin light rim round the dark roof
    for a, b in [((x0 - 0.3, y0 - 0.3), (x1 + 0.3, y0 + 0.3)), ((x0 - 0.3, y1 - 0.3), (x1 + 0.3, y1 + 0.3)),
                 ((x0 - 0.3, y0 - 0.3), (x0 + 0.3, y1 + 0.3)), ((x1 - 0.3, y0 - 0.3), (x1 + 0.3, y1 + 0.3))]:
        add_box(rim, (*a, h), (*b, h + 0.5))
    # the shop windows (west front, south end), lit warm from inside, posts every 3 m
    add_box(glass, (x0 - 0.12, y0 + 0.8, 0.2), (x0 + 0.1, y1 - 0.8, gtop))
    add_box(glass, (x0 + 0.8, y0 - 0.12, 0.2), (x1 - 0.8, y0 + 0.1, gtop))
    y = y0 + 0.8
    while y < y1 - 0.8:
        add_box(posts, (x0 - 0.3, y - 0.15, 0), (x0 + 0.05, y + 0.15, gtop + 0.4))
        y += 3.0
    add_box(posts, (x0 - 0.3, y0 + 0.5, gtop), (x0 + 0.05, y1 - 0.5, gtop + 0.5))
    # the dark band for the sign over the front
    add_box(sign_band, (x0 - 0.3, y0 + 0.5, band[0]), (x0 + 0.05, y1 - 0.5, band[1]))
    obj("shopblock", bm, mat("ShopWalls", (0.045, 0.048, 0.055), 0.7), coll)
    obj("shoprim", rim, mat("ShopRim", (0.55, 0.57, 0.6), 0.4, 0.3), coll)
    obj("shopposts", posts, mat("ShopPosts", (0.03, 0.03, 0.035), 0.5), coll)
    obj("shopband", sign_band, mat("ShopBand", (0.015, 0.015, 0.02), 0.5), coll)
    obj("shopglass", glass, mat("ShopGlass", (1.0, 0.72, 0.38), 0.1, 0.0, 0.9), coll)
    # CLUB SHOP on the band, facing west (the camera's side)
    cu = bpy.data.curves.new("shopsign", "FONT")
    cu.body = "MEGASTORE" if mega else "CLUB SHOP"
    cu.size = 2.6
    cu.align_x = "CENTER"
    cu.align_y = "CENTER"
    cu.extrude = 0.05
    text = bpy.data.objects.new("shopsign", cu)
    text.location = (x0 - 0.4, 0.0, (band[0] + band[1]) / 2)
    text.rotation_euler = (math.radians(90), 0, math.radians(-90))
    text.data.materials.append(mat("ShopSign", (1.0, 1.0, 1.0), 0.3, 0.0, 4.0))
    coll.objects.link(text)
    # fans on the square in front of it
    rnd = random.Random(11)
    for k in range(34):
        x, y = rnd.uniform(x0 - 8, x0 - 1.5), rnd.uniform(y0 - 2, y1 + 2)
        add_box(people, (x - 0.25, y - 0.25, 0), (x + 0.25, y + 0.25, 1.75))
    obj("shoppeople", people, mat("People", (0.07, 0.08, 0.12), 0.8), coll)


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
        show(sc, colls, "hold", "off", "off", "off", "on"); render(sc, f("mega"))
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
        entries.append((f"shop_{size}", points, packed(pixels(f("shop")), crop=True)))
        entries.append((f"mega_{size}", points, packed(pixels(f("mega")), crop=True)))
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
