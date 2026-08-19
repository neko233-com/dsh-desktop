#!/usr/bin/env python3
"""Fail CI when a desktop change forgets mandatory product plumbing."""

from __future__ import annotations

import struct
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def require_file(relative: str) -> None:
    path = ROOT / relative
    if not path.is_file():
        raise SystemExit(f"missing required file: {relative}")


def require_text(relative: str, *needles: str) -> None:
    text = (ROOT / relative).read_text(encoding="utf-8")
    missing = [needle for needle in needles if needle not in text]
    if missing:
        raise SystemExit(f"{relative} missing: {', '.join(missing)}")


def verify_png(relative: str) -> None:
    data = (ROOT / relative).read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"not a PNG: {relative}")
    width, height = struct.unpack(">II", data[16:24])
    if width != height or width < 512:
        raise SystemExit(f"icon master must be square and >=512px: {relative} ({width}x{height})")


def main() -> None:
    for path in (
        "AGENTS.md",
        "build.rs",
        "assets/window-chrome.js",
        "assets/goal-mode.js",
        "assets/pet-mode.js",
        "assets/icons/icon-master.png",
        "packaging/windows/dsh-desktop.ico",
        "packaging/macos/Info.plist",
        "scripts/generate-icons.py",
    ):
        require_file(path)

    require_text("src/main.rs", ".with_decorations(false)", "with_window_icon", "load_api_key", "WINDOW_CHROME_SCRIPT")
    require_text("assets/window-chrome.js", 'title="最小化"', 'title="最大化"', 'title="关闭"')
    require_text("assets/start.html", "api-key", "save_key")
    require_text(".github/workflows/release.yml", "scripts/generate-icons.py", "iconutil", "AppIcon.icns")
    verify_png("assets/icons/icon-master.png")

    ico = (ROOT / "packaging/windows/dsh-desktop.ico").read_bytes()
    reserved, kind, count = struct.unpack("<HHH", ico[:6])
    if reserved != 0 or kind != 1 or count == 0:
        raise SystemExit("invalid Windows ICO")

    print("desktop contract: ok")


if __name__ == "__main__":
    main()
