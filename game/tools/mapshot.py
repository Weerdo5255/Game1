"""Render maps to PNG files (documentation, quick visual checks).

    python -m game.tools.mapshot [out_dir] [map_id ...] [--regions] [--scale N]
"""
from __future__ import annotations

import os
import sys


def main(argv=None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    regions = "--regions" in argv
    scale = 2
    if "--scale" in argv:
        i = argv.index("--scale")
        scale = int(argv[i + 1])
        del argv[i:i + 2]
    args = [a for a in argv if not a.startswith("--")]
    out = args[0] if args else "."
    os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
    os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
    import pygame
    pygame.init()
    pygame.display.set_mode((16, 16))
    from game import config
    from game.art.chars import CharArt
    from game.art.icons import IconArt
    from game.art.tiles import TileArt
    from game.content import Content
    from game.engine.font import get_font
    from game.scenes.render import MapRenderer

    content = Content.load()
    renderer = MapRenderer(TileArt(), CharArt(), IconArt())
    font = get_font(1)
    os.makedirs(out, exist_ok=True)
    wanted = args[1:] or content.map_ids
    for map_id in wanted:
        gmap = content.load_map(map_id)
        if gmap is None:
            print("no such map:", map_id)
            continue
        surf = renderer.render_whole_map(gmap, time=0.4)
        if regions:
            t = config.TILE
            for region in gmap.regions:
                pygame.draw.rect(surf, (255, 220, 90), (region.x * t, region.y * t, region.w * t, region.h * t), 1)
                font.draw(surf, region.id, (region.x * t + 2, region.y * t + 2), (255, 220, 90), shadow=(0, 0, 0))
            for obj in gmap.objects:
                pygame.draw.rect(surf, (255, 90, 200), (obj.x * t + 1, obj.y * t + 1, t - 2, t - 2), 1)
        if scale != 1:
            surf = pygame.transform.scale(surf, (surf.get_width() * scale, surf.get_height() * scale))
        path = os.path.join(out, map_id + ".png")
        pygame.image.save(surf, path)
        print("wrote", path)
    return 0


if __name__ == "__main__":
    sys.exit(main())
