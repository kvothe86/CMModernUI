r"""The staff portraits for the monthly staff reports (user 2026-11-24): the 9 photos in
staff_faces\*.webp cut to head and shoulders, made small, with a clothing mask so the DLL can
dress them in the club's colours (`staff.rs`).

Writes src\staff_faces.bin: for each face N x N pixels of 6 bytes:
  R, G, B   the photo
  main      0..255 how much of the pixel is shirt / jacket (the club's first colour)
  trim      0..255 how much is trim: piping, a logo, a shirt under a jacket (the second colour)
  shade     the pixel's light against its clothing part's average, x 128 (folds and shadows)

python staff_faces.py [preview.png]   (the preview: each face as cut, its mask, and in four kits)
"""
import sys
from pathlib import Path

import cv2
import numpy as np

N = 80
HERE = Path(__file__).parent
OUT = HERE / "src" / "staff_faces.bin"


def face_box(img):
    """The face (x, y, w, h): the biggest patch of skin in the photo's upper half (OpenCV 5 has
    no face cascade any more); its height from its width, a face being about 1.3 x as tall."""
    h, w = img.shape[:2]
    ycc = cv2.cvtColor(img, cv2.COLOR_BGR2YCrCb)
    cr, cb = ycc[..., 1].astype(int), ycc[..., 2].astype(int)
    skin = ((cr > 135) & (cr < 180) & (cb > 80) & (cb < 130)).astype(np.uint8)
    skin[h * 55 // 100 :, :] = 0
    skin = cv2.morphologyEx(skin, cv2.MORPH_OPEN, np.ones((9, 9), np.uint8))
    n, lab, stats, _ = cv2.connectedComponentsWithStats(skin)
    k = 1 + int(np.argmax(stats[1:, cv2.CC_STAT_AREA]))
    x, y, fw = stats[k, cv2.CC_STAT_LEFT], stats[k, cv2.CC_STAT_TOP], stats[k, cv2.CC_STAT_WIDTH]
    return x, y, fw, int(fw * 1.3)


def person_mask(img, face):
    """Person against the grey studio wall: GrabCut from the border (background) and the face."""
    h, w = img.shape[:2]
    m = np.full((h, w), cv2.GC_PR_FGD, np.uint8)
    b = 12
    m[:b, :] = m[:, :b] = m[:, -b:] = cv2.GC_BGD
    m[: h // 2, : w // 10] = m[: h // 2, -w // 10 :] = cv2.GC_BGD  # the wall beside the head
    fx, fy, fw, fh = face
    m[fy + fh // 4 : fy + fh * 3 // 4, fx + fw // 4 : fx + fw * 3 // 4] = cv2.GC_FGD
    m[h - 40 :, w // 3 : w * 2 // 3] = cv2.GC_FGD  # the chest at the bottom
    bg, fg = np.zeros((1, 65), np.float64), np.zeros((1, 65), np.float64)
    cv2.grabCut(img, m, None, bg, fg, 6, cv2.GC_INIT_WITH_MASK)
    return np.isin(m, (cv2.GC_FGD, cv2.GC_PR_FGD))


def skin_or_hair(img, face):
    """Skin (YCrCb), and the face's own column down to the chin + a beard's depth."""
    ycc = cv2.cvtColor(img, cv2.COLOR_BGR2YCrCb)
    cr, cb = ycc[..., 1].astype(int), ycc[..., 2].astype(int)
    skin = (cr > 135) & (cr < 180) & (cb > 80) & (cb < 130)
    skin = cv2.morphologyEx(skin.astype(np.uint8), cv2.MORPH_OPEN, np.ones((5, 5), np.uint8)).astype(bool)
    fx, fy, fw, fh = face
    h, w = img.shape[:2]
    yy, xx = np.mgrid[:h, :w]
    # the head and neck: an ellipse round the face, longer below (chin, beard, neck)
    cx, cy = fx + fw / 2, fy + fh * 0.5
    head = ((xx - cx) / (fw * 0.56)) ** 2 + ((yy - cy) / (fh * 0.6)) ** 2 < 1
    # (a beard below the chin)
    head |= ((xx - cx) / (fw * 0.36)) ** 2 + ((yy - fy - fh * 0.95) / (fh * 0.2)) ** 2 < 1
    # (below the chin only the neck is skin: a beige jacket reads as skin too)
    skin &= (yy < fy + fh) | (np.abs(xx - cx) < fw * 0.42)
    return skin | head


def clothing(img, face):
    fx, fy, fw, fh = face
    person = person_mask(img, face)
    cloth = person & ~skin_or_hair(img, face)
    cloth[: fy + fh * 6 // 10, :] = False  # nothing above the collar's top is clothing
    # (the grey wall where GrabCut took it for the person: beside a shoulder)
    lab = cv2.cvtColor(img, cv2.COLOR_BGR2LAB).astype(np.float32)
    wall = np.median(np.vstack([lab[:60, :60].reshape(-1, 3), lab[:60, -60:].reshape(-1, 3)]), axis=0)
    cloth &= np.linalg.norm(lab - wall, axis=2) > 14
    # (and beside the neck, where the light falls off down the wall: what has the wall's colour
    # and is far from the clothing's, so a white or grey shirt stays)
    grey = np.linalg.norm(lab[..., 1:] - wall[1:], axis=2) < 8
    grey[fy + fh * 13 // 10 :, :] = False
    cloth &= ~(grey & far_from(lab, cloth))
    cloth = cv2.morphologyEx(cloth.astype(np.uint8), cv2.MORPH_OPEN, np.ones((7, 7), np.uint8))
    # keep the big parts (the body), drop specks
    n, lab, stats, _ = cv2.connectedComponentsWithStats(cloth)
    keep = np.zeros(cloth.shape, bool)
    for k in range(1, n):
        if stats[k, cv2.CC_STAT_AREA] > 4000:
            keep |= lab == k
    return keep


def far_from(lab, cloth):
    """Pixels well away from the clothing's median colour, light weighing half as much."""
    d = lab - np.median(lab[cloth], axis=0)
    return np.sqrt((d[..., 0] * 0.5) ** 2 + d[..., 1] ** 2 + d[..., 2] ** 2) > 30


def split_trim(img, cloth):
    """Main and trim inside the clothing: trim is what stands well away from the clothing's
    median colour (piping, a logo, a shirt under a jacket), light weighing half as much as
    colour, so a shirt's folds and shadows stay main."""
    lab = cv2.cvtColor(img, cv2.COLOR_BGR2LAB).astype(np.float32)
    trim = cv2.morphologyEx((cloth & far_from(lab, cloth)).astype(np.uint8), cv2.MORPH_OPEN, np.ones((3, 3), np.uint8)).astype(bool)
    share = trim.sum() / max(cloth.sum(), 1)
    print(f"  trim {share:.3f}")
    if not 0.003 < share < 0.4:
        trim[:] = False
    return cloth & ~trim, trim


# (where GrabCut still took the grey wall for the person: photo, box in the cut square 0..1)
EXCLUDE = {"2": (0.0, 0.0, 0.36, 0.84)}


def build(path):
    print(path.name)
    img = cv2.imread(str(path), cv2.IMREAD_COLOR)
    face = face_box(img)
    cloth = clothing(img, face)
    main, trim = split_trim(img, cloth)
    lum = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY).astype(np.float32)
    shade = np.full(lum.shape, 128.0, np.float32)
    for part in (main, trim):
        if part.any():
            shade[part] = np.clip(lum[part] / max(np.median(lum[part]), 1.0) * 128, 0, 255)
    # the square: head and shoulders
    fx, fy, fw, fh = face
    side = int(fw * 2.5)
    h, w = img.shape[:2]
    x0 = max(0, min(int(fx + fw / 2 - side / 2), w - side))
    y0 = max(0, min(int(fy - fh * 0.42), h - side))
    cut = lambda a: a[y0 : y0 + side, x0 : x0 + side]
    if path.stem in EXCLUDE:
        e = [int(v * side) for v in EXCLUDE[path.stem]]
        for part in (main, trim):
            cut(part)[e[1] : e[3], e[0] : e[2]] = False
    small = lambda a: cv2.resize(cut(a).astype(np.float32), (N, N), interpolation=cv2.INTER_AREA)
    rgb = cv2.resize(cv2.cvtColor(cut(img), cv2.COLOR_BGR2RGB), (N, N), interpolation=cv2.INTER_AREA)
    wm, wt = small(main) * 255, small(trim) * 255
    # (shade only means something on clothing: averaged where it is)
    cm = small(main | trim)
    sh = small(shade * (main | trim)) / np.maximum(cm, 1e-3)
    sh[cm < 1e-3] = 128
    return np.dstack([rgb, wm, wt, sh]).clip(0, 255).astype(np.uint8)


def dress(face, colours):
    """The same recolour as staff.rs `dress`, for the preview."""
    rgb = face[..., :3].astype(np.float32)
    wm, wt = (face[..., k].astype(np.float32) / 255 for k in (3, 4))
    sh = face[..., 5].astype(np.float32)[..., None] / 128
    out = rgb * (1 - wm - wt)[..., None]
    for w, c in ((wm, colours[0]), (wt, colours[1])):
        out += w[..., None] * np.clip(np.array(c, np.float32) * (0.35 + 0.65 * sh) + 18 * (sh - 1), 0, 255)
    return out.clip(0, 255).astype(np.uint8)


def main():
    faces = [build(p) for p in sorted(HERE.glob("staff_faces/*.webp"), key=lambda p: int(p.stem))]
    assert len(faces) == 9, len(faces)
    OUT.write_bytes(b"".join(f.tobytes() for f in faces))
    print(f"{OUT}: {len(faces)} faces of {N} x {N}, {OUT.stat().st_size} bytes")
    if len(sys.argv) > 1:
        kits = [((0xd0, 0x10, 0x20), (0xff, 0xff, 0xff)), ((0x26, 0x52, 0x8e), (0xe8, 0xe8, 0xe8)), ((0x10, 0x10, 0x10), (0xf0, 0xc8, 0x10)), ((0x1a, 0x7a, 0x30), (0xff, 0xff, 0xff))]
        rows = []
        for f in faces:
            mask = np.zeros((N, N, 3), np.uint8)
            mask[..., 0], mask[..., 1] = f[..., 3], f[..., 4]
            rows.append(np.hstack([f[..., :3], mask] + [dress(f, k) for k in kits]))
        sheet = cv2.resize(np.vstack(rows), None, fx=3, fy=3, interpolation=cv2.INTER_NEAREST)
        cv2.imwrite(sys.argv[1], cv2.cvtColor(sheet, cv2.COLOR_RGB2BGR))


if __name__ == "__main__":
    main()
