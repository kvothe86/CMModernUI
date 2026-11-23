"""Merchandise icons for the club shop card (user 2026-11-23, after a picture of a shirt, a scarf,
a mug and a jacket in green and white, and a white lock): Blender 5.x, no window:
    blender -b --factory-startup -P build_merch.py -- <out folder>
-> shirt.png, scarf.png, mug.png, jacket.png, lock.png (512x512, see-through background).
MERCH_ONLY=shirt,mug renders some of them.

Shirt, jacket and scarf are flat-lay pieces: a silhouette, filled in numpy, raised like a cushion
(a height field from the distance to its edge, a few soft folds), painted with a texture drawn in
the same frame. The mug is a lathe with a crest decal. The lock is a flat white glyph.
"""
import bpy, bmesh, math, sys, os
import numpy as np
from mathutils import Vector

OUT = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else os.path.join(os.path.dirname(os.path.abspath(__file__)), "merch")
SIZE = 512
N = 300  # height-field grid
T = 1024  # texture
GREEN = (0.085, 0.225, 0.14)  # sRGB, the reference's bottle green
LIGHT_GREEN = (0.098, 0.242, 0.152)
WHITE = (0.95, 0.95, 0.93)


# ---------------------------------------------------------------- 2D masks (frame [0,1]^2, y up)
def grid(n):
    y, x = (np.mgrid[0:n, 0:n] + 0.5) / n
    return x, y


def fill(poly, n):
    """The even-odd inside of a polygon on an n x n grid."""
    x, y = grid(n)
    inside = np.zeros((n, n), bool)
    for i in range(len(poly)):
        (x0, y0), (x1, y1) = poly[i], poly[(i + 1) % len(poly)]
        cross = (y0 > y) != (y1 > y)
        xi = x0 + (y - y0) * (x1 - x0) / ((y1 - y0) or 1e-9)
        inside ^= cross & (x < xi)
    return inside


def disc(cx, cy, r, n):
    x, y = grid(n)
    return (x - cx) ** 2 + (y - cy) ** 2 < r * r


def band(p0, p1, half, n):
    """The points within half of the segment p0-p1 (a stroke)."""
    x, y = grid(n)
    (x0, y0), (x1, y1) = p0, p1
    dx, dy = x1 - x0, y1 - y0
    t = np.clip(((x - x0) * dx + (y - y0) * dy) / (dx * dx + dy * dy), 0, 1)
    return (x - x0 - t * dx) ** 2 + (y - y0 - t * dy) ** 2 < half * half


def rounded(x0, y0, x1, y1, r, n):
    x, y = grid(n)
    qx = np.maximum(np.abs(x - (x0 + x1) / 2) - (x1 - x0) / 2 + r, 0)
    qy = np.maximum(np.abs(y - (y0 + y1) / 2) - (y1 - y0) / 2 + r, 0)
    return (np.hypot(qx, qy) < r) & (np.abs(x - (x0 + x1) / 2) < (x1 - x0) / 2) & (np.abs(y - (y0 + y1) / 2) < (y1 - y0) / 2)


def distance(mask):
    """Grid steps to the edge (alternating 4- and 8-neighbour erosion: near round)."""
    d = np.zeros(mask.shape, np.float32)
    cur, k = mask.copy(), 0
    while cur.any():
        d += cur
        nb = cur.copy()
        nb[1:] &= cur[:-1]; nb[:-1] &= cur[1:]; nb[:, 1:] &= cur[:, :-1]; nb[:, :-1] &= cur[:, 1:]
        if k % 2:
            nb[1:, 1:] &= cur[:-1, :-1]; nb[:-1, :-1] &= cur[1:, 1:]; nb[1:, :-1] &= cur[:-1, 1:]; nb[:-1, 1:] &= cur[1:, :-1]
        cur, k = nb, k + 1
    return d


def blur(a, times=2):
    for _ in range(times):
        a = (a + np.roll(a, 1, 0) + np.roll(a, -1, 0) + np.roll(a, 1, 1) + np.roll(a, -1, 1)) / 5
    return a


def cushion(mask, height, edge, folds=()):
    """Heights: a round edge of `edge` (share of the grid), flat beyond; folds (ax, ay, freq, amp)."""
    d = distance(mask) / (edge * mask.shape[0])
    t = np.clip(d, 0, 1)
    h = height * np.sqrt(1 - (1 - t) ** 2) + height * 0.35 * np.clip(d / 4, 0, 1)
    x, y = grid(mask.shape[0])
    for ax, ay, f, a in folds:
        h += a * height * t * np.sin((x * ax + y * ay) * f * 2 * math.pi)
    return blur(np.where(mask, h, 0.0), 3) * mask


# ---------------------------------------------------------------- textures
MODE = "ref"  # ref: green and white as the reference; neutral: greys; class: the club-colour mask


def role(col):
    """A reference colour's part for the game: 1 the club's first colour (the greens), 2 its second
    (the whites), 0 drawn as it is (zip, teeth); and the grey that keeps its light or dark."""
    lum = lambda c: 0.3 * c[0] + 0.6 * c[1] + 0.1 * c[2]
    if col[1] > col[0] * 1.3 and col[1] > col[2]:
        return 1, min(0.82 * lum(col) / lum(GREEN), 1.0)
    if min(col) > 0.6:
        return 2, min(0.82 * lum(col) / lum(WHITE), 1.0)
    return 0, None


def paint(layers, n=T):
    """An sRGB picture from (mask, colour) layers, the last on top; in the game's modes the greys
    or the class colours (red 1, green 2, blue 0) instead."""
    img = np.zeros((n, n, 4), np.float32)
    img[:, :, 3] = 1
    for mask, col in layers:
        if MODE != "ref":
            k, grey = role(col)
            if MODE == "class":
                col = ((1, 0, 0), (0, 1, 0), (0, 0, 1))[[1, 2, 0].index(k)]
            elif k:
                col = (grey,) * 3
        img[mask, :3] = col
    return img


def lattice(n, step, colour, width=0.0016):
    """The reference shirt's faint diagonal pattern: thin lines in two directions."""
    x, y = grid(n)
    a = np.abs(((x + y) / step) % 1 - 0.5) * step < width
    b = np.abs(((x - y * 0.6) / step) % 1 - 0.5) * step < width
    return (a | b), colour


def crest(cx, cy, r, n, ink=WHITE, ground=GREEN):
    """A round club crest: a ring, a tree (crown, trunk) inside; no lettering."""
    x, y = grid(n)
    d = np.hypot(x - cx, y - cy)
    ring = (d < r) & (d > r * 0.84)
    inner = (d < r * 0.72) & (d > r * 0.66)
    crown = disc(cx, cy + r * 0.12, r * 0.3, n) | disc(cx - r * 0.2, cy + r * 0.02, r * 0.2, n) | disc(cx + r * 0.2, cy + r * 0.02, r * 0.2, n)
    trunk = (np.abs(x - cx) < r * 0.06) & (y > cy - r * 0.38) & (y < cy)
    roots = (np.abs(y - (cy - r * 0.38)) < r * 0.04) & (np.abs(x - cx) < r * 0.3)
    return [(d < r, ground), (ring | inner | crown | trunk | roots, ink)]


def image(name, px):
    img = bpy.data.images.new(name, px.shape[1], px.shape[0], alpha=True)
    img.colorspace_settings.name = "sRGB"
    img.pixels.foreach_set(px.ravel())
    img.pack()
    return img


# ---------------------------------------------------------------- scene
def scene(ortho=1.06):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    sc = bpy.context.scene
    r = sc.render
    r.engine = "BLENDER_EEVEE"
    r.resolution_x = r.resolution_y = SIZE
    r.film_transparent = True
    r.image_settings.file_format = "PNG"
    r.image_settings.color_mode = "RGBA"
    sc.eevee.taa_render_samples = 64
    for k, v in (("use_raytracing", True), ("use_shadows", True)):
        try:
            setattr(sc.eevee, k, v)
        except Exception:
            pass
    sc.view_settings.view_transform = "Standard" if MODE == "class" else "AgX"
    sc.view_settings.exposure = 0.0 if MODE == "class" else 0.1
    world = bpy.data.worlds.new("w")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs[0].default_value = (0.6, 0.62, 0.65, 1)
    world.node_tree.nodes["Background"].inputs[1].default_value = 0.35
    sc.world = world
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = ortho
    cam.location = (0, 0, 5)
    sc.collection.objects.link(cam)
    sc.camera = cam
    # a soft key from the top left, a fill from the right, a rim from behind
    for name, loc, power, size in (("key", (-2.2, 2.0, 3.2), 210, 2.5), ("fill", (2.5, -0.5, 2.0), 70, 3.0), ("rim", (0.5, 3.0, 0.8), 60, 2.0)):
        l = bpy.data.objects.new(name, bpy.data.lights.new(name, "AREA"))
        l.data.energy = power
        l.data.size = size
        l.location = loc
        l.rotation_euler = (Vector((0, 0, 0)) - Vector(loc)).to_track_quat("-Z", "Y").to_euler()
        sc.collection.objects.link(l)
    return sc, cam


def material(name, px, rough=0.8, sheen=0.0, spec=0.3):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    p = nt.nodes.get("Principled BSDF")
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = image(name, px)
    if MODE == "class":  # (flat: the mask's colours as they are, no light)
        em = nt.nodes.new("ShaderNodeEmission")
        tex.interpolation = "Closest"
        nt.links.new(tex.outputs["Color"], em.inputs["Color"])
        nt.links.new(em.outputs["Emission"], nt.nodes["Material Output"].inputs["Surface"])
        return m
    nt.links.new(tex.outputs["Color"], p.inputs["Base Color"])
    p.inputs["Roughness"].default_value = rough
    p.inputs["Specular IOR Level"].default_value = spec
    if sheen:
        p.inputs["Sheen Weight"].default_value = sheen
    # a fine weave: a little bump from noise, so the cloth is not plastic
    nz = nt.nodes.new("ShaderNodeTexNoise")
    nz.inputs["Scale"].default_value = 420.0
    nz.inputs["Detail"].default_value = 4.0
    bump = nt.nodes.new("ShaderNodeBump")
    bump.inputs["Strength"].default_value = 0.12
    bump.inputs["Distance"].default_value = 0.002
    nt.links.new(nz.outputs["Fac"], bump.inputs["Height"])
    nt.links.new(bump.outputs["Normal"], p.inputs["Normal"])
    return m


def heightfield(name, mask, h, m):
    """A mesh over the mask's grid cells, z = h, UVs = the frame."""
    n = mask.shape[0]
    ii, jj = np.nonzero(mask)
    index = -np.ones((n, n), np.int64)
    index[ii, jj] = np.arange(len(ii))
    verts = np.stack([(jj + 0.5) / n - 0.5, (ii + 0.5) / n - 0.5, h[ii, jj]], 1)
    a, b, c, d = index[:-1, :-1], index[:-1, 1:], index[1:, 1:], index[1:, :-1]
    ok = (a >= 0) & (b >= 0) & (c >= 0) & (d >= 0)
    faces = np.stack([a[ok], b[ok], c[ok], d[ok]], 1)
    me = bpy.data.meshes.new(name)
    me.from_pydata(verts.tolist(), [], faces.tolist())
    uv = me.uv_layers.new()
    loops = np.empty(len(me.loops), np.int64)
    me.loops.foreach_get("vertex_index", loops)
    uv.data.foreach_set("uv", (verts[loops, :2] + 0.5).ravel())
    me.shade_smooth()
    me.materials.append(m)
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    return o


def save(sc, name):
    sc.render.filepath = os.path.join(OUT, name + ".png") if MODE == "ref" else os.path.join(OUT, "pack", f"{name}_{MODE}.png")
    bpy.ops.render.render(write_still=True)


# ---------------------------------------------------------------- the pieces
SHIRT_HALF = [(0.405, 0.895), (0.30, 0.875), (0.215, 0.85), (0.11, 0.76), (0.04, 0.665), (0.135, 0.565), (0.205, 0.615), (0.255, 0.65), (0.27, 0.40), (0.275, 0.11), (0.5, 0.10)]
FRONT_NECK = [(0.5, 0.79), (0.455, 0.84)], [(0.545, 0.84)]  # a V
BACK_NECK = [(0.5, 0.878)], []  # round, high at the back


def shirt_layers(outline, at=lambda p: p, front=True):
    """A shirt's texture layers (green body, faint lattice, white collar, cuffs and piping, seams,
    the crest on the front), its points moved by `at` (the kit's smaller shirt)."""
    P = lambda pts: [at(p) for p in pts]
    tm = fill(P(outline), T)
    if front:
        collar = fill(P([(0.5, 0.79), (0.40, 0.895), (0.375, 0.89), (0.5, 0.755), (0.625, 0.89), (0.60, 0.895)]), T)
    else:
        collar = band(at((0.405, 0.89)), at((0.5, 0.874)), 0.009, T) | band(at((0.5, 0.874)), at((0.595, 0.89)), 0.009, T)
    cuffs = fill(P([(0.04, 0.665), (0.135, 0.565), (0.158, 0.588), (0.064, 0.69)]), T)
    cuffs |= fill(P([(1 - 0.04, 0.665), (1 - 0.135, 0.565), (1 - 0.158, 0.588), (1 - 0.064, 0.69)]), T)
    seams = band(at((0.215, 0.85)), at((0.255, 0.65)), 0.0025, T) | band(at((0.785, 0.85)), at((0.745, 0.65)), 0.0025, T)
    seams |= band(at((0.275, 0.125)), at((0.725, 0.125)), 0.0018, T)
    pat, pc = lattice(T, 0.07, LIGHT_GREEN)
    _, ty = grid(T)
    # (white piping from the collar over the shoulders, as the reference's shirt)
    piping = band(at((0.395, 0.882)), at((0.215, 0.838)), 0.005, T) | band(at((0.215, 0.838)), at((0.12, 0.752)), 0.005, T)
    piping |= band(at((0.605, 0.882)), at((0.785, 0.838)), 0.005, T) | band(at((0.785, 0.838)), at((0.88, 0.752)), 0.005, T)
    layers = [(tm, GREEN), (tm & pat & (ty < at((0, 0.76))[1]), pc), (seams, (0.07, 0.19, 0.115)), (collar | cuffs | (piping & tm), WHITE)]
    if front:
        c, r = at((0.625, 0.69)), (at((0.058, 0))[0] - at((0, 0))[0])
        layers += crest(c[0], c[1], r, T)
    return layers


def shirt_outline(neck):
    head, tail = neck
    return head + SHIRT_HALF + [(1 - x, y) for x, y in reversed(SHIRT_HALF)] + tail


def shirt():
    sc, _ = scene()
    outline = shirt_outline(FRONT_NECK)
    mask = fill(outline, N)
    h = cushion(mask, 0.03, 0.05, [(1.0, 0.25, 2.2, 0.08), (-0.4, 1.0, 1.6, 0.06)])
    heightfield("shirt", mask, h, material("Shirt", paint(shirt_layers(outline)), 0.75, 0.3))
    save(sc, "shirt")


def number():
    """The shirt's back with a name and a number (the shop's "Name and number")."""
    sc, _ = scene()
    outline = shirt_outline(BACK_NECK)
    mask = fill(outline, N)
    h = cushion(mask, 0.03, 0.05, [(1.0, 0.25, 2.2, 0.08), (-0.4, 1.0, 1.6, 0.06)])
    x, y = grid(T)
    name = np.zeros((T, T), bool)
    for i in range(7):  # (letters as blocks: no real name)
        x0 = 0.385 + i * 0.033
        name |= (x > x0) & (x < x0 + 0.024) & (y > 0.735) & (y < 0.77)
    one = band((0.42, 0.40), (0.42, 0.655), 0.021, T) | band((0.42, 0.655), (0.375, 0.61), 0.016, T)
    zero = (((x - 0.555) / 0.08) ** 2 + ((y - 0.528) / 0.135) ** 2 < 1) & ~((((x - 0.555) / 0.037) ** 2 + ((y - 0.528) / 0.092) ** 2) < 1)
    layers = shirt_layers(outline, front=False) + [((name | one | zero) & fill(outline, T), WHITE)]
    heightfield("number", mask, h, material("Number", paint(layers), 0.75, 0.3))
    save(sc, "number")


def kit():
    """A small kit: shirt over shorts (the shop's "Kids' kits")."""
    sc, _ = scene()
    at = lambda p: (0.5 + (p[0] - 0.5) * 0.66, 0.41 + p[1] * 0.58)
    outline = [at(p) for p in shirt_outline(FRONT_NECK)]
    shorts = [(0.315, 0.5), (0.685, 0.5), (0.725, 0.14), (0.535, 0.12), (0.5, 0.27), (0.465, 0.12), (0.275, 0.14)]
    top, bottom = fill(outline, N), fill(shorts, N)
    h = np.maximum(cushion(top, 0.028, 0.05, [(1.0, 0.25, 2.2, 0.06)]) + top * 0.004, cushion(bottom, 0.024, 0.05, [(0.3, 1.0, 2.0, 0.06)]))
    st = fill(shorts, T)
    side = (band((0.315, 0.5), (0.275, 0.14), 0.012, T) | band((0.685, 0.5), (0.725, 0.14), 0.012, T)) & st
    hem = (band((0.275, 0.15), (0.465, 0.13), 0.006, T) | band((0.535, 0.13), (0.725, 0.15), 0.006, T)) & st
    layers = [(st, WHITE), (side | hem, GREEN), (band((0.315, 0.485), (0.685, 0.485), 0.006, T) & st, (0.6, 0.6, 0.6))]
    layers += shirt_layers(shirt_outline(FRONT_NECK), at)
    heightfield("kit", top | bottom, h, material("Kit", paint(layers), 0.75, 0.3))
    save(sc, "kit")


def jacket():
    sc, _ = scene()
    half = [(0.5, 0.955), (0.415, 0.955), (0.405, 0.885), (0.255, 0.855), (0.175, 0.79), (0.115, 0.30), (0.105, 0.135), (0.215, 0.125), (0.228, 0.30), (0.272, 0.64), (0.285, 0.40), (0.288, 0.075), (0.5, 0.075)]
    outline = half + [(1 - x, y) for x, y in reversed(half[1:-1])]
    mask = fill(outline, N)
    x, y = grid(N)
    h = cushion(mask, 0.032, 0.045, [(1.0, 0.1, 2.6, 0.07), (0.2, 1.0, 1.8, 0.05)])
    h -= 0.006 * (np.abs(x - 0.5) < 0.006) * mask  # the zip's groove
    tm = fill(outline, T)
    tx, ty = grid(T)
    dark = (0.1, 0.25, 0.15)
    collar = tm & (ty > 0.885)
    trim = tm & (ty > 0.94)
    stripes = np.zeros((T, T), bool)
    for side in (0, 1):
        a, b = (0.175, 0.79), (0.112, 0.17)
        if side:
            a, b = (1 - a[0], a[1]), (1 - b[0], b[1])
        sgn = 1 if side == 0 else -1
        for off in (0.018, 0.04):
            stripes |= band((a[0] + sgn * off, a[1] - 0.02), (b[0] + sgn * off, b[1] + 0.01), 0.0065, T)
            # (and over the shoulder to the collar)
            c = (0.37 if side == 0 else 0.63, 0.868 - off * 0.7)
            stripes |= band(c, (a[0] + sgn * off, a[1] - 0.02), 0.0065, T)
    stripes &= tm & (ty < 0.885)
    cuffs = tm & (ty < 0.165) & ((tx < 0.25) | (tx > 0.75))
    waist = tm & (ty < 0.11) & (tx > 0.27) & (tx < 0.73)
    zipper = (np.abs(tx - 0.5) < 0.004) & tm
    teeth = (np.abs(tx - 0.5) < 0.0022) & tm & ((ty * 260).astype(int) % 2 == 0)
    pull = fill([(0.494, 0.94), (0.506, 0.94), (0.508, 0.90), (0.492, 0.90)], T)
    pockets = band((0.36, 0.36), (0.39, 0.23), 0.002, T) | band((0.64, 0.36), (0.61, 0.23), 0.002, T)
    seams = band((0.255, 0.855), (0.272, 0.64), 0.002, T) | band((0.745, 0.855), (0.728, 0.64), 0.002, T)
    pat, pc = lattice(T, 0.09, (0.15, 0.34, 0.21), 0.0012)
    layers = [(tm, GREEN), (tm & pat, pc), (collar | cuffs | waist, dark), (seams | pockets, (0.08, 0.19, 0.12)),
              (stripes | trim, WHITE), (zipper, (0.07, 0.08, 0.08)), (teeth, (0.35, 0.36, 0.36)), (pull, (0.75, 0.76, 0.77))]
    layers += crest(0.63, 0.745, 0.05, T)
    heightfield("jacket", mask, h, material("Jacket", paint(layers), 0.55, 0.15, 0.4))
    save(sc, "jacket")


def scarf():
    sc, _ = scene()
    # two strips from a fold at the top, hanging apart (the reference's scarf), fringe at the ends
    top, length, width = (0.5, 0.85), 0.7, 0.2
    strips = []
    for ang in (-13, 13):
        a = math.radians(ang)
        d = np.array([math.sin(a), -math.cos(a)])  # down the strip
        nrm = np.array([-d[1], d[0]])
        o = np.array(top) + nrm * (np.sign(ang) * width * 0.45)
        strips.append((o, d, nrm))

    def coords(n):
        x, y = grid(n)
        return [((x - o[0]) * d[0] + (y - o[1]) * d[1], (x - o[0]) * nrm[0] + (y - o[1]) * nrm[1]) for o, d, nrm in strips]

    def scarf_mask(n, fringe):
        x, y = grid(n)
        # (the fold over the top: a rounded cap joining the two strips)
        m = ((x - 0.5) / (width * 0.98)) ** 2 + ((y - top[1] - 0.02) / 0.09) ** 2 < 1
        for s, t in coords(n):
            m |= (s > -0.03) & (s < length) & (np.abs(t) < width / 2)
            if fringe:
                m |= (s >= length) & (s < length + 0.075) & (np.abs(t) < width / 2 - 0.004) & ((((t + 1) / 0.0135) % 1) < 0.55)
        return m

    mask = scarf_mask(N, False)
    fr = scarf_mask(N, True) & ~mask
    h = cushion(mask, 0.022, 0.05, [(0.0, 1.0, 3.0, 0.1)]) + fr * 0.008
    # the texture: green and white blocks down each strip, knit ribs, the crest on the left one
    tm = scarf_mask(T, True)
    green, white, ribs = (np.zeros((T, T), bool) for _ in range(3))
    for s, t in coords(T):
        inside = (s > -0.03) & (s < length + 0.08) & (np.abs(t) < width / 2)
        u = s / length
        stripe = ((u > 0.12) & (u < 0.2)) | ((u > 0.27) & (u < 0.35)) | ((u > 0.78) & (u < 0.86)) | ((u > 0.92) & (u < 1.0))
        fringe_white = (u >= 1.0) & ((((t + 1) / 0.027) % 1) < 0.5)
        white |= inside & (stripe | fringe_white)
        green |= inside & ~(stripe | fringe_white)
        ribs |= inside & ((((t + 1) / 0.009) % 1) < 0.3)
    layers = [(tm, GREEN), (tm & white, WHITE), (tm & green & ribs, (0.11, 0.27, 0.165)), (tm & white & ribs, (0.84, 0.84, 0.82))]
    o, d, _ = strips[0]
    c = o + d * length * 0.56
    layers += crest(c[0], c[1], 0.068, T)
    heightfield("scarf", mask | fr, h, material("Scarf", paint(layers), 0.9, 0.6, 0.2))
    save(sc, "scarf")


def mug():
    sc, cam = scene()
    cam.data.type = "PERSP"
    cam.data.lens = 85
    sc.view_settings.exposure = 0.55  # (white china, as the reference's)
    cam.location = (0, -11.0, 4.2)
    cam.rotation_euler = (Vector((0.15, 0, 1.05)) - cam.location).to_track_quat("-Z", "Y").to_euler()
    # the body: a lathe of the profile (r, z), rounded at the rim and the foot
    prof = [(0.0, 0.0), (0.82, 0.0), (0.9, 0.015), (0.95, 0.06), (0.97, 0.14), (0.975, 1.9), (0.975, 2.08), (0.965, 2.13), (0.945, 2.15), (0.925, 2.13), (0.915, 2.08), (0.915, 0.32), (0.85, 0.24), (0.0, 0.22)]
    bm = bmesh.new()
    vs = [bm.verts.new((r, 0, z)) for r, z in prof]
    for a, b in zip(vs, vs[1:]):
        bm.edges.new((a, b))
    bmesh.ops.spin(bm, geom=bm.verts[:] + bm.edges[:], cent=(0, 0, 0), axis=(0, 0, 1), angle=math.radians(360), steps=128, use_duplicate=False)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-4)
    me = bpy.data.meshes.new("mug")
    bm.to_mesh(me)
    me.shade_smooth()
    ceramic = bpy.data.materials.new("Ceramic")
    ceramic.use_nodes = True
    p = ceramic.node_tree.nodes.get("Principled BSDF")
    p.inputs["Base Color"].default_value = (0.95, 0.95, 0.94, 1)
    p.inputs["Roughness"].default_value = 0.12
    p.inputs["Coat Weight"].default_value = 0.5
    me.materials.append(ceramic)
    sc.collection.objects.link(bpy.data.objects.new("mug", me))
    # the handle: a torus on the right, its inner half inside the body
    bpy.ops.mesh.primitive_torus_add(major_radius=0.5, minor_radius=0.1, major_segments=64, minor_segments=24, location=(0.98, 0, 1.12), rotation=(math.radians(90), 0, 0))
    handle = bpy.context.active_object
    handle.scale = (1.0, 1.25, 1.0)
    handle.data.materials.append(ceramic)
    handle.data.shade_smooth()
    # the crest on the front: a curved patch just outside the body, see-through round it
    n = T // 2
    x, y = grid(n)
    d = np.hypot(x - 0.5, y - 0.5)
    px = np.zeros((n, n, 4), np.float32)
    for mask, col in crest(0.5, 0.5, 0.36, n, GREEN, (0.96, 0.96, 0.95)) + [((d < 0.48) & (d > 0.455), GREEN)]:
        px[mask, :3] = col
        px[mask, 3] = 1
    dec = bpy.data.materials.new("Decal")
    dec.use_nodes = True
    nt = dec.node_tree
    p = nt.nodes.get("Principled BSDF")
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = image("decal", px)
    tex.extension = "CLIP"
    nt.links.new(tex.outputs["Color"], p.inputs["Base Color"])
    nt.links.new(tex.outputs["Alpha"], p.inputs["Alpha"])
    p.inputs["Roughness"].default_value = 0.15
    span, z0, z1, rr = math.radians(84), 0.42, 1.78, 0.979
    bm = bmesh.new()
    cols, rows = 48, 2
    g = [[bm.verts.new((rr * math.sin(-span / 2 + span * i / cols), -rr * math.cos(-span / 2 + span * i / cols), z0 + (z1 - z0) * j / rows)) for j in range(rows + 1)] for i in range(cols + 1)]
    uvl = bm.loops.layers.uv.new()
    aspect = span * rr / (z1 - z0)  # (the patch is wider than high: the picture kept square on it)
    for i in range(cols):
        for j in range(rows):
            f = bm.faces.new((g[i][j], g[i + 1][j], g[i + 1][j + 1], g[i][j + 1]))
            for loop, (a, b) in zip(f.loops, ((i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1))):
                loop[uvl].uv = (0.5 + (a / cols - 0.5) * aspect, b / rows)
    me = bpy.data.meshes.new("decal")
    bm.to_mesh(me)
    me.shade_smooth()
    me.materials.append(dec)
    sc.collection.objects.link(bpy.data.objects.new("decal", me))
    save(sc, "mug")


def lock_alpha(size):
    """The reference's lock, a white shackle and body with the keyhole cut out: its coverage per
    pixel (4x supersampled), rows bottom up."""
    n = size * 4
    x, y = grid(n)
    body = rounded(0.24, 0.12, 0.76, 0.54, 0.06, n)
    d = np.hypot(x - 0.5, y - 0.64)
    legs = (np.abs(x - 0.5) > 0.11) & (np.abs(x - 0.5) < 0.185) & (y > 0.5) & (y < 0.64)
    shackle = ((d < 0.185) & (d > 0.11) & (y >= 0.64)) | legs
    hole = disc(0.5, 0.375, 0.058, n) | fill([(0.474, 0.37), (0.526, 0.37), (0.545, 0.215), (0.455, 0.215)], n)
    return ((body | shackle) & ~hole).astype(np.float32).reshape(size, 4, size, 4).mean((1, 3))


def lock():
    px = np.ones((SIZE, SIZE, 4), np.float32)
    px[:, :, 3] = lock_alpha(SIZE)
    img = bpy.data.images.new("lock", SIZE, SIZE, alpha=True)
    img.pixels.foreach_set(px.ravel())
    img.filepath_raw = os.path.join(OUT, "lock.png")
    img.file_format = "PNG"
    img.save()


# ---------------------------------------------------------------- the game's icons
ICON = 96  # (the shop card's item boxes are 50-60 pixels at 1680x1050)
PACKED = ("shirt", "scarf", "number", "kit")  # the DLL's order (stadium.rs `merch_icon`), the lock last


def load(path):
    img = bpy.data.images.load(path)
    a = np.empty(img.size[0] * img.size[1] * 4, np.float32)
    img.pixels.foreach_get(a)
    a = a.reshape(img.size[1], img.size[0], 4)[::-1]  # (rows top down, as the screen)
    bpy.data.images.remove(img)
    return a


def pack():
    """ui\\src\\merch.bin, the shop card's item pictures in the club's colours: per picture 96x96
    pixels of (light 0..255, alpha, class 0/1/2), rows top down. Each piece is rendered twice at
    4x: in greys (its light) and as the flat class mask; then averaged down 4x4."""
    global MODE, SIZE
    SIZE, f = ICON * 4, 4
    out = bytearray()
    for name in PACKED:
        shots = {}
        for MODE in ("neutral", "class"):
            globals()[name]()
            shots[MODE] = load(os.path.join(OUT, "pack", f"{name}_{MODE}.png"))
        grey, cls = shots["neutral"], shots["class"]
        alpha = grey[:, :, 3]
        light = (grey[:, :, 0] * 0.3 + grey[:, :, 1] * 0.6 + grey[:, :, 2] * 0.1) * 255
        kind = np.choose(np.argmax(cls[:, :, :3], 2), [1, 2, 0])
        # (the light scaled so the lit cloth is 215, the DLL's 205 a club colour at full)
        # (and its folds deepened, a curve of 1.8: flat club colours lost the reference's cloth)
        cloth = (kind > 0) & (alpha > 0.5)
        light = 215.0 * (light / max(np.percentile(light[cloth], 90), 1.0)) ** 1.8
        blocks = lambda a: a.reshape(ICON, f, ICON, f)
        a = blocks(alpha).mean((1, 3))
        l = (blocks(light * alpha).sum((1, 3)) / np.maximum(blocks(alpha).sum((1, 3)), 1e-6)).clip(0, 255)
        votes = np.stack([blocks((kind == k) * alpha).sum((1, 3)) for k in (0, 1, 2)], 2)
        k = np.argmax(votes, 2)
        out += np.stack([l, a * 255, k], 2).round().astype(np.uint8).tobytes()
        print("packed", name)
    lk = lock_alpha(ICON)[::-1]
    out += np.stack([np.full_like(lk, 255), lk * 255, np.zeros_like(lk)], 2).round().astype(np.uint8).tobytes()
    MODE, SIZE = "ref", 512
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src", "merch.bin")
    open(path, "wb").write(bytes(out))
    print("written", path, len(out), "bytes")


os.makedirs(os.path.join(OUT, "pack"), exist_ok=True)
for name in os.environ.get("MERCH_ONLY", "shirt,jacket,scarf,mug,number,kit,lock,pack").split(","):
    globals()[name]()
print("merch written to", OUT)
