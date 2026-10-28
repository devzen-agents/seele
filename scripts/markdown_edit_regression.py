"""Byte-exact oracle for a manual editor experiment; never invokes SEELE/SQLite.

Python 3.9+, standard library only. See docs/MARKDOWN-EDIT-REGRESSION.md.
"""

import argparse
import hashlib
import json
from pathlib import Path
import platform

POSITIONS = ("start", "middle", "end")
FILENAME = "synthetic-long.md"


def fixture(eol="lf"):
    """Deterministic >200 KiB document, independent of platform text translation."""
    lines = ["# Synthetic SEELE editor regression", "<!-- SENTINEL:START -->"]
    for index in range(1800):
        if index == 900:
            lines.append("<!-- SENTINEL:MIDDLE -->")
        if index in (0, 900, 1799):
            position = {0: "start", 900: "middle", 1799: "end"}[index]
            lines.append(f"edit-{position}: ORIGINAL-{position}")
        lines.extend([
            f"## Synthetic section {index:04d}",
            "Preserve: café, 日本語, 😀, e\u0301; no real memories or credentials.",
            "| key | value |", f"| synthetic-{index:04d} | untouched |",
            "```text", f"literal sample {index:04d}: **not an edit**", "```", "",
        ])
    lines.append("<!-- SENTINEL:END -->")
    separator = {"lf": "\n", "crlf": "\r\n"}[eol]
    return (separator.join(lines) + separator).encode("utf-8")


def expected(eol="lf", position="middle"):
    if position not in POSITIONS:
        raise ValueError("invalid edit position")
    before = fixture(eol)
    old = f"edit-{position}: ORIGINAL-{position}".encode("ascii")
    new = f"edit-{position}: REVISED-{position}".encode("ascii")
    if before.count(old) != 1:
        raise ValueError("edit anchor must occur exactly once")
    return before.replace(old, new, 1)


def verify(actual, eol="lf", position="middle"):
    wanted = expected(eol, position)
    mismatch = next((i for i, (a, b) in enumerate(zip(actual, wanted)) if a != b), None)
    if mismatch is None and len(actual) != len(wanted):
        mismatch = min(len(actual), len(wanted))
    return {
        "passed": actual == wanted,
        "expected_bytes": len(wanted),
        "actual_bytes": len(actual),
        "expected_sha256": hashlib.sha256(wanted).hexdigest(),
        "actual_sha256": hashlib.sha256(actual).hexdigest(),
        "first_mismatch_byte": mismatch,
        "eol": eol,
        "position": position,
        "platform": platform.platform(),
        "scope": "byte comparison only; editor identity/execution not verified",
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "verify"))
    parser.add_argument("directory", type=Path)
    parser.add_argument("--eol", choices=("lf", "crlf"), default="lf")
    parser.add_argument("--position", choices=POSITIONS, default="middle")
    args = parser.parse_args(argv)
    path = args.directory / FILENAME
    try:
        if args.command == "prepare":
            # A fresh directory is required: never overwrite an existing file/run.
            args.directory.mkdir(parents=False, exist_ok=False)
            with path.open("xb") as stream:
                stream.write(fixture(args.eol))
            print(f"Created {path} ({path.stat().st_size} bytes).")
            print(
                f"Edit only {FILENAME}: replace the unique exact text "
                f"'edit-{args.position}: ORIGINAL-{args.position}' with "
                f"'edit-{args.position}: REVISED-{args.position}'. "
                "Preserve every other byte, including newline style, Unicode, "
                "and the final newline. Do not run SEELE or access any database."
            )
            return 0
        report = verify(path.read_bytes(), args.eol, args.position)
        print(json.dumps(report, indent=2))
        return 0 if report["passed"] else 1
    except OSError as error:
        parser.exit(2, f"error: {error}\n")


if __name__ == "__main__":
    raise SystemExit(main())
