#!/usr/bin/env python3
"""Launch the character creator.  See game/tools/char_editor.py for the manual.

    python character_creator.py                              # editor window
    python character_creator.py --new "Kira Vance" --role engineer
    python character_creator.py --list
"""
import sys

from game.tools.char_editor import main

if __name__ == "__main__":
    sys.exit(main())
