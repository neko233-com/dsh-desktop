#!/usr/bin/env python3
"""Generate platform icon files from one square mascot master image."""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image


MAC_FILES = (
    (16, "icon_16x16.png"),
    (32, "icon_16x16@2x.png"),
    (32, "icon_32x32.png"),
    (64, "icon_32x32@2x.png"),
    (128, "icon_128x128.png"),
    (256, "icon_128x128@2x.png"),
    (256, "icon_256x256.png"),
    (512, "icon_256x256@2x.png"),
    (512, "icon_512x512.png"),
    (1024, "icon_512x512@2x.png"),
)


def square(image: Image.Image, size: int) -> Image.Image:
    return image.resize((size, size), Image.Resampling.LANCZOS)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", default="assets/icons/icon-master.png")
    parser.add_argument("--windows-output", default="packaging/windows/dsh-desktop.ico")
    parser.add_argument("--macos-output", default="packaging/macos/AppIcon.iconset")
    args = parser.parse_args()

    source = Image.open(args.source).convert("RGBA")
    if source.width != source.height:
        raise SystemExit("icon master must be square")

    windows_output = Path(args.windows_output)
    windows_output.parent.mkdir(parents=True, exist_ok=True)
    square(source, 1024).save(
        windows_output,
        format="ICO",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )

    macos_output = Path(args.macos_output)
    macos_output.mkdir(parents=True, exist_ok=True)
    for size, name in MAC_FILES:
        square(source, size).save(macos_output / name, format="PNG", optimize=True)


if __name__ == "__main__":
    main()
