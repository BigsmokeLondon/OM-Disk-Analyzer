"""Build the app's logo images from the OM Industries logo pack (needs Pillow, only when rebuilding).

Usage: python tools/make_assets.py "<folder of the logo pack>"
Writes ready-sized PNGs (and the .ico) into assets/ so the app itself needs nothing extra.
"""
import os
import shutil
import sys

from PIL import Image

SIZES = {
    "icon": ("OM_Industries_Icon_512.png", [16, 20, 24, 32, 40, 48, 64, 128, 256], "square"),
    "badge": ("OM_Industries_Logo.png", [96, 128, 160, 200, 256, 320], "square"),
    "horizontal": ("OM_Industries_Horizontal.png", [24, 30, 36, 45, 54, 72], "height"),
}


def main(src):
    out = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "assets")
    os.makedirs(out, exist_ok=True)
    for kind, (name, sizes, how) in SIZES.items():
        im = Image.open(os.path.join(src, name)).convert("RGBA")
        im = im.crop(im.getbbox())
        for s in sizes:
            if how == "square":
                side = max(im.size)
                sq = Image.new("RGBA", (side, side), (0, 0, 0, 0))
                sq.paste(im, ((side - im.width) // 2, (side - im.height) // 2))
                res = sq.resize((s, s), Image.LANCZOS)
            else:
                res = im.resize((max(1, round(im.width * s / im.height)), s), Image.LANCZOS)
            res.save(os.path.join(out, f"{kind}_{s}.png"), optimize=True)
    shutil.copyfile(os.path.join(src, "OM_Industries_Icon.ico"), os.path.join(out, "om.ico"))
    # macOS app icon
    big = Image.open(os.path.join(src, "OM_Industries_Icon_512.png")).convert("RGBA")
    big.resize((1024, 1024), Image.LANCZOS).save(os.path.join(out, "om.icns"))
    print("assets written to", out)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else ".")
