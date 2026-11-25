# turn the blender renders (monitor.py output) into the site's assets:
#   public/frames/1440/*.webp and public/frames/800/*.webp (alpha kept, the page supplies the black)
#   public/og.png, 1200x630, the front-facing frame on the page colour
# usage: python3 -I render/frames.py RENDER_DIR      (needs cwebp and macOS sips)
import os
import struct
import subprocess
import sys
import tempfile
import zlib

SRC = sys.argv[1]
HERE = os.path.dirname(os.path.abspath(__file__))
PUB = os.path.join(HERE, "..", "public")
BG = (8, 9, 10)


def webp(src, dst, width=None):
    cmd = ["cwebp", "-quiet", "-q", "84", "-alpha_q", "92", "-m", "6", "-sharp_yuv"]
    if width:
        cmd += ["-resize", str(width), "0"]
    subprocess.run(cmd + [src, "-o", dst], check=True)


def read_png(path):
    d = open(path, "rb").read()
    i, idat, w, h, ct = 8, b"", 0, 0, 0
    while i < len(d):
        n, = struct.unpack(">I", d[i:i + 4])
        t, c = d[i + 4:i + 8], d[i + 8:i + 8 + n]
        if t == b"IHDR":
            w, h, bd, ct = struct.unpack(">IIBB", c[:10])
            assert bd == 8 and ct in (2, 6), "expected 8-bit rgb or rgba"
        if t == b"IDAT":
            idat += c
        i += 12 + n
    bpp = 4 if ct == 6 else 3
    raw, stride = zlib.decompress(idat), w * bpp
    rows, prev, p = [], bytearray(stride), 0
    for _ in range(h):
        f = raw[p]
        p += 1
        line = bytearray(raw[p:p + stride])
        p += stride
        for x in range(stride):
            a = line[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            if f == 1:
                line[x] = (line[x] + a) & 255
            elif f == 2:
                line[x] = (line[x] + b) & 255
            elif f == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line
    return w, h, bpp, rows


def write_png(path, w, h, rgb_rows):
    raw = b"".join(b"\0" + bytes(r) for r in rgb_rows)

    def chunk(t, c):
        return struct.pack(">I", len(c)) + t + c + struct.pack(">I", zlib.crc32(t + c) & 0xFFFFFFFF)

    open(path, "wb").write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def og(front):
    W, H, inner_h = 1200, 630, 600
    with tempfile.TemporaryDirectory() as tmp:
        small = os.path.join(tmp, "small.png")
        subprocess.run(["sips", "--resampleHeight", str(inner_h), front, "--out", small], check=True, capture_output=True)
        w, h, bpp, rows = read_png(small)
    ox, oy = (W - w) // 2, H - h
    out = [bytearray(bytes(BG) * W) for _ in range(H)]
    # feather towards the frame edges like the page's css mask, so the floor's faint shadow
    # never ends in a visible rectangle
    def feather(x, y):
        d = ((x / w - 0.5) / 0.62) ** 2 + ((y / h - 0.52) / 0.68) ** 2
        d = d ** 0.5
        return 1.0 if d <= 0.62 else max(0.0, 1 - (d - 0.62) / 0.38)

    for y in range(h):
        src, dst = rows[y], out[oy + y]
        for x in range(w):
            r, g, b = src[x * bpp:x * bpp + 3]
            a = (src[x * bpp + 3] / 255 if bpp == 4 else 1) * feather(x, y)
            dst[(ox + x) * 3:(ox + x) * 3 + 3] = bytes(int(v * a + bg * (1 - a)) for v, bg in zip((r, g, b), BG))
    write_png(os.path.join(PUB, "og.png"), W, H, out)


names = sorted(n for n in os.listdir(SRC) if n.endswith(".png") and (n.startswith("turn_") or n.startswith("boot_")))
for size, width in (("1440", None), ("800", 800)):
    os.makedirs(os.path.join(PUB, "frames", size), exist_ok=True)
    for n in names:
        if size == "800" and n.startswith("boot_"):
            continue  # the power-on stills are only shown in the 1440 set
        webp(os.path.join(SRC, n), os.path.join(PUB, "frames", size, n[:-4] + ".webp"), width)
turns = [n for n in names if n.startswith("turn_")]
og(os.path.join(SRC, turns[-1]))
print("frames:", len(turns), "turn +", len(names) - len(turns), "boot; og.png written")
