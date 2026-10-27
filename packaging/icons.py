#!/usr/bin/env python3
"""Regenerate every DarkRaw app icon from assets/app-icon/darkraw-source.png.

Needs only Pillow (pip install pillow). The outputs are committed, so packaging never needs this.

    python packaging/icons.py

The source is a 512 px picture of a rounded-square tile. The tile is cut out
(TILE_BOX), its corners made transparent, and then rendered two ways, as the old SVG-based script did:

  * macOS (darkraw-1024.png, darkraw.icns): the tile padded onto Apple's 824/1024 body grid.
  * Windows and Linux (darkraw.ico, hicolor/*): the tile edge to edge, no padding (TIGHT_CROP can
    crop it further if the aperture needs to read bigger at 16-48 px).

photocraft.svg and photocraft-small.svg are NOT touched: a test in crates/io/src/svg.rs opens
photocraft.svg as a sample file.
"""
import base64
import io
import struct
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
DIR = ROOT / "assets" / "app-icon"
SRC = DIR / "darkraw-source.png"

# The tile inside the 512 px source (almost edge to edge); upscaled with Lanczos for the big sizes.
TILE_BOX = (2, 2, 510, 510)
CORNER = 0.19  # corner radius as a fraction of the tile side
TIGHT_CROP = 0.0  # Windows/Linux: fraction cropped off each side (0 = the whole rounded tile)
MAC_BODY = 824 / 1024  # Apple's icon body inside its 1024 canvas
SS = 4  # supersampling for the corner mask

HICOLOR = [16, 24, 32, 48, 64, 128, 256, 512]
ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]
APP_ID = "ai.storyteller.photocraft"


def masked_tile() -> Image.Image:
    tile = Image.open(SRC).convert("RGB").crop(TILE_BOX).resize((1016, 1016), Image.LANCZOS).convert("RGBA")
    side = tile.width
    big = Image.new("L", (side * SS, side * SS), 0)
    r = round(side * CORNER * SS)
    ImageDraw.Draw(big).rounded_rectangle((0, 0, side * SS - 1, side * SS - 1), radius=r, fill=255)
    tile.putalpha(big.resize((side, side), Image.LANCZOS))
    return tile


def png_bytes(im: Image.Image) -> bytes:
    out = io.BytesIO()
    im.save(out, "PNG", optimize=True)
    return out.getvalue()


def tight(tile: Image.Image, size: int) -> Image.Image:
    c = round(tile.width * TIGHT_CROP)
    return tile.crop((c, c, tile.width - c, tile.height - c)).resize((size, size), Image.LANCZOS)


def mac(tile: Image.Image, size: int) -> Image.Image:
    body = round(size * MAC_BODY)
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    off = (size - body) // 2
    canvas.paste(tile.resize((body, body), Image.LANCZOS), (off, off))
    return canvas


def write_ico(path: Path, images: list[Image.Image]) -> None:
    blobs = [png_bytes(im) for im in images]
    head = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries = b""
    for im, blob in zip(images, blobs):
        w = 0 if im.width >= 256 else im.width
        entries += struct.pack("<BBBBHHII", w, w, 0, 0, 1, 32, len(blob), offset)
        offset += len(blob)
    path.write_bytes(head + entries + b"".join(blobs))


def write_icns(path: Path, tile: Image.Image) -> None:
    # (type code, pixel size): all PNG payloads, 16..1024 incl. the @2x variants.
    kinds = [(b"icp4", 16), (b"icp5", 32), (b"ic11", 32), (b"ic12", 64), (b"ic07", 128),
             (b"ic13", 256), (b"ic08", 256), (b"ic14", 512), (b"ic09", 512), (b"ic10", 1024)]
    body = b""
    for code, size in kinds:
        blob = png_bytes(mac(tile, size))
        body += code + struct.pack(">I", len(blob) + 8) + blob
    path.write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)


def main() -> None:
    tile = masked_tile()

    mac(tile, 1024).save(DIR / "darkraw-1024.png", optimize=True)

    for s in HICOLOR:
        d = DIR / "hicolor" / f"{s}x{s}" / "apps"
        d.mkdir(parents=True, exist_ok=True)
        (d / f"{APP_ID}.png").write_bytes(png_bytes(tight(tile, s)))

    # The scalable theme icon: the 512 px render wrapped in an SVG.
    b64 = base64.b64encode(png_bytes(tight(tile, 512))).decode()
    d = DIR / "hicolor" / "scalable" / "apps"
    d.mkdir(parents=True, exist_ok=True)
    (d / f"{APP_ID}.svg").write_text(
        '<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512">'
        f'<image width="512" height="512" href="data:image/png;base64,{b64}"/></svg>\n',
        encoding="utf-8",
    )

    write_ico(DIR / "darkraw.ico", [tight(tile, s) for s in ICO_SIZES])
    write_icns(DIR / "darkraw.icns", tile)
    print(f"icons written to {DIR}")


if __name__ == "__main__":
    main()
