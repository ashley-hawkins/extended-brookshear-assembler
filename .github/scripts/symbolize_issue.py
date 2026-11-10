#!/usr/bin/env python3
"""Turns a wasm stack trace in a crash-report issue into function names and file:line.

Reads the issue body from $ISSUE_BODY (never from a shell string: it is untrusted input),
looks for `build: <git hash>` and V8/Firefox-style `...wasm-function[N]:0xOFFSET` frames,
downloads that build's debug symbols from $SYMBOLS_BASE_URL/symbols/, runs
`wasm-tools addr2line`, and writes a markdown comment to the path given as argv[1].
Writes nothing if there is nothing to do.

Testing without GitHub: set SYMBOLS_FILE to a local debug .wasm instead of SYMBOLS_BASE_URL.
"""
import os
import re
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request

BUILD_RE = re.compile(r"^\s*build:\s*([0-9a-f]{7,40})(\s*\(dirty\))?\s*$", re.M)
OFFSET_RE = re.compile(r"wasm-function\[\d+\]:(0x[0-9a-fA-F]+)")
MAX_FRAMES = 64
MAX_SYMBOLS_BYTES = 200 * 1024 * 1024


def main() -> int:
    out_path = sys.argv[1]
    body = os.environ.get("ISSUE_BODY", "")

    build = BUILD_RE.search(body)
    offsets = OFFSET_RE.findall(body)[:MAX_FRAMES]  # hex only: safe as command arguments
    if not build or not offsets:
        return 0  # not a wasm crash report
    if build.group(2):
        comment("This report is from a dirty (locally modified) build, so its symbols were not published.", out_path)
        return 0
    build_id = build.group(1)  # validated by the regex: only [0-9a-f], safe in paths and URLs

    with tempfile.TemporaryDirectory() as tmp:
        symbols = os.environ.get("SYMBOLS_FILE")
        if not symbols:
            symbols = os.path.join(tmp, "symbols.wasm")
            if not download_symbols(build_id, symbols):
                comment(f"No published symbols for build `{build_id}`, so this trace can't be resolved.", out_path)
                return 0

        r = subprocess.run(
            ["wasm-tools", "addr2line", symbols, *offsets],
            capture_output=True, text=True,
        )
        if r.returncode != 0:
            comment(f"Resolving the trace against build `{build_id}` failed:\n\n```\n{fence_safe(r.stderr)}\n```", out_path)
            return 0
        comment(f"Resolved against build `{build_id}`:\n\n```\n{fence_safe(r.stdout)}\n```", out_path)
    return 0


def download_symbols(build_id: str, dest: str) -> bool:
    """Fetches the build's symbols. The site answers unknown paths with a 200 HTML page,
    so the status code is not enough: the body must be a wasm module."""
    base = os.environ["SYMBOLS_BASE_URL"].rstrip("/")
    url = f"{base}/symbols/brookshear_ui_bg-{build_id}.wasm"
    try:
        # urllib sends no Accept-Encoding, so the server returns the decoded wasm.
        with urllib.request.urlopen(url, timeout=60) as response:
            data = response.read(MAX_SYMBOLS_BYTES + 1)
    except (urllib.error.URLError, TimeoutError):
        return False
    if len(data) > MAX_SYMBOLS_BYTES or not data.startswith(b"\0asm"):
        return False
    with open(dest, "wb") as f:
        f.write(data)
    return True


def fence_safe(text: str) -> str:
    return text.replace("```", "'''")[:60000]


def comment(text: str, path: str) -> None:
    with open(path, "w") as f:
        f.write(text + "\n\n<sub>Automatically generated from the DWARF of the published build.</sub>\n")


if __name__ == "__main__":
    sys.exit(main())
