#!/usr/bin/env python3

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
UI_MANIFEST = REPO_ROOT / "crates" / "ui" / "Cargo.toml"
OUTPUT_DIR = REPO_ROOT / "crates" / "ui" / "src" / "fonts"

TEXT_RANGES = [
    "U+0020-007E",  # Printable ASCII
]

MONOSPACE_RANGES = [
    "U+0020-007E",  # Printable ASCII
    "U+00A0-00FF",  # Latin-1 Supplement
    "U+0100-017F",  # Latin Extended-A
    "U+0180-024F",  # Latin Extended-B
    "U+0300-036F",  # Combining Diacritical Marks
    "U+1E00-1EFF",  # Latin Extended Additional
]

SHARED_EXTRA_CHARS = "◻?✔🗐📋─│┼┆┤▲▶╭┬╮╰┴╯├┌┐"
EMOJI_EXTRA_CHARS = "✔🗐📋"

FONT_JOBS = [
    {
        "source": "Ubuntu-Light.ttf",
        "target": "UbuntuLightSubset.ttf",
        "chars": SHARED_EXTRA_CHARS,
        "ranges": TEXT_RANGES,
    },
    {
        "source": "Hack-Regular.ttf",
        "target": "HackRegularSubset.ttf",
        "chars": SHARED_EXTRA_CHARS,
        "ranges": MONOSPACE_RANGES,
    },
    {
        "source": "NotoEmoji-Regular.ttf",
        "target": "NotoEmojiRegularSubset.ttf",
        "chars": EMOJI_EXTRA_CHARS,
        "ranges": [],
    },
    {
        "source": "emoji-icon-font.ttf",
        "target": "EmojiIconFontSubset.ttf",
        "chars": EMOJI_EXTRA_CHARS,
        "ranges": [],
    },
]


def main() -> int:
    pyftsubset = shutil.which("pyftsubset")
    if pyftsubset is None:
        print("error: `pyftsubset` was not found in PATH", file=sys.stderr)
        return 1

    fonts_dir = locate_epaint_fonts_dir()
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    for job in FONT_JOBS:
        subset_font(
            pyftsubset=pyftsubset,
            input_path=fonts_dir / job["source"],
            output_path=OUTPUT_DIR / job["target"],
            ranges=job["ranges"],
            chars=job["chars"],
        )

    print("Wrote subset fonts:")
    for job in FONT_JOBS:
        output_path = OUTPUT_DIR / job["target"]
        print(f"- {output_path.relative_to(REPO_ROOT)} ({output_path.stat().st_size} bytes)")

    return 0


def locate_epaint_fonts_dir() -> Path:
    metadata = cargo_metadata()
    packages = {package["id"]: package for package in metadata["packages"]}
    resolve = metadata.get("resolve")
    if resolve is not None:
        nodes = {node["id"]: node for node in resolve["nodes"]}
        root_id = next(
            package_id
            for package_id, package in packages.items()
            if Path(package["manifest_path"]) == UI_MANIFEST
        )

        reachable = [root_id]
        seen: set[str] = set()
        while reachable:
            package_id = reachable.pop()
            if package_id in seen:
                continue
            seen.add(package_id)

            package = packages[package_id]
            if package["name"] == "epaint_default_fonts":
                fonts_dir = Path(package["manifest_path"]).resolve().parent / "fonts"
                if not fonts_dir.is_dir():
                    raise RuntimeError(f"Resolved fonts directory does not exist: {fonts_dir}")
                return fonts_dir

            for dep in nodes.get(package_id, {}).get("deps", []):
                reachable.append(dep["pkg"])

    registry_fonts_dir = locate_epaint_fonts_dir_in_registry()
    if registry_fonts_dir is not None:
        return registry_fonts_dir

    raise RuntimeError(
        "Failed to locate `epaint_default_fonts`; install egui sources with `cargo fetch` first"
    )


def locate_epaint_fonts_dir_in_registry() -> Path | None:
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    registry_src = cargo_home / "registry" / "src"
    if not registry_src.is_dir():
        return None

    candidates = []
    for manifest_path in registry_src.glob("*/epaint_default_fonts-*/Cargo.toml"):
        package_dir = manifest_path.parent
        fonts_dir = package_dir / "fonts"
        if fonts_dir.is_dir():
            candidates.append((parse_version_suffix(package_dir.name), fonts_dir))

    if not candidates:
        return None

    candidates.sort(key=lambda item: item[0])
    return candidates[-1][1]


def parse_version_suffix(package_dir_name: str) -> tuple[int, ...]:
    version = package_dir_name.removeprefix("epaint_default_fonts-")
    numeric_parts = []
    for part in version.split("."):
        digits = "".join(char for char in part if char.isdigit())
        numeric_parts.append(int(digits or "0"))
    return tuple(numeric_parts)


def cargo_metadata() -> dict:
    result = subprocess.run(
        [
            "cargo",
            "metadata",
            "--format-version",
            "1",
            "--manifest-path",
            str(UI_MANIFEST),
        ],
        check=True,
        capture_output=True,
        text=True,
        cwd=REPO_ROOT,
    )
    return json.loads(result.stdout)


def subset_font(
    *,
    pyftsubset: str,
    input_path: Path,
    output_path: Path,
    ranges: list[str],
    chars: str,
) -> None:
    if not input_path.is_file():
        raise RuntimeError(f"Font source does not exist: {input_path}")

    unicodes = build_unicode_arg(ranges=ranges, chars=chars)
    subprocess.run(
        [
            pyftsubset,
            str(input_path),
            f"--output-file={output_path}",
            f"--unicodes={unicodes}",
            "--ignore-missing-unicodes",
            "--layout-features=*",
            "--glyph-names",
            "--symbol-cmap",
            "--legacy-cmap",
            "--notdef-glyph",
            "--notdef-outline",
            "--recommended-glyphs",
            "--name-IDs=*",
            "--name-legacy",
            "--name-languages=*",
            "--passthrough-tables",
            "--no-hinting",
        ],
        check=True,
        cwd=REPO_ROOT,
    )


def build_unicode_arg(*, ranges: list[str], chars: str) -> str:
    parts = list(ranges)
    seen_chars: set[str] = set()
    for char in chars:
        if char in seen_chars:
            continue
        seen_chars.add(char)
        parts.append(f"U+{ord(char):04X}")
    return ",".join(parts)


if __name__ == "__main__":
    raise SystemExit(main())
