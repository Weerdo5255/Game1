"""The game's master palette.

A deliberately small, hand-picked set of colour ramps in the spirit of 16-bit
consoles.  All procedural art (tiles, sprites, icons, UI) pulls its colours
from here so that everything sits together on screen.

Ramps run dark -> light.  Plain tuples only: this module has no pygame
dependency so the tools and tests can use it too.
"""
from __future__ import annotations

from typing import Dict, List, Tuple

Color = Tuple[int, int, int]

BLACK: Color = (10, 10, 18)
SPACE: Color = (13, 14, 30)
WHITE: Color = (236, 240, 244)

# cold blue-grey metal used for most of the station
STEEL: List[Color] = [(26, 30, 46), (44, 52, 74), (68, 80, 106), (102, 118, 146), (148, 164, 188), (198, 210, 226)]
# deep blues for space, shadows and UI panels
NAVY: List[Color] = [(14, 16, 34), (22, 28, 58), (34, 44, 90), (52, 68, 128), (84, 108, 172)]
CYAN: List[Color] = [(14, 62, 80), (22, 108, 128), (44, 164, 176), (112, 220, 216), (196, 248, 240)]
GREEN: List[Color] = [(18, 54, 44), (34, 96, 58), (66, 146, 70), (128, 196, 84), (200, 236, 140)]
AMBER: List[Color] = [(98, 50, 24), (160, 88, 24), (220, 142, 38), (248, 198, 84), (252, 236, 168)]
RED: List[Color] = [(80, 18, 36), (144, 32, 44), (208, 60, 54), (244, 122, 94), (252, 196, 170)]
PURPLE: List[Color] = [(40, 22, 72), (78, 42, 126), (130, 76, 184), (188, 136, 232), (232, 204, 252)]
BROWN: List[Color] = [(46, 32, 30), (82, 56, 42), (126, 88, 58), (176, 132, 90), (220, 184, 140)]
ICE: List[Color] = [(36, 62, 104), (64, 108, 160), (118, 166, 212), (184, 220, 244), (234, 248, 252)]
ROCK: List[Color] = [(30, 28, 40), (50, 46, 62), (76, 70, 90), (110, 102, 122), (150, 142, 158)]
BEIGE: List[Color] = [(84, 74, 78), (128, 114, 108), (172, 156, 140), (212, 198, 176), (240, 232, 214)]

# Named single colours used by UI and text markup.
UI = {
    "bg": NAVY[0],
    "panel": NAVY[1],
    "panel_hi": NAVY[2],
    "border": CYAN[2],
    "border_dim": CYAN[0],
    "text": WHITE,
    "dim": STEEL[3],
    "disabled": STEEL[2],
    "accent": CYAN[3],
    "select": AMBER[3],
    "good": GREEN[3],
    "warn": AMBER[2],
    "bad": RED[2],
    "shadow": BLACK,
}

# <x>...</x> tags usable inside any displayed text (dialogue, item text...).
TEXT_COLORS: Dict[str, Color] = {
    "w": WHITE,
    "y": AMBER[3],
    "r": RED[3],
    "g": GREEN[3],
    "c": CYAN[3],
    "p": PURPLE[3],
    "d": STEEL[3],
    "b": NAVY[4],
}

# ---------------------------------------------------------------------------
# Character appearance options.  Each entry is (shadow, base, highlight).
# The character creator lists these by key, and character files refer to them
# by key, so adding an entry here immediately makes it available everywhere.
# ---------------------------------------------------------------------------
Ramp3 = Tuple[Color, Color, Color]

SKIN_TONES: Dict[str, Ramp3] = {
    "porcelain": ((196, 140, 124), (240, 196, 172), (252, 226, 206)),
    "fair": ((180, 118, 96), (228, 172, 138), (248, 208, 176)),
    "tan": ((150, 94, 68), (204, 146, 104), (232, 184, 140)),
    "olive": ((122, 88, 60), (176, 134, 92), (208, 172, 124)),
    "brown": ((92, 56, 44), (140, 90, 62), (176, 124, 88)),
    "umber": ((60, 38, 36), (98, 62, 48), (134, 90, 66)),
    "ebony": ((40, 28, 34), (70, 46, 44), (102, 68, 58)),
    "ashen": ((104, 112, 130), (160, 170, 184), (204, 212, 222)),
    "viridian": ((40, 96, 84), (84, 152, 124), (140, 200, 164)),
    "chrome": ((84, 98, 122), (148, 164, 188), (220, 230, 240)),
}

HAIR_COLORS: Dict[str, Ramp3] = {
    "black": ((14, 14, 24), (36, 34, 50), (70, 68, 92)),
    "brown": ((44, 28, 26), (84, 54, 40), (128, 88, 60)),
    "auburn": ((70, 24, 24), (130, 48, 34), (184, 88, 52)),
    "ginger": ((120, 52, 20), (196, 100, 36), (236, 152, 72)),
    "blonde": ((148, 108, 44), (220, 180, 84), (248, 228, 150)),
    "grey": ((78, 84, 100), (132, 140, 156), (188, 196, 208)),
    "white": ((150, 160, 180), (214, 222, 232), (244, 248, 252)),
    "blue": ((24, 44, 112), (48, 92, 184), (108, 160, 236)),
    "teal": ((14, 72, 84), (30, 136, 144), (96, 208, 200)),
    "magenta": ((92, 22, 84), (172, 52, 140), (232, 120, 196)),
    "green": ((22, 70, 40), (54, 132, 62), (128, 196, 96)),
    "crimson": ((88, 14, 30), (176, 36, 48), (236, 96, 88)),
}

CLOTH_COLORS: Dict[str, Ramp3] = {
    "navy": ((18, 24, 54), (36, 50, 104), (70, 92, 160)),
    "steel": ((44, 52, 74), (84, 98, 126), (136, 152, 180)),
    "slate": ((26, 30, 46), (52, 60, 82), (90, 102, 128)),
    "white": ((148, 160, 184), (214, 222, 234), (244, 248, 252)),
    "black": ((12, 12, 22), (30, 30, 46), (60, 62, 84)),
    "red": ((92, 20, 36), (168, 40, 46), (224, 88, 72)),
    "orange": ((120, 54, 20), (204, 104, 30), (244, 160, 68)),
    "amber": ((128, 84, 20), (212, 156, 44), (248, 212, 108)),
    "green": ((20, 62, 46), (40, 112, 64), (92, 168, 88)),
    "olive": ((48, 54, 34), (90, 100, 56), (140, 152, 92)),
    "teal": ((14, 66, 80), (26, 122, 136), (72, 188, 188)),
    "cyan": ((22, 100, 124), (56, 172, 188), (140, 228, 224)),
    "purple": ((48, 26, 88), (92, 52, 150), (150, 104, 208)),
    "magenta": ((96, 24, 88), (168, 52, 140), (224, 112, 192)),
    "brown": ((50, 34, 30), (96, 66, 46), (146, 106, 70)),
    "tan": ((112, 92, 70), (176, 152, 116), (222, 204, 168)),
}


def lerp(a: Color, b: Color, t: float) -> Color:
    """Linear blend between two colours (t = 0 -> a, t = 1 -> b)."""
    return (
        int(a[0] + (b[0] - a[0]) * t),
        int(a[1] + (b[1] - a[1]) * t),
        int(a[2] + (b[2] - a[2]) * t),
    )


def darken(c: Color, amount: float) -> Color:
    return lerp(c, BLACK, amount)


def lighten(c: Color, amount: float) -> Color:
    return lerp(c, WHITE, amount)
