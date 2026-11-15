"""Dump contact sheets of the procedural art (handy when adding tiles).

    python -m game.tools.artdump [output_dir]

Writes ``tiles.png``, ``props.png``, ``characters.png`` and ``icons.png``.
"""
from __future__ import annotations

import os
import sys


def _init():
    os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
    os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
    import pygame
    pygame.init()
    pygame.display.set_mode((16, 16))
    return pygame


def dump_tiles(path: str, scale: int = 3) -> None:
    pygame = _init()
    from game.art import tiles
    from game.engine import palette
    from game.engine.font import get_font
    art = tiles.TileArt()
    font = get_font(1)
    names = [n for n, t in tiles.TILES.items() if not t.hidden]
    cols = 8
    cell_w, cell_h = 92, 34
    rows = (len(names) + cols - 1) // cols
    sheet = pygame.Surface((cols * cell_w, rows * cell_h))
    sheet.fill(palette.NAVY[1])
    for i, name in enumerate(names):
        tdef = tiles.TILES[name]
        x, y = (i % cols) * cell_w + 2, (i // cols) * cell_h + 2
        shown = max(tdef.frames, tdef.variants, 1)
        for k in range(min(shown, 5)):
            px = x + k * 17
            if tdef.layer != "ground":
                sheet.blit(art.get("floor_plate"), (px, y))
            if tdef.group:
                mask = (0, tiles.E | tiles.W, tiles.SO, tiles.E | tiles.W | tiles.SO, tiles.N)[k]
                sheet.blit(art.get(name, 0, k, mask), (px, y))
            elif tdef.frames > 1:
                sheet.blit(art.get(name, k, 0), (px, y))
            else:
                sheet.blit(art.get(name, 0, k), (px, y))
        font.draw(sheet, name, (x, y + 19), palette.WHITE)
    pygame.image.save(pygame.transform.scale(sheet, (sheet.get_width() * scale, sheet.get_height() * scale)), path)


def dump_props(path: str, scale: int = 3) -> None:
    pygame = _init()
    from game.art import tiles
    from game.engine import palette
    from game.engine.font import get_font
    art = tiles.TileArt()
    font = get_font(1)
    sheet = pygame.Surface((560, 230))
    sheet.fill(palette.NAVY[1])
    x = 4
    y = 4
    row_h = 0
    for name, pdef in tiles.PROPS.items():
        frames = max(tiles.TILES[pdef.parts[0][3]].frames, 1)
        for f in range(min(frames, 2)):
            w = pdef.w * 16
            if x + w + 4 > sheet.get_width():
                x = 4
                y += row_h + 14
                row_h = 0
            for dx, dy, _layer, part in pdef.parts:
                sheet.blit(art.get("floor_plate"), (x + dx * 16, y + dy * 16))
            for dx, dy, _layer, part in pdef.parts:
                sheet.blit(art.get(part, f), (x + dx * 16, y + dy * 16))
            if f == 0:
                font.draw(sheet, name, (x, y + pdef.h * 16 + 1), palette.WHITE)
            x += w + 6
            row_h = max(row_h, pdef.h * 16)
        x += 6
    pygame.image.save(pygame.transform.scale(sheet, (sheet.get_width() * scale, sheet.get_height() * scale)), path)


def main(argv=None) -> int:
    import argparse
    parser = argparse.ArgumentParser(prog="python -m game.tools.artdump",
                                     description="Write contact sheets of every tile, prop, sprite and icon.")
    parser.add_argument("out", nargs="?", default="art_dump", help="output directory (default: ./art_dump)")
    out = parser.parse_args(argv).out
    os.makedirs(out, exist_ok=True)
    dump_tiles(os.path.join(out, "tiles.png"))
    dump_props(os.path.join(out, "props.png"))
    try:
        from game.tools import artdump_chars
        artdump_chars.dump(out)
    except ImportError:
        pass
    print("wrote art sheets to", out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
