"""Derive a Bold companion from the checked-in Regular face. Requires fontTools."""

import hashlib
from io import BytesIO
from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._g_l_y_f import GlyphCoordinates


def main():
    root = Path(__file__).resolve().parents[1] / "fonts"
    source = (root / "accelerando-terminal.ttf").read_bytes()
    assert hashlib.sha256(source).hexdigest() == "b0ebee3d316d0afcdac01f506fbbebd721d880c6e13126eb43bf40a6def6abb5"
    font = TTFont(BytesIO(source), recalcTimestamp=False)
    original = font.getBestCmap()
    metrics = dict(font["hmtx"].metrics)
    outlines = []
    for name in font.getGlyphOrder():
        glyph = font["glyf"][name]
        if glyph.numberOfContours == 0:
            continue
        assert glyph.numberOfContours > 0
        points = list(glyph.coordinates)
        start = 0
        for end in glyph.endPtsOfContours:
            corners = points[start:end + 1]
            assert len(corners) == 4 and all(flag & 1 for flag in glyph.flags[start:end + 1])
            assert tuple(corners[0][i] + corners[2][i] for i in (0, 1)) == tuple(corners[1][i] + corners[3][i] for i in (0, 1))
            edges = [(corners[i + 1][0] - corners[i][0], corners[i + 1][1] - corners[i][1]) for i in (0, 1)]
            short = min(range(2), key=lambda i: edges[i][0] ** 2 + edges[i][1] ** 2)
            dx, dy = edges[short]
            # Double stroke thickness around its center without duplicating outlines.
            for i, (x, y) in enumerate(corners):
                direction = -0.5 if i in (short, (short + 3) % 4) else 0.5
                points[start + i] = (x + direction * dx, y + direction * dy)
            start = end + 1
        outlines.append((glyph, points))
    bottom = min(y for _, points in outlines for _, y in points)
    top = max(y for _, points in outlines for _, y in points)
    for glyph, points in outlines:
        glyph.coordinates = GlyphCoordinates([(round(x), round((y - bottom) * 778 / (top - bottom))) for x, y in points])
    font["OS/2"].usWeightClass = 700
    font["OS/2"].fsSelection = (font["OS/2"].fsSelection & ~64) | 32
    font["head"].macStyle |= 1
    for record in font["name"].names:
        if record.nameID in (2, 3, 4, 6):
            record.string = record.toUnicode().replace("Regular", "Bold").encode(record.getEncoding())
    payload = BytesIO()
    font.save(payload)
    data = payload.getvalue()
    checked = TTFont(BytesIO(data))
    assert checked.getBestCmap() == original
    assert checked["hmtx"].metrics == metrics
    assert checked["OS/2"].usWeightClass == 700
    distinct = set()
    for cp in range(0x100000, 0x102000):
        glyph = checked["glyf"][original[cp]]
        assert glyph.numberOfContours > 0
        assert 0 <= glyph.xMin < glyph.xMax <= 600
        assert 0 <= glyph.yMin < glyph.yMax <= 778
        distinct.add((tuple(glyph.coordinates), tuple(glyph.endPtsOfContours)))
    assert len(distinct) == 8192
    target = root / "accelerando-terminal-bold.ttf"
    target.write_bytes(data)
    print(f"{target}: {len(data)} bytes; SHA256 {hashlib.sha256(data).hexdigest()}")


if __name__ == "__main__":
    main()
