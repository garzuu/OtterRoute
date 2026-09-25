#!/usr/bin/env python3
"""Prepara le illustrazioni della lontra per il pannello (web/public/brand/).

Le immagini originali sono PNG RGBA con sfondo trasparente. Lo script:
  - azzera i pixel quasi trasparenti (l'alone è nei colori dei pixel a alpha 0),
  - ritaglia al riquadro della figura,
  - ridimensiona (lato lungo max 320 px, basta per ~160 px a doppia densità) e ottimizza il PNG.

Non è in CI e Pillow non è una dipendenza del progetto. Uso, con un venv a parte:
  python3 -m venv /tmp/brand && /tmp/brand/bin/pip install pillow
  /tmp/brand/bin/python scripts/prepare-brand.py ~/Documents
"""
import sys
from pathlib import Path

from PIL import Image

OUT = Path(__file__).resolve().parent.parent / "web" / "public" / "brand"
MAX_SIDE = 320

# suffisso del file originale -> nome in uscita
SOURCES = {
    "23_02_02-1": "welcome",  # saluta
    "23_02_05-2": "bucket",  # con il bucket
    "23_02_08-3": "laptop",  # al portatile
    "23_02_10-4": "verified",  # con la spunta
    "23_02_11-5": "search",  # con la lente
    "23_02_12-6": "sleeping",  # dorme sul bucket
}


# avatar del profilo: lontra nel cerchio, quadrata e ridotta a 96 px (si mostra a 28–48 px)
AVATAR = ("23_24_02-6", "avatar")
AVATAR_SIZE = 96


def prepare_avatar(src: Path, dst: Path) -> None:
    im = Image.open(src).convert("RGBA")
    alpha = im.getchannel("A").point(lambda v: 0 if v < 24 else v)
    im.putalpha(alpha)
    im = im.crop(alpha.getbbox())
    side = max(im.size)
    canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    canvas.alpha_composite(im, ((side - im.width) // 2, (side - im.height) // 2))
    canvas.resize((AVATAR_SIZE, AVATAR_SIZE), Image.LANCZOS).save(dst, optimize=True)
    print(f"{dst.name}: {AVATAR_SIZE}x{AVATAR_SIZE}, {dst.stat().st_size // 1024} KB")


def prepare(src: Path, dst: Path) -> None:
    im = Image.open(src).convert("RGBA")
    alpha = im.getchannel("A").point(lambda v: 0 if v < 24 else v)
    im.putalpha(alpha)
    im = im.crop(alpha.getbbox())
    im.thumbnail((MAX_SIDE, MAX_SIDE), Image.LANCZOS)  # Pillow filtra in modo premoltiplicato
    im.save(dst, optimize=True)
    print(f"{dst.name}: {im.size[0]}x{im.size[1]}, {dst.stat().st_size // 1024} KB")


def main() -> None:
    folder = Path(sys.argv[1]).expanduser() if len(sys.argv) > 1 else Path.home() / "Documents"
    OUT.mkdir(parents=True, exist_ok=True)
    for suffix, name in SOURCES.items():
        matches = sorted(folder.glob(f"Immagine ChatGPT*{suffix}.png"))
        if not matches:
            sys.exit(f"originale non trovato: *{suffix}.png in {folder}")
        prepare(matches[0], OUT / f"{name}.png")
    suffix, name = AVATAR
    matches = sorted(folder.glob(f"Immagine ChatGPT*{suffix}.png"))
    if not matches:
        sys.exit(f"originale non trovato: *{suffix}.png in {folder}")
    prepare_avatar(matches[0], OUT / f"{name}.png")


if __name__ == "__main__":
    main()
