#!/usr/bin/env python3
"""Tethys Deep - launch the game.

    python main.py                 # play
    python main.py --scale 4       # bigger window
    python main.py --map hub       # jump straight into a map (used by the editor's play-test)
    python main.py --load slot1    # load a save slot directly
"""
from __future__ import annotations

import argparse
import sys


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description="Tethys Deep")
    parser.add_argument("--scale", type=int, default=None, help="window scale factor (1-8)")
    parser.add_argument("--data", default=None, help="content directory (default: ./data)")
    parser.add_argument("--map", default=None, help="start a quick test game on this map")
    parser.add_argument("--pos", default=None, help="x,y start position used with --map")
    parser.add_argument("--load", default=None, help="load a save slot (e.g. autosave, slot1)")
    parser.add_argument("--no-audio", action="store_true", help="disable sound")
    parser.add_argument("--debug", action="store_true", help="start with the debug overlay on")
    args = parser.parse_args(argv)

    try:
        import pygame  # noqa: F401
    except ImportError:
        print("This game needs pygame. Install it with:  pip install pygame-ce", file=sys.stderr)
        return 1

    from game import save
    from game.content import Content
    from game.engine.app import App
    from game.scenes.play import PlayScene
    from game.scenes.title import TitleScene
    from game.world.world import World

    content = Content.load(args.data)
    for problem in content.problems:
        print("content problem:", problem, file=sys.stderr)
    app = App(scale=args.scale, audio=not args.no_audio, caption=content.title, content=content)

    if args.load:
        world = save.load_game(content, args.load)
        scene = PlayScene(app, world)
    elif args.map:
        world = World.new_game(content)
        if args.map not in world.maps:
            print("unknown map %r; available: %s" % (args.map, ", ".join(world.maps)), file=sys.stderr)
            return 1
        x, y = world.player.x, world.player.y
        if args.pos:
            x, y = (int(v) for v in args.pos.split(","))
        else:
            gmap = world.maps[args.map]
            cell = world.nav.nearest_free(world.player, args.map, gmap.width // 2, gmap.height // 2, radius=40)
            if cell:
                x, y = cell
        world.transfer(world.player, args.map, x, y)
        world.flags["audit_authority"] = True
        scene = PlayScene(app, world)
    else:
        scene = TitleScene(app)
    if args.debug and isinstance(scene, PlayScene):
        scene.debug = True
    app.push_scene(scene)
    app.run()
    return 0


if __name__ == "__main__":
    sys.exit(main())
