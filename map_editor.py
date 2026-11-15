#!/usr/bin/env python3
"""Launch the map editor.  See game/tools/map_editor.py for the manual.

    python map_editor.py [map_id]
    python map_editor.py --new <id> [width height]
"""
import sys

from game.tools.map_editor import main

if __name__ == "__main__":
    sys.exit(main())
