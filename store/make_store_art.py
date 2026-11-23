#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["pillow>=10"]
# ///
"""Store art + launcher icons from headless renders in store/graphics/src (see store/README.md).

Re-render the sources with:
    exodrop --screenshot store/graphics/src/art-0-sq.png --sector 0 --cam aerial --size 2400x2400 --no-hud
    exodrop --screenshot store/graphics/src/art-2-wide.png --sector 2 --cam aerial --size 2048x1000 --no-hud
    exodrop --screenshot store/graphics/src/s0-title.png --sector 0 --cam title --size 1920x1080 --touch-ui
    (and s0-action / s1-action / s2-action with --cam action --touch-ui)
"""
import pathlib
from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "store/graphics/src"
OUT = ROOT / "store/graphics"
RES = ROOT / "mobile/android/app/src/main/res"
FONT = "/System/Library/Fonts/Supplemental/Impact.ttf"


def crop(src, cx, cy, size, out):
    im = Image.open(SRC / src).convert("RGB")
    return im.crop((cx - size // 2, cy - size // 2, cx + size // 2, cy + size // 2)).resize((out, out), Image.LANCZOS)


# Play hi-res icon: 512x512, 32-bit, opaque
crop("art-0-sq.png", 880, 1560, 1100, 512).convert("RGBA").save(OUT / "icon-512.png")

# Launcher: adaptive background (108 dp; the visible mask is the centre ~66%) + legacy square icon
for dpi, scale in {"mdpi": 1, "hdpi": 1.5, "xhdpi": 2, "xxhdpi": 3, "xxxhdpi": 4}.items():
    d = RES / f"mipmap-{dpi}"
    d.mkdir(parents=True, exist_ok=True)
    crop("art-0-sq.png", 880, 1600, 1500, int(108 * scale)).save(d / "ic_launcher_background.png")
    crop("art-0-sq.png", 880, 1560, 1100, int(48 * scale)).save(d / "ic_launcher.png")
(RES / "mipmap-anydpi-v26").mkdir(parents=True, exist_ok=True)
(RES / "mipmap-anydpi-v26/ic_launcher.xml").write_text(
    '<?xml version="1.0" encoding="utf-8"?>\n'
    '<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">\n'
    '    <background android:drawable="@mipmap/ic_launcher_background" />\n'
    '    <foreground android:drawable="@android:color/transparent" />\n'
    "</adaptive-icon>\n")

# Feature graphic: 1024x500, opaque, title on the left over a darkened gradient
fg = Image.open(SRC / "art-2-wide.png").convert("RGB").resize((1024, 500), Image.LANCZOS)
shade = Image.new("L", (1024, 500))
ImageDraw.Draw(shade).rectangle((0, 0, 1024, 500), fill=0)
for x in range(560):
    ImageDraw.Draw(shade).line((x, 0, x, 500), fill=int(170 * (1 - x / 560) ** 1.4))
fg = Image.composite(Image.new("RGB", fg.size, (8, 12, 24)), fg, shade)
d = ImageDraw.Draw(fg)
title = ImageFont.truetype(FONT, 120)
tag = ImageFont.truetype(FONT, 34)
glow = Image.new("RGBA", fg.size, (0, 0, 0, 0))
ImageDraw.Draw(glow).text((52, 140), "EXODROP", font=title, fill=(46, 235, 255, 200))
fg.paste(glow.filter(ImageFilter.GaussianBlur(10)), (0, 0), glow.filter(ImageFilter.GaussianBlur(10)))
d.text((52, 140), "EXODROP", font=title, fill=(240, 248, 255))
d.text((56, 290), "ORBITAL EXO-SUIT ACTION", font=tag, fill=(255, 170, 40))
fg.save(OUT / "feature-graphic.png")

# Phone screenshots (16:9 landscape, 1920x1080, real renders with the touch HUD)
for i, name in enumerate(["s0-action", "s1-action", "s2-action", "s0-title"], 1):
    Image.open(SRC / f"{name}.png").convert("RGB").save(OUT / f"phone-{i}.png")
print("ok")
