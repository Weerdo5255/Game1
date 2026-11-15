"""Parametric character sprites.

A character's look is described by a small ``appearance`` dictionary (see
:data:`DEFAULT_APPEARANCE`), which is exactly what is stored in a character's
JSON file::

    {"body": "human", "skin": "tan", "hair": "short", "hair_color": "black",
     "outfit": "uniform", "primary": "navy", "secondary": "amber",
     "legs": "slate", "extras": ["glasses"]}

From that the generator builds a full walk cycle in four directions (16x24
sprites), a "sleeping" head and a 32x32 dialogue portrait.  Nobody has to draw
anything to add a new crew member: the character creator just edits these
keys.  New hair styles / outfits are added by extending the tables below.

Template legend
---------------
``o`` outline   ``s S`` skin / shadow   ``n`` hands   ``e`` eyes
``h H i`` hair / shadow / highlight     ``a A j`` top / shadow / highlight
``t`` shirt     ``b B`` trim colour     ``p P`` legs  ``f F`` boots
``v V w`` visor glass / shadow / glint  ``g`` glasses
"""
from __future__ import annotations

from typing import Dict, List, Optional, Sequence, Tuple

from ..engine import palette as P

W, H = 16, 24
PORTRAIT = 32
OUTLINE = (16, 14, 28)

DEFAULT_APPEARANCE = {
    "body": "human",
    "skin": "tan",
    "hair": "short",
    "hair_color": "brown",
    "outfit": "uniform",
    "primary": "steel",
    "secondary": "amber",
    "legs": "slate",
    "extras": [],
}

BODIES = ["human", "drone"]
EXTRAS = ["glasses", "beard"]

_E = "................"

# ---------------------------------------------------------------------------
# Base body (bald).  Rows 0..23.
# ---------------------------------------------------------------------------
_LEGS_IDLE = [
    "....oppppppo....",
    "....opppppPo....",
    "....oppoopPo....",
    "....oppoopPo....",
    "....offoofFo....",
    "....offoofFo....",
    "....oooooooo....",
]
_LEGS_STEP = [
    "....oppppppno...",
    "....opppppPo....",
    "....oppoopPo....",
    "....oppoofFo....",
    "....offoofFo....",
    "....offo.oo.....",
    "....oooo........",
]

_DOWN_HEAD = [
    _E, _E,
    ".....oooooo.....",
    "....osssssso....",
    "...osssssssso...",
    "...osssssssso...",
    "...osssssssso...",
    "...osssssssso...",
    "...ossessesso...",
    "...ossessesso...",
    "...oSssssssSo...",
    "....oSssssSo....",
]
_UP_HEAD = [
    _E, _E,
    ".....oooooo.....",
    "....osssssso....",
    "...osssssssso...",
    "...osssssssso...",
    "...osssssssso...",
    "...osssssssso...",
    "...osssssssso...",
    "...osssssssso...",
    "...oSssssssSo...",
    "....oSSSSSSo....",
]
_LEFT_HEAD = [
    _E, _E,
    ".....oooooo.....",
    "....osssssso....",
    "...osssssssso...",
    "...osssssssso...",
    "...osssssssso...",
    "...osssssssso...",
    "...osessssSso...",
    "...osessssSso...",
    "...osssssssSo...",
    "....osssssSo....",
]

BASE: Dict[str, List[List[str]]] = {
    "down": [
        _DOWN_HEAD + [
            "....oaaSSaao....",
            "...oaaabbaaao...",
            "...oAaaaaaaAo...",
            "...oAaaaaaaAo...",
            "...onbbbbbbno...",
        ] + _LEGS_IDLE,
        _DOWN_HEAD + [
            "....oaaSSaao....",
            "...oaaabbaaao...",
            "...oAaaaaaaAo...",
            "...oAaaaaaaAo...",
            "...onbbbbbbAo...",
        ] + _LEGS_STEP,
    ],
    "up": [
        _UP_HEAD + [
            "....oaaaaaao....",
            "...oaaaaaaaao...",
            "...oAaaaaaaAo...",
            "...oAaaaaaaAo...",
            "...onbbbbbbno...",
        ] + _LEGS_IDLE,
        _UP_HEAD + [
            "....oaaaaaao....",
            "...oaaaaaaaao...",
            "...oAaaaaaaAo...",
            "...oAaaaaaaAo...",
            "...onbbbbbbAo...",
        ] + _LEGS_STEP,
    ],
    "left": [
        _LEFT_HEAD + [
            ".....oaSSao.....",
            "....oaaaaaao....",
            "....oaaAAaao....",
            "....oaaAAaao....",
            "....obbnnbbo....",
            "....oppppppo....",
            ".....opppPo.....",
            ".....opppPo.....",
            ".....opppPo.....",
            "....offffFo.....",
            "....offffFo.....",
            "....ooooooo.....",
        ],
        _LEFT_HEAD + [
            ".....oaSSao.....",
            "....oaaaaaao....",
            "....oaAAaaao....",
            "....oAAaaaao....",
            "...onnbbbbbo....",
            "....oppppppo....",
            "....oppppPPo....",
            "...opppooPPPo...",
            "...oppo..oPPo...",
            "..offfo..oFFo...",
            "..offfo..oFFFo..",
            "..ooooo..ooooo..",
        ],
        _LEFT_HEAD + [
            ".....oaSSao.....",
            "....oaaaaaao....",
            "....oaaaAAao....",
            "....oaaaaAAo....",
            "....obbbbbnno...",
            "....oppppppo....",
            "....oPPppppo....",
            "...oPPPoopppo...",
            "...oPPo..oppo...",
            "..oFFFo..offo...",
            "..oFFFo..offfo..",
            "..ooooo..ooooo..",
        ],
    ],
}

# ---------------------------------------------------------------------------
# Hair styles: overlays starting at row 0 ("." keeps what is underneath).
# ---------------------------------------------------------------------------
HAIR: Dict[str, Dict[str, List[str]]] = {
    "bald": {"down": [], "up": [], "left": []},
    "buzz": {
        "down": [_E, _E, _E,
                 "....oHHHHHHo....",
                 "...oHHHHHHHHo...",
                 "...oHH....HHo..."],
        "up": [_E, _E, _E,
               "....oHHHHHHo....",
               "...oHHHHHHHHo...",
               "...oHHHHHHHHo...",
               "...oHHHHHHHHo...",
               "...oHHHHHHHHo...",
               "...oH.HHHH.Ho..."],
        "left": [_E, _E, _E,
                 "....oHHHHHHo....",
                 "...oHHHHHHHHo...",
                 "...oH.HHHHHHo...",
                 "...o...HHHHHo...",
                 "...o....HHHHo...",
                 "...o.....HHHo..."],
    },
    "short": {
        "down": [_E, _E,
                 ".....oooooo.....",
                 "....ohhhhhho....",
                 "...ohiihhhhho...",
                 "...ohhhhhhhho...",
                 "...ohh....hho...",
                 "...oh......ho..."],
        "up": [_E, _E,
               ".....oooooo.....",
               "....ohhhhhho....",
               "...ohiihhhhho...",
               "...ohhhhhhhho...",
               "...ohhhhhhhho...",
               "...ohhhhhhhho...",
               "...oHhhhhhhHo...",
               "...oHHhhhhHHo...",
               "...o.HHHHHH.o..."],
        "left": [_E, _E,
                 ".....oooooo.....",
                 "....ohhhhhho....",
                 "...ohiihhhhho...",
                 "...ohhhhhhhho...",
                 "...oh..hhhhho...",
                 "...o...hhhhho...",
                 "...o....hhhho...",
                 "...o....Hhhho...",
                 "...o.....HHHo..."],
    },
    "bob": {
        "down": [_E, _E,
                 ".....oooooo.....",
                 "....ohhhhhho....",
                 "...ohiihhhhho...",
                 "..ohhhhhhhhhho..",
                 "..ohhhh..hhhho..",
                 "..ohh......hho..",
                 "..ohh......hho..",
                 "..ohh......hho..",
                 "..oHH......HHo..",
                 "...oo......oo..."],
        "up": [_E, _E,
               ".....oooooo.....",
               "....ohhhhhho....",
               "...ohiihhhhho...",
               "..ohhhhhhhhhho..",
               "..ohhhhhhhhhho..",
               "..ohhhhhhhhhho..",
               "..ohhhhhhhhhho..",
               "..ohHhhhhhhHho..",
               "..oHHHHHHHHHHo..",
               "...oooooooooo..."],
        "left": [_E, _E,
                 ".....oooooo.....",
                 "....ohhhhhho....",
                 "...ohiihhhhho...",
                 "...ohhhhhhhhho..",
                 "...ohh.hhhhhho..",
                 "...o...hhhhhho..",
                 "...o....hhhhho..",
                 "...o....hhhhho..",
                 "...o.....HHHHo..",
                 "..........oooo.."],
    },
    "long": {
        "down": [_E, _E,
                 ".....oooooo.....",
                 "....ohhhhhho....",
                 "...ohiihhhhho...",
                 "..ohhhhhhhhhho..",
                 "..ohhh....hhho..",
                 "..ohh......hho..",
                 "..ohh......hho..",
                 "..ohh......hho..",
                 "..ohH......Hho..",
                 "..oHHo....oHHo..",
                 "..oHHo....oHHo..",
                 "...oo......oo..."],
        "up": [_E, _E,
               ".....oooooo.....",
               "....ohhhhhho....",
               "...ohiihhhhho...",
               "..ohhhhhhhhhho..",
               "..ohhhhhhhhhho..",
               "..ohhhhhhhhhho..",
               "..ohhhhhhhhhho..",
               "..ohhhhhhhhhho..",
               "..ohHhhhhhhHho..",
               "..oHHhhhhhhHHo..",
               "..oHHHhhhhHHHo..",
               "...oHHHHHHHHo...",
               "....oooooooo...."],
        "left": [_E, _E,
                 ".....oooooo.....",
                 "....ohhhhhho....",
                 "...ohiihhhhho...",
                 "...ohhhhhhhhho..",
                 "...ohh.hhhhhho..",
                 "...o...hhhhhho..",
                 "...o....hhhhho..",
                 "...o....hhhhho..",
                 "...o.....hhhHo..",
                 ".........ohHHo..",
                 ".........ohHHo..",
                 "..........oHHo..",
                 "...........oo..."],
    },
    "ponytail": {
        "down": [".......oo.......",
                 "......ohho......",
                 ".....oohhoo.....",
                 "....ohhhhhho....",
                 "...ohiihhhhho...",
                 "...ohhhhhhhho...",
                 "...ohh....hho...",
                 "...oh......ho..."],
        "up": [".......oo.......",
               "......ohho......",
               ".....oohhoo.....",
               "....ohhhhhho....",
               "...ohiihhhhho...",
               "...ohhhhhhhho...",
               "...ohhhhhhhho...",
               "...ohhhhhhhho...",
               "...oHhhhhhhHo...",
               "...oHHhhhhHHo...",
               "...o.HHhhHH.o...",
               "......ohho......",
               "......ohHo......",
               "......oHHo......",
               ".......oo......."],
        "left": [_E, _E,
                 ".....oooooo.....",
                 "....ohhhhhho....",
                 "...ohiihhhhho...",
                 "...ohhhhhhhho...",
                 "...oh..hhhhho...",
                 "...o...hhhhhoo..",
                 "...o....hhhhoho.",
                 "...o....Hhhhoho.",
                 "...o.....HHHoHo.",
                 "............oHo.",
                 ".............o.."],
    },
    "spiky": {
        "down": ["....oo.oo.oo....",
                 "...ohhohhohho...",
                 "...ohhhhhhhho...",
                 "...ohhhhhhhho...",
                 "...ohihhhihho...",
                 "...ohhhhhhhho...",
                 "...ohh....hho...",
                 "...oh......ho..."],
        "up": ["....oo.oo.oo....",
               "...ohhohhohho...",
               "...ohhhhhhhho...",
               "...ohhhhhhhho...",
               "...ohihhhihho...",
               "...ohhhhhhhho...",
               "...ohhhhhhhho...",
               "...ohhhhhhhho...",
               "...oHhhhhhhHo...",
               "...oHHhhhhHHo...",
               "...o.HHHHHH.o..."],
        "left": ["....oo.oo.oo....",
                 "...ohhohhohho...",
                 "...ohhhhhhhho...",
                 "...ohhhhhhhho...",
                 "...ohihhhihho...",
                 "...ohhhhhhhho...",
                 "...oh..hhhhho...",
                 "...o...hhhhho...",
                 "...o....hhhho...",
                 "...o....Hhhho...",
                 "...o.....HHHo..."],
    },
    "cap": {
        "down": [_E, _E,
                 ".....oooooo.....",
                 "....obbbbbbo....",
                 "...obbbbbbbbo...",
                 "...obbbbbbbbo...",
                 "..oBBBBBBBBBBo..",
                 "...oh......ho..."],
        "up": [_E, _E,
               ".....oooooo.....",
               "....obbbbbbo....",
               "...obbbbbbbbo...",
               "...obbbbbbbbo...",
               "...obbbbbbbbo...",
               "...oBBBBBBBBo...",
               "...ohhhhhhhho...",
               "...oHhhhhhhHo...",
               "...o.HHHHHH.o..."],
        "left": [_E, _E,
                 ".....oooooo.....",
                 "....obbbbbbo....",
                 "...obbbbbbbbo...",
                 "...obbbbbbbbo...",
                 ".oBBBBBBBBBBo...",
                 "..oo...hhhhho...",
                 "...o....hhhho...",
                 "...o....Hhhho...",
                 "...o.....HHHo..."],
    },
    "helmet": {
        "down": [_E,
                 ".....oooooo.....",
                 "....obbbbbbo....",
                 "...obbbbbbbbo...",
                 "..obbbbbbbbbbo..",
                 "..obbbbbbbbbbo..",
                 "..oboooooooobo..",
                 "..obovvvvvvobo..",
                 "..obovwvvvvobo..",
                 "..obovvvvvVobo..",
                 "..oboVVVVVVobo..",
                 "..oBboooooobBo..",
                 "...oBBBBBBBBo..."],
        "up": [_E,
               ".....oooooo.....",
               "....obbbbbbo....",
               "...obbbbbbbbo...",
               "..obbbbbbbbbbo..",
               "..obbbbbbbbbbo..",
               "..obbbbbbbbbbo..",
               "..obbbbbbbbbbo..",
               "..obbbbbbbbbbo..",
               "..obbbbbbbbbbo..",
               "..oBbbbbbbbbBo..",
               "..oBBbbbbbbBBo..",
               "...oBBBBBBBBo..."],
        "left": [_E,
                 ".....oooooo.....",
                 "....obbbbbbo....",
                 "...obbbbbbbbo...",
                 "..obbbbbbbbbbo..",
                 "..obbbbbbbbbbo..",
                 "..oooooobbbbbo..",
                 "..ovvvvobbbbbo..",
                 "..ovwvvobbbbbo..",
                 "..ovvvVobbbbbo..",
                 "..oVVVVobbbbBo..",
                 "..oooooobbbBBo..",
                 "...oBBBBBBBBo..."],
    },
}

EXTRA_OVERLAYS: Dict[str, Dict[str, List[str]]] = {
    "glasses": {
        "down": [_E] * 8 + [".....g.gg.g.....", ".....g....g....."],
        "up": [],
        "left": [_E] * 8 + ["....g.g.........", "....g..........."],
    },
    "beard": {
        "down": [_E] * 10 + ["....H......H....", ".....HHHHHH....."],
        "up": [],
        "left": [_E] * 10 + ["....HHHH........", ".....HHHH......."],
    },
}

# ---------------------------------------------------------------------------
# Outfits: colour role remapping plus small overlays on the body rows.
# "roles" maps template letters onto appearance keys.
# ---------------------------------------------------------------------------
OUTFITS: Dict[str, dict] = {
    "uniform": {"label": "Uniform", "roles": {}, "overlay": {}},
    "jumpsuit": {"label": "Jumpsuit", "roles": {"p": "primary"}, "overlay": {}},
    "labcoat": {
        "label": "Lab coat",
        "roles": {"a": "coat", "t": "primary"},
        "overlay": {
            "down": [_E] * 13 + [".......tt.......", ".......tt.......", ".......tt.......",
                                 _E, ".....aa..aa.....", ".....Aa..aA....."],
            "up": [_E] * 17 + [".....aaaaaa.....", ".....AaaaaA....."],
            "left": [_E] * 13 + [".....t..........", ".....t..........", ".....t..........",
                                 _E, ".....aaaaaa....."],
        },
    },
    "vest": {
        "label": "Armoured vest",
        "roles": {"n": "secondary"},
        "overlay": {
            "down": [_E] * 13 + ["....bb....bb....", "......BbbB......", "......BbbB......"],
            "up": [_E] * 13 + ["....bb....bb....", ".....bbbbbb.....", ".....BbbbbB....."],
            "left": [_E] * 13 + [".....bbbbbb....."],
        },
    },
    "suit": {
        "label": "Pressure suit",
        "roles": {"p": "primary", "n": "secondary", "f": "secondary"},
        "overlay": {
            "down": [_E] * 13 + [_E, "......BBBB......", "......BvwB......"],
            "up": [_E] * 13 + [".....BBBBBB.....", ".....BbbbbB.....", ".....BBBBBB....."],
            "left": [_E] * 13 + [".........BB.....", ".........BB....."],
        },
    },
}

HAIR_STYLES = list(HAIR.keys())
OUTFIT_STYLES = list(OUTFITS.keys())

# ---------------------------------------------------------------------------
# Drone body
# ---------------------------------------------------------------------------
_DRONE_TOP = [_E] * 5 + [".......bb.......", ".......oo.......", "....oooooooo....",
                         "...ojjaaaaaAo...", "..ojaaaaaaaaAo.."]
_DRONE_BOTTOM = ["..oaaaaaaaaaAo..", "..oAabbaabbaAo..", "...oAAAAAAAAo...", "....oooooooo....",
                 ".....oo..oo.....", ".....ff..ff.....", _E, _E, _E]
DRONE: Dict[str, List[str]] = {
    "down": _DRONE_TOP + ["..oaooooooooAo..", "..oaovvvvvvoAo..", "..oaovwvvvVoAo..",
                          "..oaovvvvvVoAo..", "..oaooooooooAo.."] + _DRONE_BOTTOM,
    "up": _DRONE_TOP + ["..oaaaaaaaaaAo..", "..oaBBBBBBBBAo..", "..oaBaBaBaBaAo..",
                        "..oaBBBBBBBBAo..", "..oaaaaaaaaaAo.."] + _DRONE_BOTTOM,
    "left": _DRONE_TOP + ["..ooooooaaaaAo..", "..ovvvvoaaaaAo..", "..owvvvoabbaAo..",
                          "..ovvvVoaaaaAo..", "..ooooooaaaaAo.."] + _DRONE_BOTTOM,
}


def normalize(appearance: Optional[dict]) -> dict:
    """Fill in missing keys and replace unknown values with defaults."""
    out = dict(DEFAULT_APPEARANCE)
    if appearance:
        out.update({k: v for k, v in appearance.items() if v is not None})
    if out["body"] not in BODIES:
        out["body"] = "human"
    if out["skin"] not in P.SKIN_TONES:
        out["skin"] = DEFAULT_APPEARANCE["skin"]
    if out["hair"] not in HAIR:
        out["hair"] = DEFAULT_APPEARANCE["hair"]
    if out["hair_color"] not in P.HAIR_COLORS:
        out["hair_color"] = DEFAULT_APPEARANCE["hair_color"]
    if out["outfit"] not in OUTFITS:
        out["outfit"] = DEFAULT_APPEARANCE["outfit"]
    for key in ("primary", "secondary", "legs"):
        if out[key] not in P.CLOTH_COLORS:
            out[key] = DEFAULT_APPEARANCE[key]
    out["extras"] = sorted(e for e in (out.get("extras") or []) if e in EXTRAS)
    return out


def appearance_key(appearance: dict) -> tuple:
    a = normalize(appearance)
    return (a["body"], a["skin"], a["hair"], a["hair_color"], a["outfit"], a["primary"],
            a["secondary"], a["legs"], tuple(a["extras"]))


def _colors(a: dict) -> Dict[str, tuple]:
    skin = P.SKIN_TONES[a["skin"]]
    hair = P.HAIR_COLORS[a["hair_color"]]
    ramps = {
        "primary": P.CLOTH_COLORS[a["primary"]],
        "secondary": P.CLOTH_COLORS[a["secondary"]],
        "legs": P.CLOTH_COLORS[a["legs"]],
        "coat": P.CLOTH_COLORS["white"],
        "boots": P.CLOTH_COLORS["black"],
        "skin": skin,
    }
    roles = {"a": "primary", "t": "primary", "b": "secondary", "p": "legs", "f": "boots", "n": "skin"}
    roles.update(OUTFITS[a["outfit"]]["roles"])
    pal: Dict[str, tuple] = {
        "o": OUTLINE,
        "s": skin[1], "S": skin[0],
        "e": (24, 20, 40),
        "h": hair[1], "H": hair[0], "i": hair[2],
        "v": P.CYAN[2], "V": P.CYAN[0], "w": P.WHITE,
        "g": (170, 214, 236),
    }
    for letter, role in roles.items():
        ramp = ramps[role]
        pal[letter] = ramp[1]
        pal[letter.upper()] = ramp[0]
    pal["j"] = ramps[roles["a"]][2]
    pal["n"] = ramps[roles["n"]][1]
    return pal


def _compose(base: Sequence[str], overlays: Sequence[Sequence[str]]) -> List[str]:
    rows = [list(r) for r in base]
    for overlay in overlays:
        for y, row in enumerate(overlay):
            if y >= len(rows):
                break
            for x, ch in enumerate(row):
                if ch != ".":
                    rows[y][x] = ch
    return ["".join(r) for r in rows]


def _human_frames(a: dict) -> Dict[str, List[List[str]]]:
    """ASCII frames per direction: [idle, step A, idle, step B]."""
    out: Dict[str, List[List[str]]] = {}
    outfit = OUTFITS[a["outfit"]]["overlay"]
    for d in ("down", "up", "left"):
        overlays = [outfit.get(d, []), HAIR[a["hair"]][d]]
        overlays += [EXTRA_OVERLAYS[e][d] for e in a["extras"]]
        frames = [_compose(f, overlays) for f in BASE[d]]
        if d == "left":
            out[d] = [frames[0], frames[1], frames[0], frames[2]]
        else:
            out[d] = [frames[0], frames[1], frames[0], [row[::-1] for row in frames[1]]]
    return out


class CharArt:
    """Builds and caches sprite sets for appearances."""

    DIRECTIONS = ("down", "up", "left", "right")

    def __init__(self):
        self._sprites: Dict[tuple, Dict[Tuple[str, int], object]] = {}
        self._portraits: Dict[tuple, object] = {}
        self._sleep: Dict[tuple, object] = {}

    def frames(self, appearance: dict) -> Dict[Tuple[str, int], object]:
        """All walk frames, keyed by (direction, frame 0..3)."""
        key = appearance_key(appearance)
        cached = self._sprites.get(key)
        if cached is None:
            cached = self._sprites[key] = self._build(normalize(appearance))
        return cached

    def sprite(self, appearance: dict, direction: str = "down", frame: int = 0):
        return self.frames(appearance)[(direction, frame % 4)]

    def _build(self, a: dict) -> Dict[Tuple[str, int], object]:
        import pygame
        from .canvas import Canvas
        pal = _colors(a)
        result: Dict[Tuple[str, int], object] = {}
        if a["body"] == "drone":
            pal = dict(pal)
            pal["f"] = P.AMBER[3]
            for d in ("down", "up", "left"):
                for f in range(4):
                    c = Canvas(W, H)
                    rows = list(DRONE[d])
                    if f in (1, 3):
                        rows[19] = ".....ww..ww....."
                    c.grid(rows, pal, 0, 1 if f in (1, 2) else 0)
                    result[(d, f)] = c.surface
        else:
            for d, frames in _human_frames(a).items():
                for f, rows in enumerate(frames):
                    c = Canvas(W, H)
                    c.grid(rows, pal)
                    result[(d, f)] = c.surface
        for f in range(4):
            result[("right", f)] = pygame.transform.flip(result[("left", f)], True, False)
        return result

    def sleeping(self, appearance: dict):
        """Just the head, eyes closed - drawn on a pillow while asleep."""
        key = appearance_key(appearance)
        surf = self._sleep.get(key)
        if surf is None:
            import pygame
            from .canvas import Canvas
            a = normalize(appearance)
            if a["body"] == "drone":
                surf = self.sprite(a, "down", 0)
            else:
                rows = _human_frames(a)["down"][0][:12]
                rows = [r for r in rows]
                rows[8] = rows[8].replace("e", "s")
                rows[9] = rows[9].replace("e", "S")
                c = Canvas(W, 13)
                c.grid(rows, _colors(a))
                c.hline(4, 12, 8, OUTLINE)
                surf = c.surface
            self._sleep[key] = surf
        return surf

    # -- portraits -----------------------------------------------------------
    def portrait(self, appearance: dict):
        key = appearance_key(appearance)
        surf = self._portraits.get(key)
        if surf is None:
            a = normalize(appearance)
            surf = _drone_portrait(a) if a["body"] == "drone" else _human_portrait(a)
            self._portraits[key] = surf
        return surf


def _human_portrait(a: dict):
    from .canvas import Canvas
    c = Canvas(PORTRAIT, PORTRAIT)
    skin = P.SKIN_TONES[a["skin"]]
    hair = P.HAIR_COLORS[a["hair_color"]]
    outfit = a["outfit"]
    top = P.CLOTH_COLORS["white"] if outfit == "labcoat" else P.CLOTH_COLORS[a["primary"]]
    trim = P.CLOTH_COLORS[a["secondary"]]
    style = a["hair"]
    o = OUTLINE

    # hair that falls behind the shoulders
    if style == "long":
        c.rect(5, 10, 22, 20, o)
        c.rect(6, 10, 20, 19, hair[0])
        c.vline(7, 12, 16, hair[1])
        c.vline(24, 12, 16, hair[1])
    elif style == "bob":
        c.rect(5, 9, 22, 13, o)
        c.rect(6, 9, 20, 12, hair[0])
    elif style == "ponytail":
        c.rect(22, 12, 6, 14, o)
        c.rect(23, 13, 4, 12, hair[0])
        c.vline(24, 14, 9, hair[1])

    # shoulders
    c.ellipse(2, 23, 28, 18, o)
    c.ellipse(3, 24, 26, 16, top[1])
    c.ellipse(3, 24, 12, 16, top[2])
    c.ellipse(6, 25, 20, 14, top[1])
    c.rect(21, 26, 7, 6, top[0])
    if outfit == "labcoat":
        shirt = P.CLOTH_COLORS[a["primary"]]
        c.poly([(12, 24), (20, 24), (16, 31)], shirt[1])
        c.line(12, 24, 16, 31, top[0])
        c.line(20, 24, 16, 31, top[0])
    elif outfit == "vest":
        c.rect(3, 26, 7, 3, trim[1])
        c.rect(22, 26, 7, 3, trim[0])
        c.rect(11, 27, 10, 5, trim[0])
        c.hline(11, 27, 10, trim[1])
    elif outfit == "suit":
        c.ellipse(9, 22, 14, 6, o)
        c.ellipse(10, 23, 12, 4, trim[1])
        c.hline(11, 24, 10, trim[2])
    else:
        c.poly([(12, 24), (16, 28), (20, 24)], trim[1])
        c.line(12, 24, 16, 28, trim[0])
        if outfit == "jumpsuit":
            c.vline(16, 28, 4, top[0])
    # neck
    c.rect(13, 20, 6, 5, skin[0])
    c.rect(13, 20, 3, 4, skin[1])

    # head
    c.ellipse(7, 3, 18, 21, o)
    c.ellipse(8, 4, 16, 19, skin[1])
    c.ellipse(8, 4, 9, 19, skin[1])
    for y in range(6, 22):                      # shade the right side of the face
        w = 3 if 9 < y < 19 else 2
        for x in range(23 - w, 24):
            if c.get(x, y)[:3] == skin[1]:
                c.px(x, y, skin[0])
    c.rect(11, 6, 5, 2, skin[2])
    c.rect(7, 12, 1, 4, skin[0])                # ears
    c.rect(24, 12, 1, 4, skin[0])
    # features
    eye = (30, 26, 48)
    for ex in (11, 18):
        c.rect(ex, 13, 3, 2, P.WHITE)
        c.rect(ex + 1, 13, 1, 2, eye)
        c.hline(ex, 11, 3, hair[0])
    c.px(16, 15, skin[0])
    c.px(16, 16, skin[0])
    c.px(15, 17, skin[0])
    c.hline(14, 19, 4, P.lerp(skin[0], P.RED[1], 0.5))
    c.px(13, 18, skin[0])
    c.px(18, 18, skin[0])

    # hair on top of the head
    if style == "buzz":
        c.ellipse(8, 3, 16, 8, hair[0])
        c.dither(9, 8, 14, 2, hair[0])
    elif style in ("short", "ponytail", "spiky", "bob", "long"):
        c.ellipse(7, 2, 18, 11, o)
        c.ellipse(8, 3, 16, 9, hair[1])
        c.rect(8, 8, 2, 6 if style in ("short", "ponytail", "spiky") else 12, hair[1])
        c.rect(22, 8, 2, 6 if style in ("short", "ponytail", "spiky") else 12, hair[0])
        c.rect(10, 9, 5, 2, hair[1])
        c.rect(15, 9, 3, 1, hair[1])
        c.line(11, 5, 15, 4, hair[2])
        c.px(18, 5, hair[2])
        if style == "spiky":
            for x in (9, 13, 17, 21):
                c.poly([(x - 2, 4), (x, 0), (x + 2, 4)], hair[1])
                c.line(x - 2, 4, x, 0, o)
                c.line(x + 2, 4, x, 0, o)
        if style == "ponytail":
            c.ellipse(13, 0, 7, 5, o)
            c.ellipse(14, 1, 5, 3, hair[1])
        if style in ("bob", "long"):
            c.vline(7, 8, 12, o)
            c.vline(24, 8, 12, o)
    elif style == "cap":
        c.ellipse(7, 2, 18, 10, o)
        c.ellipse(8, 3, 16, 8, trim[1])
        c.rect(6, 9, 20, 3, o)
        c.rect(7, 9, 18, 2, trim[0])
        c.hline(9, 4, 6, trim[2])
        c.rect(8, 11, 1, 3, hair[1])
        c.rect(23, 11, 1, 3, hair[0])
    elif style == "helmet":
        c.ellipse(5, 1, 22, 23, o)
        c.ellipse(6, 2, 20, 21, trim[1])
        c.ellipse(6, 2, 10, 21, trim[2])
        c.ellipse(9, 4, 14, 17, trim[1])
        c.rect(8, 9, 16, 10, o)
        c.rect(9, 10, 14, 8, P.CYAN[1])
        c.rect(9, 10, 14, 3, P.CYAN[2])
        c.line(11, 15, 15, 11, P.CYAN[4])
        c.line(14, 16, 16, 14, P.CYAN[3])

    if "beard" in a["extras"] and style != "helmet":
        for y in range(17, 23):
            for x in range(9, 23):
                if c.get(x, y)[:3] in (skin[1], skin[0]) and (y >= 20 or x < 12 or x > 19):
                    c.px(x, y, hair[1] if (x + y) % 3 else hair[0])
        c.hline(13, 18, 6, hair[0])
    if "glasses" in a["extras"] and style != "helmet":
        for ex in (10, 17):
            c.frame_rect(ex, 12, 5, 4, (40, 44, 62))
            c.px(ex + 1, 13, (190, 226, 244))
        c.hline(15, 13, 2, (40, 44, 62))
    return c.surface


def _drone_portrait(a: dict):
    from .canvas import Canvas
    c = Canvas(PORTRAIT, PORTRAIT)
    shell = P.CLOTH_COLORS[a["primary"]]
    trim = P.CLOTH_COLORS[a["secondary"]]
    o = OUTLINE
    c.vline(16, 1, 5, o)
    c.rect(15, 0, 3, 2, trim[2])
    c.rect(4, 6, 24, 22, o)
    c.rect(5, 7, 22, 20, shell[1])
    c.rect(5, 7, 22, 3, shell[2])
    c.rect(24, 10, 3, 17, shell[0])
    c.rect(7, 12, 18, 9, o)
    c.rect(8, 13, 16, 7, P.CYAN[0])
    c.circle(16, 16, 3, P.CYAN[2])
    c.circle(16, 16, 1.5, P.CYAN[4])
    c.px(14, 14, P.WHITE)
    for x in (9, 12, 20, 23):
        c.px(x, 23, trim[1])
    c.hline(10, 25, 12, shell[0])
    c.rect(1, 13, 3, 6, o)
    c.rect(28, 13, 3, 6, o)
    c.rect(2, 14, 1, 4, trim[1])
    c.rect(29, 14, 1, 4, trim[1])
    return c.surface


def validate_templates() -> List[str]:
    """Sanity-check the ASCII tables; returns a list of problems (tests)."""
    problems: List[str] = []

    def check(name, rows, exact=None):
        if exact is not None and len(rows) != exact:
            problems.append("%s: %d rows (expected %d)" % (name, len(rows), exact))
        for i, row in enumerate(rows):
            if len(row) != W:
                problems.append("%s row %d: width %d" % (name, i, len(row)))

    for d, frames in BASE.items():
        for i, rows in enumerate(frames):
            check("base %s %d" % (d, i), rows, H)
    for style, dirs in HAIR.items():
        for d, rows in dirs.items():
            check("hair %s %s" % (style, d), rows)
    for name, dirs in EXTRA_OVERLAYS.items():
        for d, rows in dirs.items():
            check("extra %s %s" % (name, d), rows)
    for name, spec in OUTFITS.items():
        for d, rows in spec["overlay"].items():
            check("outfit %s %s" % (name, d), rows)
    for d, rows in DRONE.items():
        check("drone %s" % d, rows, H)
    return problems
