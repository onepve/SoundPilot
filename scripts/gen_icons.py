#!/usr/bin/env python3
"""Generate SoundPilot icons: geometric volume icon (speaker + waves), PNG + ICO.
Redesign: bigger speaker, thicker waves so it reads at 16-32px."""
import struct, os
from PIL import Image, ImageDraw

OUT = "/data/projects/SoundPilot/src-tauri/icons"
os.makedirs(OUT, exist_ok=True)
BASE = 512

def build(size: int) -> Image.Image:
    s = size / BASE
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    # indigo rounded square background
    d.rounded_rectangle((int(16*s), int(16*s), int(size-16*s), int(size-16*s)),
                        radius=int(104*s), fill=(63, 81, 181, 255))
    # speaker: cabinet + cone, scaled to occupy left-center, vertical center 256
    cab = (int(96*s), int(206*s), int(176*s), int(306*s))
    d.rounded_rectangle(cab, radius=int(10*s), fill=(255, 255, 255, 255))
    d.polygon([(int(168*s), int(236*s)), (int(268*s), int(140*s)),
               (int(268*s), int(372*s)), (int(168*s), int(276*s))],
              fill=(255, 255, 255, 255))
    # three thick arcs centered at (285, 256)
    cx, cy = 285*s, 256*s
    for r, w in ((52, 22), (104, 22), (156, 22)):
        rpx, wpx = r*s, w*s
        d.arc((cx-rpx, cy-rpx, cx+rpx, cy+rpx), start=-52, end=52,
              fill=(255, 255, 255, 255), width=max(2, int(wpx)))
    return img

for name, sz in [("icon.png", 512), ("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256)]:
    im = build(512) if sz == 512 else build(512).resize((sz, sz), Image.LANCZOS)
    im.save(os.path.join(OUT, name))
    print("wrote", name, sz)

ico_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
build(512).save(os.path.join(OUT, "icon.ico"), format="ICO", sizes=ico_sizes)
with open(os.path.join(OUT, "icon.ico"), "rb") as f:
    data = f.read()
count = struct.unpack("<H", data[4:6])[0]
print("icon.ico bytes:", len(data), "entries:", count)
assert count >= 6

# ASCII sanity render at 32x32
sp = build(512).resize((32, 32), Image.LANCZOS).load()
for y in range(32):
    print("".join(" " if sp[x, y][3] < 100 else ("#" if min(sp[x, y][:3]) > 220 else ".")
                  for x in range(32)))
print("OK")
