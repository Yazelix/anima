"""Adapt the pinned upstream font for terminal cells. Requires fontTools."""

import hashlib
from io import BytesIO
from pathlib import Path
import sys

from fontTools.pens.transformPen import TransformPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont


def main():
    if len(sys.argv) != 2:
        raise SystemExit("Usage: python3 demo/import_accelerando_font.py /path/to/accelerando-8192")
    source = (Path(sys.argv[1]) / "docs/assets/accelerando-8192.ttf").read_bytes()
    if hashlib.sha256(source).hexdigest() != "4c1b5a2fb70dfd4f5930b95e8f48448035dc45ce7aced625944168cb2069f942":
        raise SystemExit("font does not match pinned revision b1e789d50ce92af0ce142675d38da451d0c8d078")
    font = TTFont(BytesIO(source), recalcTimestamp=False)
    original = font.getBestCmap()
    assert all(original[0xF0000 + i] != ".notdef" for i in range(8192))
    # Eon reserves supplementary-A for Nerd Font icons. Keep glyph IDs intact.
    for table in font["cmap"].tables:
        table.cmap = {
            cp + 0x10000 if 0xF0000 <= cp <= 0xF1FFF else cp: name
            for cp, name in table.cmap.items()
        }
    glyphs = font.getGlyphSet()
    for name in font.getGlyphOrder():
        pen = TTGlyphPen(None)
        # Fit a mono cell horizontally and match ordinary capitals vertically.
        glyphs[name].draw(TransformPen(pen, (0.6, 0, 0, 1.2, 0, -211.2)))
        font["glyf"][name] = pen.glyph()
        font["hmtx"][name] = (600, round(font["hmtx"][name][1] * 0.6))
    font["OS/2"].xAvgCharWidth = 600
    names = {
        1: "Accelerando Terminal", 2: "Regular", 3: "Accelerando Terminal Regular",
        4: "Accelerando Terminal Regular", 6: "AccelerandoTerminal-Regular",
    }
    for record in font["name"].names:
        if record.nameID in names:
            record.string = names[record.nameID].encode(record.getEncoding())
    payload = BytesIO()
    font.save(payload)
    data = payload.getvalue()
    checked = TTFont(BytesIO(data))
    cmap = checked.getBestCmap()
    assert set(cmap) == {32, *range(0x100000, 0x102000)}
    for i in range(8192):
        name = cmap[0x100000 + i]
        glyph = checked["glyf"][name]
        assert name == original[0xF0000 + i] and glyph.numberOfContours > 0
        assert checked["hmtx"][name][0] == 600
        assert 0 <= glyph.xMin < glyph.xMax <= 600
        assert 0 <= glyph.yMin < glyph.yMax <= 778
    target = Path(__file__).resolve().parents[1] / "fonts/accelerando-terminal.ttf"
    target.parent.mkdir(exist_ok=True)
    target.write_bytes(data)
    print(f"{target}: {len(data)} bytes; SHA256 {hashlib.sha256(data).hexdigest()}")


if __name__ == "__main__":
    main()
