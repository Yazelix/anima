"""Rebuild the embedded glyphs from the pinned Accelerando-8192 checkout."""

import hashlib
import json
from pathlib import Path
import struct
import sys


def main():
    if len(sys.argv) != 2:
        raise SystemExit("Usage: python3 demo/import_accelerando.py /path/to/accelerando-8192")
    source = (Path(sys.argv[1]) / "docs/data/alphabet.json").read_bytes()
    if hashlib.sha256(source).hexdigest() != "c587cb1c2ca6ab4fe49bc1bc91050a131c83f6ddc4131dfc706e1f14229141c8":
        raise SystemExit("alphabet.json does not match pinned revision b1e789d50ce92af0ce142675d38da451d0c8d078")
    bitmaps = []
    for shape in json.loads(source)["shapes"]:
        bitmap = 0
        for (x0, y0), (x1, y1) in shape:
            dx, dy = x1 - x0, y1 - y0
            # Every pinned segment joins neighboring grid points horizontally, vertically or diagonally.
            assert max(abs(dx), abs(dy)) == 1
            for step in range(4):
                x, y = x0 * 3 + dx * step, y0 * 3 + dy * step
                bitmap |= 1 << (y * 7 + x)
        bitmaps.append(bitmap)
    assert len(bitmaps) == len(set(bitmaps)) == 8192
    assert all(0 < bitmap < (1 << 49) for bitmap in bitmaps)
    payload = struct.pack("<8192Q", *bitmaps)
    target = Path(__file__).resolve().parents[1] / "src/accelerando-glyphs.bin"
    target.write_bytes(payload)
    print(f"{target}: {len(payload)} bytes; SHA256 {hashlib.sha256(payload).hexdigest()}")


if __name__ == "__main__":
    main()
