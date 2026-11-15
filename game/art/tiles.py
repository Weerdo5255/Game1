"""Tile registry and procedural tile art.

Every tile the maps can use is declared here with the ``@tile`` decorator: a
little metadata (is it solid, which editor category, does it glow ...) plus a
function that paints its 16x16 picture onto a :class:`Canvas`.  Nothing is
loaded from image files.

The registry itself (``TILES`` / ``PROPS``) has no pygame dependency, so the
world simulation can ask "is this tile solid?" in a headless process.  Only
:class:`TileArt`, which actually rasterises tiles, needs pygame.

Adding a tile
-------------
::

    @tile("my_tile", "My tile", "decor", layer="decor")
    def _my_tile(c):
        c.fill(S[2])
        c.px(8, 8, CY[3])

It immediately shows up in the map editor palette.  Large multi-tile objects
are declared with ``@prop`` in ``props.py``.
"""
from __future__ import annotations

import zlib
from dataclasses import dataclass, field
from typing import Callable, Dict, List, Optional, Tuple

from ..engine import palette as P

S = P.STEEL
NV = P.NAVY
CY = P.CYAN
GR = P.GREEN
AM = P.AMBER
RD = P.RED
PU = P.PURPLE
BR = P.BROWN
IC = P.ICE
RK = P.ROCK
BG = P.BEIGE
BLACK = P.BLACK
WHITE = P.WHITE

# autotile neighbour bits: set when the neighbour belongs to the same group
N, E, SO, W, NE, SE, SW, NW = 1, 2, 4, 8, 16, 32, 64, 128
NEIGHBOURS = ((0, -1, N), (1, 0, E), (0, 1, SO), (-1, 0, W),
              (1, -1, NE), (1, 1, SE), (-1, 1, SW), (-1, -1, NW))

CATEGORIES = ["floor", "wall", "door", "furniture", "machine", "nature", "decor", "sign", "overhead", "prop"]


@dataclass
class TileDef:
    name: str
    label: str
    category: str
    layer: str                      # layer the editor suggests for this tile
    solid: bool
    draw: Callable
    frames: int = 1                 # animation frames
    fps: float = 0.0                # 0 = frame is chosen by game logic, not time
    variants: int = 1               # random look-alikes picked per map cell
    group: str = ""                 # autotile group ("" = none)
    light: Optional[Tuple[int, Tuple[int, int, int]]] = None  # (radius px, colour)
    tags: Tuple[str, ...] = ()
    prop: str = ""                  # owning multi-tile prop, if any
    hidden: bool = False            # prop parts are hidden from the palette


@dataclass
class PropDef:
    """A multi-tile object: placing it stamps several tiles at once."""
    name: str
    label: str
    category: str
    w: int
    h: int
    parts: List[Tuple[int, int, str, str]] = field(default_factory=list)  # dx, dy, layer, tile


TILES: Dict[str, TileDef] = {}
PROPS: Dict[str, PropDef] = {}


def tile(name: str, label: str, category: str, layer: str = "objects", solid: bool = False, **kw):
    """Register a tile; decorates its drawing function."""
    def deco(fn):
        TILES[name] = TileDef(name=name, label=label, category=category, layer=layer,
                              solid=solid, draw=fn, **kw)
        return fn
    return deco


def is_solid(name: str) -> bool:
    t = TILES.get(name)
    return bool(t and t.solid)


def group_of(name: str) -> str:
    t = TILES.get(name)
    return t.group if t else ""


def stable_hash(*parts) -> int:
    """Process-independent hash (Python's ``hash`` is salted for strings)."""
    return zlib.crc32(":".join(str(p) for p in parts).encode("utf8"))


def cell_variant(name: str, x: int, y: int) -> int:
    """Which random variant of ``name`` the map cell (x, y) shows."""
    t = TILES.get(name)
    if not t or t.variants <= 1:
        return 0
    return ((x * 73856093) ^ (y * 19349663)) % t.variants


# ===========================================================================
# Floors
# ===========================================================================
@tile("floor_plate", "Deck plate", "floor", layer="ground", variants=9)
def _floor_plate(c):
    c.fill(S[2])
    hi = P.lerp(S[2], S[3], 0.45)
    c.hline(0, 0, 16, hi)
    c.vline(0, 0, 16, hi)
    c.hline(0, 15, 16, S[1])
    c.vline(15, 0, 16, S[1])
    for x, y in ((2, 2), (12, 2), (2, 12), (12, 12)):
        c.px(x, y, S[1])
        c.px(x + 1, y + 1, hi)
    if c.variant == 1:
        c.line(5, 9, 9, 6, hi)
    elif c.variant == 2:
        c.speckle(3, 3, 10, 10, [S[1]], 4)
    elif c.variant == 3:
        c.rect(6, 7, 4, 2, S[1])
        c.hline(6, 9, 4, hi)


@tile("floor_grate", "Grating", "floor", layer="ground")
def _floor_grate(c):
    c.fill(S[0])
    for y in range(0, 16, 4):
        c.hline(0, y, 16, S[3])
        c.hline(0, y + 1, 16, S[1])
    for x in (0, 8):
        c.vline(x, 0, 16, S[2])
        c.vline(x + 1, 0, 16, S[1])


@tile("floor_hab", "Habitat floor", "floor", layer="ground", variants=3)
def _floor_hab(c):
    a, b, seam, hi = (58, 80, 96), (66, 92, 108), (46, 64, 80), (80, 108, 122)
    c.fill(b)
    c.rect(0, 0, 8, 8, a)
    c.rect(8, 8, 8, 8, a)
    c.hline(0, 7, 16, seam)
    c.hline(0, 15, 16, seam)
    c.vline(7, 0, 16, seam)
    c.vline(15, 0, 16, seam)
    c.px(0, 0, hi)
    c.px(8, 8, hi)
    if c.variant == 1:
        c.px(11, 3, hi)
    elif c.variant == 2:
        c.px(3, 11, seam)


@tile("floor_lab", "Lab tile", "floor", layer="ground", variants=3)
def _floor_lab(c):
    c.fill(S[4])
    for o in (0, 8):
        c.hline(0, o + 7, 16, S[3])
        c.vline(o + 7, 0, 16, S[3])
        c.hline(o, o, 3, S[5])
        c.vline(o, o, 3, S[5])
    c.hline(8, 0, 3, S[5])
    c.hline(0, 8, 3, S[5])
    if c.variant == 1:
        c.px(4, 12, S[3])
    elif c.variant == 2:
        c.px(12, 4, S[5])


@tile("floor_dark", "Service floor", "floor", layer="ground", variants=3)
def _floor_dark(c):
    c.fill(S[1])
    c.hline(0, 15, 16, S[0])
    c.vline(15, 0, 16, S[0])
    hi = P.lerp(S[1], S[2], 0.6)
    for y in range(2, 16, 4):
        for x in range(2 if (y // 4) % 2 else 4, 16, 4):
            c.px(x, y, hi)
            c.px(x + 1, y + 1, hi)
    if c.variant == 1:
        c.rect(5, 6, 3, 2, S[0])
    elif c.variant == 2:
        c.px(10, 10, S[2])


@tile("floor_hazard", "Hazard stripes", "floor", layer="ground")
def _floor_hazard(c):
    for y in range(16):
        for x in range(16):
            c.px(x, y, AM[2] if ((x + y) // 4) % 2 == 0 else S[0])
    c.hline(0, 0, 16, AM[3])
    c.hline(0, 15, 16, S[0])


@tile("floor_pad", "Landing pad", "floor", layer="ground")
def _floor_pad(c):
    c.fill(S[1])
    line = P.lerp(S[1], S[0], 0.6)
    for o in (0, 8):
        c.hline(0, o, 16, line)
        c.vline(o, 0, 16, line)
    c.px(4, 4, S[2])
    c.px(12, 12, S[2])


@tile("floor_threshold", "Dark threshold", "floor", layer="ground")
def _floor_threshold(c):
    c.fill(BLACK)
    c.hline(0, 15, 16, S[1])
    c.dither(0, 12, 16, 3, S[0])


@tile("floor_glass", "Glass floor", "floor", layer="ground")
def _floor_glass(c):
    c.fill((30, 44, 90, 70))
    c.frame_rect(0, 0, 16, 16, S[3])
    c.px(0, 0, S[4])
    c.line(3, 6, 6, 3, (200, 230, 255, 110))
    c.line(8, 12, 12, 8, (200, 230, 255, 70))


@tile("ice_floor", "Ice floor", "floor", layer="ground", variants=5, tags=("cold",))
def _ice_floor(c):
    base = P.lerp(IC[1], IC[2], 0.45)
    c.fill(base)
    c.speckle(0, 0, 16, 16, [IC[1], P.lerp(IC[1], IC[2], 0.7)], 10)
    if c.variant in (1, 3):
        x = c.rng.randrange(2, 9)
        c.line(x, 2, x + 5, 7, IC[3])
        c.px(x + 6, 8, IC[4])
    if c.variant in (2, 3):
        c.line(3, 12, 8, 13, IC[3])
    if c.variant == 4:
        c.rect(9, 4, 3, 2, IC[1])
    c.px(c.rng.randrange(16), c.rng.randrange(16), IC[4])


@tile("rock_floor", "Rock floor", "floor", layer="ground", variants=5)
def _rock_floor(c):
    c.fill(RK[2])
    c.speckle(0, 0, 16, 16, [RK[1], P.lerp(RK[2], RK[3], 0.5)], 14)
    for _ in range(1 + c.variant % 3):
        x, y = c.rng.randrange(1, 13), c.rng.randrange(1, 13)
        c.rect(x, y, 2, 1, RK[3])
        c.rect(x, y + 1, 2, 1, RK[1])


@tile("snow_floor", "Frost", "floor", layer="ground", variants=4, tags=("cold",))
def _snow_floor(c):
    c.fill(IC[4])
    c.speckle(0, 0, 16, 16, [IC[3]], 8 + c.variant * 2)
    if c.variant == 2:
        c.dither(4, 9, 7, 3, IC[3])


@tile("rug", "Rug", "decor", layer="decor", group="rug")
def _rug(c):
    c.fill((104, 36, 62))
    c.dither(0, 0, 16, 16, (92, 30, 56))
    m = c.mask
    edge, trim = (62, 18, 40), AM[2]
    if not m & N:
        c.hline(0, 0, 16, edge)
        c.hline(1 if not m & W else 0, 2, 16, trim)
    if not m & SO:
        c.hline(0, 15, 16, edge)
        c.hline(0, 13, 16, trim)
    if not m & W:
        c.vline(0, 0, 16, edge)
        c.vline(2, 0 if m & N else 2, 16 if m & SO else 12, trim)
    if not m & E:
        c.vline(15, 0, 16, edge)
        c.vline(13, 0 if m & N else 2, 16 if m & SO else 12, trim)


# ===========================================================================
# Walls (autotiled).  A wall cell with open floor to its south shows its
# front face; otherwise we are looking down at its roof.
# ===========================================================================
def _roof(c, top, edge, speck=None):
    m = c.mask
    c.fill(top)
    if speck:
        c.speckle(0, 0, 16, 16, [speck], 6)
    if not m & N:
        c.hline(0, 0, 16, edge)
    if not m & W:
        c.vline(0, 0, 16, edge)
    if not m & E:
        c.vline(15, 0, 16, edge)
    if not m & SO:
        c.hline(0, 15, 16, edge)
    if (m & N) and (m & E) and not m & NE:
        c.px(15, 0, edge)
    if (m & N) and (m & W) and not m & NW:
        c.px(0, 0, edge)
    if (m & SO) and (m & E) and not m & SE:
        c.px(15, 15, edge)
    if (m & SO) and (m & W) and not m & SW:
        c.px(0, 15, edge)


def _wall(c, top, edge, lip, face, face_lo, base, trim=None, speck=None, detail=None):
    """Shared wall painter.  ``detail(c)`` may decorate the face."""
    m = c.mask
    if m & SO:
        _roof(c, top, edge, speck)
        return
    # roof strip along the top of the tile, then the front face below it
    c.rect(0, 0, 16, 5, top)
    if not m & N:
        c.hline(0, 0, 16, edge)
    c.hline(0, 5, 16, lip)
    c.rect(0, 6, 16, 7, face)
    c.rect(0, 13, 16, 2, face_lo)
    c.hline(0, 15, 16, base)
    c.vline(15, 6, 9, face_lo)
    if trim:
        c.hline(0, 8, 16, trim)
    if detail:
        detail(c)
    if not m & W:
        c.vline(0, 0, 5, edge)
        c.vline(0, 5, 11, lip)
    if not m & E:
        c.vline(15, 0, 5, edge)
        c.vline(15, 5, 11, base)


@tile("wall", "Bulkhead", "wall", layer="ground", solid=True, group="wall", variants=4)
def _wall_steel(c):
    def detail(c):
        if c.variant == 1:
            for y in (10, 12):
                c.hline(4, y, 7, S[1])
        elif c.variant == 2:
            c.rect(10, 10, 3, 2, S[1])
            c.px(11, 10, CY[3])
    _wall(c, S[0], S[2], S[4], S[2], S[1], BLACK, trim=CY[1], detail=detail)


@tile("wall_hab", "Habitat wall", "wall", layer="ground", solid=True, group="wall", variants=3)
def _wall_hab(c):
    def detail(c):
        if c.variant == 1:
            c.rect(5, 10, 6, 2, BG[1])
    _wall(c, (36, 34, 48), (72, 68, 86), BG[4], BG[2], BG[1], (40, 34, 40), trim=AM[2], detail=detail)


@tile("wall_lab", "Lab wall", "wall", layer="ground", solid=True, group="wall", variants=3)
def _wall_lab(c):
    def detail(c):
        if c.variant == 1:
            c.px(4, 11, CY[2])
            c.px(6, 11, CY[2])
    _wall(c, S[1], S[3], WHITE, S[5], S[4], S[2], trim=CY[2], detail=detail)


@tile("wall_dark", "Machine wall", "wall", layer="ground", solid=True, group="wall", variants=3)
def _wall_dark(c):
    def detail(c):
        for x in range(0, 16, 4):
            c.rect(x, 8, 2, 1, AM[2])
        if c.variant == 1:
            c.rect(3, 10, 2, 3, S[0])
            c.rect(7, 10, 2, 3, S[0])
            c.rect(11, 10, 2, 3, S[0])
    _wall(c, BLACK, S[1], S[3], S[1], S[0], BLACK, trim=S[0], detail=detail)


@tile("wall_ice", "Ice wall", "wall", layer="ground", solid=True, group="wall", variants=4, tags=("cold",))
def _wall_ice(c):
    def detail(c):
        c.speckle(0, 6, 16, 9, [IC[3], IC[1]], 9)
        x = 2 + c.rng.randrange(9)
        c.line(x, 6, x + 2, 12, IC[4])
        for i in range(0, 16, 3):
            c.px(i + c.rng.randrange(2), 14, IC[0])
    _wall(c, (22, 36, 70), IC[1], IC[4], IC[2], IC[1], IC[0], speck=(30, 50, 92), detail=detail)


@tile("wall_rock", "Rock wall", "wall", layer="ground", solid=True, group="wall", variants=4)
def _wall_rock(c):
    def detail(c):
        c.speckle(0, 6, 16, 9, [RK[3], RK[1]], 10)
        x = 2 + c.rng.randrange(9)
        c.rect(x, 9, 3, 1, RK[4])
        c.rect(x, 10, 3, 1, RK[1])
    _wall(c, RK[0], RK[2], RK[4], RK[2], RK[1], BLACK, speck=RK[1], detail=detail)


@tile("window", "Viewport", "wall", layer="ground", solid=True, group="wall")
def _window(c):
    m = c.mask
    if m & SO:
        _roof(c, S[0], S[2])
        return
    _wall(c, S[0], S[2], S[4], S[2], S[1], BLACK)
    # glass: transparent so the starfield behind the map shows through
    c.rect(0, 6, 16, 9, S[3])
    c.clear(0, 7, 16, 7)
    c.rect(0, 7, 16, 7, (40, 70, 130, 60))
    c.vline(15, 6, 9, S[2])
    c.line(3, 12, 7, 8, (220, 240, 255, 120))
    c.line(6, 12, 8, 10, (220, 240, 255, 70))
    if not m & W:
        c.vline(0, 5, 11, S[4])
    if not m & E:
        c.vline(15, 5, 11, BLACK)


# ===========================================================================
# Doors.  Frame 0 closed, 1 half open, 2 open (chosen by the door's state).
# ===========================================================================
def _door_face(c, panel, panel_lo, stripe):
    gap = (0, 3, 6)[min(c.frame, 2)]
    c.rect(0, 0, 16, 5, S[0])
    c.hline(0, 0, 16, S[2])
    c.hline(0, 5, 16, S[4])
    c.rect(0, 6, 16, 10, BLACK)
    if gap >= 6:
        c.clear(2, 6, 12, 10)
    lw = 8 - gap
    for x0 in (0, 16 - lw):
        c.rect(x0, 6, lw, 10, panel)
        c.rect(x0, 13, lw, 3, panel_lo)
        if lw >= 5:
            c.rect(x0 + 1 if x0 == 0 else x0 + lw - 4, 9, 3, 1, stripe)
    if gap == 0:
        c.vline(7, 6, 10, S[1])
        c.vline(8, 6, 10, S[4])
    c.hline(0, 15, 16, BLACK)
    if gap >= 6:
        c.clear(2, 15, 12, 1)


@tile("door", "Door (north wall)", "door", solid=False, frames=3)
def _door(c):
    _door_face(c, S[3], S[2], CY[3])


@tile("door_secure", "Secure door", "door", solid=False, frames=3)
def _door_secure(c):
    _door_face(c, (150, 96, 60), (112, 70, 50), AM[3])


@tile("door_v", "Door (side wall)", "door", solid=False, frames=3)
def _door_v(c):
    retract = (0, 4, 6)[min(c.frame, 2)]
    c.rect(5, 0, 6, 2, S[1])
    c.rect(5, 14, 6, 2, S[1])
    for y0, h in ((0, 8 - retract), (8 + retract, 8 - retract)):
        if h > 0:
            c.rect(6, y0, 4, h, S[3])
            c.vline(6, y0, h, S[4])
            c.vline(9, y0, h, S[2])
    if retract == 0:
        c.hline(6, 7, 4, CY[3])
        c.hline(6, 8, 4, CY[1])


@tile("door_h", "Door (south wall)", "door", solid=False, frames=3)
def _door_h(c):
    retract = (0, 4, 6)[min(c.frame, 2)]
    c.rect(0, 5, 2, 6, S[1])
    c.rect(14, 5, 2, 6, S[1])
    for x0, w in ((0, 8 - retract), (8 + retract, 8 - retract)):
        if w > 0:
            c.rect(x0, 6, w, 4, S[3])
            c.hline(x0, 6, w, S[4])
            c.hline(x0, 9, w, S[2])
    if retract == 0:
        c.vline(7, 6, 4, CY[3])
        c.vline(8, 6, 4, CY[1])


# ===========================================================================
# Furniture
# ===========================================================================
@tile("crate", "Cargo crate", "furniture", solid=True, variants=3)
def _crate(c):
    accent = (AM[2], CY[2], RD[2])[c.variant]
    c.rect(1, 2, 14, 13, BLACK)
    c.rect(2, 3, 12, 6, S[4])
    c.hline(2, 3, 12, S[5])
    c.rect(2, 9, 12, 5, S[2])
    c.hline(2, 9, 12, S[1])
    c.rect(7, 3, 2, 11, S[3])
    c.rect(3, 11, 3, 2, accent)
    c.px(12, 11, S[4])
    c.px(12, 12, S[1])


@tile("crate_stack", "Stacked crates", "furniture", solid=True)
def _crate_stack(c):
    c.rect(0, 0, 16, 16, BLACK)
    c.rect(1, 1, 14, 5, BR[3])
    c.hline(1, 1, 14, BR[4])
    c.rect(1, 6, 14, 3, BR[2])
    c.rect(1, 10, 7, 5, S[2])
    c.rect(9, 10, 6, 5, S[3])
    c.hline(1, 10, 14, S[4])
    c.vline(8, 10, 5, BLACK)
    c.hline(1, 9, 14, BLACK)
    c.rect(3, 7, 4, 1, AM[2])
    c.rect(11, 12, 2, 2, CY[2])


@tile("barrel", "Drum", "furniture", solid=True, variants=3)
def _barrel(c):
    body = (CY, AM, GR)[c.variant]
    c.ellipse(3, 2, 10, 13, BLACK)
    c.rect(3, 5, 10, 8, BLACK)
    c.rect(4, 5, 8, 8, body[1])
    c.vline(4, 5, 8, body[0])
    c.vline(5, 5, 8, body[2])
    c.vline(11, 5, 8, body[0])
    c.ellipse(4, 2, 8, 5, S[4])
    c.ellipse(5, 3, 6, 3, S[3])
    c.hline(4, 8, 8, body[0])
    c.hline(5, 13, 6, body[0])
    c.rect(7, 10, 2, 2, WHITE)


@tile("table", "Table", "furniture", solid=True, group="table")
def _table(c):
    m = c.mask
    x0 = 0 if m & W else 1
    x1 = 16 if m & E else 15
    c.rect(x0, 3, x1 - x0, 10, BLACK)
    c.rect(x0 + (0 if m & W else 1), 4, x1 - x0 - (0 if m & W else 1) - (0 if m & E else 1), 6, BG[3])
    c.hline(x0 + (0 if m & W else 1), 4, x1 - x0 - (0 if m & W else 1) - (0 if m & E else 1), BG[4])
    c.rect(x0 + (0 if m & W else 1), 10, x1 - x0 - (0 if m & W else 1) - (0 if m & E else 1), 2, BG[1])
    if not m & W:
        c.rect(2, 13, 2, 2, S[1])
    if not m & E:
        c.rect(12, 13, 2, 2, S[1])


@tile("desk", "Desk", "furniture", solid=True, group="desk")
def _desk(c):
    m = c.mask
    c.rect(0 if m & W else 1, 2, 16 - (0 if m & W else 1) - (0 if m & E else 1), 13, BLACK)
    x0 = 0 if m & W else 2
    w = 16 - x0 - (0 if m & E else 2)
    c.rect(x0, 3, w, 5, S[3])
    c.hline(x0, 3, w, S[4])
    c.rect(x0, 8, w, 6, S[1])
    c.hline(x0, 8, w, S[2])
    c.rect(x0 + 2, 10, max(1, w - 4), 1, CY[1])


@tile("chair", "Chair", "furniture", solid=False, tags=("seat",))
def _chair(c):
    c.rect(4, 3, 8, 11, BLACK)
    c.rect(5, 4, 6, 4, CY[1])
    c.hline(5, 4, 6, CY[2])
    c.rect(5, 8, 6, 3, S[2])
    c.hline(5, 8, 6, S[3])
    c.rect(5, 11, 1, 2, S[1])
    c.rect(10, 11, 1, 2, S[1])


@tile("stool", "Stool", "furniture", solid=False, tags=("seat",))
def _stool(c):
    c.ellipse(4, 5, 8, 6, BLACK)
    c.ellipse(5, 6, 6, 4, AM[2])
    c.hline(6, 6, 4, AM[3])
    c.rect(7, 10, 2, 4, S[1])
    c.hline(5, 14, 6, S[0])


@tile("bench", "Bench", "furniture", solid=True, group="bench")
def _bench(c):
    m = c.mask
    x0 = 0 if m & W else 1
    w = 16 - x0 - (0 if m & E else 1)
    c.rect(x0, 6, w, 7, BLACK)
    c.rect(x0 + (0 if m & W else 1), 7, w - (0 if m & W else 1) - (0 if m & E else 1), 3, BR[3])
    c.rect(x0 + (0 if m & W else 1), 10, w - (0 if m & W else 1) - (0 if m & E else 1), 2, BR[1])
    if not m & W:
        c.rect(2, 13, 1, 2, S[0])
    if not m & E:
        c.rect(13, 13, 1, 2, S[0])


@tile("planter", "Planter", "furniture", solid=True, frames=2, fps=1.2, variants=3)
def _planter(c):
    c.rect(1, 6, 14, 9, BLACK)
    c.rect(2, 7, 12, 3, BR[1])
    c.hline(2, 7, 12, BR[0])
    c.rect(2, 10, 12, 4, S[2])
    c.hline(2, 10, 12, S[3])
    c.rect(3, 12, 3, 1, CY[2])
    sway = c.frame
    for i, x in enumerate((3, 6, 9, 12)):
        tall = (4, 6, 5, 7)[(i + c.variant) % 4]
        lean = sway if i % 2 == 0 else -sway
        for k in range(tall):
            c.px(x + (lean if k > tall // 2 else 0), 7 - k, GR[2] if k % 2 else GR[1])
        tip_x = x + lean
        c.px(tip_x - 1, 8 - tall, GR[3])
        c.px(tip_x + 1, 8 - tall, GR[3])
        c.px(tip_x, 7 - tall, GR[4])
        if c.variant == 2 and i % 2 == 0:
            c.px(tip_x, 9 - tall, RD[3])
        if c.variant == 1 and i % 2 == 1:
            c.px(tip_x + 1, 10 - tall, AM[3])


@tile("pot_plant", "Potted plant", "furniture", solid=True)
def _pot_plant(c):
    c.rect(5, 10, 6, 5, BLACK)
    c.rect(6, 11, 4, 3, BR[2])
    c.hline(6, 11, 4, BR[3])
    for x, y, col in ((7, 9, GR[1]), (8, 8, GR[2]), (6, 7, GR[2]), (9, 6, GR[3]), (5, 5, GR[3]),
                      (10, 4, GR[2]), (7, 4, GR[3]), (8, 3, GR[4]), (4, 6, GR[1]), (11, 6, GR[1]),
                      (7, 6, GR[1]), (8, 5, GR[2]), (9, 8, GR[1]), (6, 9, GR[1])):
        c.px(x, y, col)
    c.px(6, 3, GR[2])
    c.px(10, 3, GR[3])


# ===========================================================================
# Machines and consoles
# ===========================================================================
def _console(c, glow, glow_hi, body=S):
    c.rect(1, 2, 14, 13, BLACK)
    c.rect(2, 3, 12, 6, body[1])
    c.rect(3, 4, 10, 4, NV[0])
    # scrolling read-out
    for i in range(3):
        y = 4 + (i + c.frame) % 4
        w = (7, 4, 8)[(i + c.frame) % 3]
        c.hline(4, y, w, glow if i else glow_hi)
    c.rect(2, 9, 12, 2, body[3])
    c.hline(2, 9, 12, body[4])
    for i, x in enumerate(range(3, 13, 2)):
        c.px(x, 10, glow_hi if (i + c.frame) % 4 == 0 else body[1])
    c.rect(2, 11, 12, 3, body[2])
    c.hline(2, 13, 12, body[1])


@tile("console", "Console", "machine", solid=True, frames=4, fps=3, light=(22, CY[2]))
def _console_cyan(c):
    _console(c, CY[2], CY[4])


@tile("console_amber", "Console (amber)", "machine", solid=True, frames=4, fps=3, light=(22, AM[2]))
def _console_amber(c):
    _console(c, AM[2], AM[4])


@tile("console_green", "Console (green)", "machine", solid=True, frames=4, fps=3, light=(22, GR[3]))
def _console_green(c):
    _console(c, GR[3], GR[4])


@tile("console_red", "Console (alert)", "machine", solid=True, frames=4, fps=5, light=(22, RD[2]))
def _console_red(c):
    _console(c, RD[2], RD[4])


@tile("terminal", "Data terminal", "machine", solid=True, frames=2, fps=1.5, light=(16, GR[3]))
def _terminal(c):
    c.rect(3, 1, 10, 14, BLACK)
    c.rect(4, 2, 8, 7, S[2])
    c.rect(5, 3, 6, 5, (12, 30, 24))
    c.hline(6, 4, 4, GR[3])
    c.hline(6, 6, 2, GR[2])
    if c.frame:
        c.px(9, 6, GR[4])
    c.rect(4, 9, 8, 5, S[1])
    c.hline(4, 9, 8, S[3])
    c.rect(6, 11, 4, 1, S[3])


@tile("pump", "Pump", "machine", solid=True, frames=4, fps=6)
def _pump(c):
    c.rect(1, 3, 14, 12, BLACK)
    c.rect(2, 4, 12, 10, S[2])
    c.hline(2, 4, 12, S[4])
    c.rect(2, 12, 12, 2, S[1])
    c.circle(8, 8, 3.5, S[0])
    c.circle(8, 8, 2.5, CY[1])
    dx, dy = ((0, -2), (2, 0), (0, 2), (-2, 0))[c.frame % 4]
    c.px(8 + dx, 8 + dy, CY[4])
    c.px(8 - dx, 8 - dy, CY[2])
    c.px(8, 8, S[4])
    c.rect(3, 6, 2, 1, AM[3])
    c.rect(12, 5, 2, 5, IC[1])


@tile("generator", "Power cell bank", "machine", solid=True, frames=2, fps=2, light=(18, AM[2]))
def _generator(c):
    c.rect(1, 1, 14, 14, BLACK)
    c.rect(2, 2, 12, 11, S[1])
    c.hline(2, 2, 12, S[3])
    for i, x in enumerate((3, 6, 9, 12)):
        c.rect(x, 4, 2, 7, S[0])
        lit = 6 if (i + c.frame) % 2 else 4
        c.rect(x, 11 - lit, 2, lit, AM[2])
        c.px(x, 11 - lit, AM[4])
    c.rect(2, 13, 12, 1, S[0])


@tile("dispenser_small", "Wall dispenser", "machine", solid=True, light=(14, CY[2]))
def _dispenser_small(c):
    c.rect(2, 1, 12, 14, BLACK)
    c.rect(3, 2, 10, 12, S[3])
    c.hline(3, 2, 10, S[5])
    c.rect(4, 4, 8, 3, CY[1])
    c.hline(5, 5, 5, CY[4])
    c.rect(5, 9, 6, 4, BLACK)
    c.rect(6, 11, 4, 2, AM[2])
    c.px(12, 8, GR[3])


@tile("vent_floor", "Floor vent", "decor", layer="decor")
def _vent_floor(c):
    c.rect(3, 4, 10, 8, S[0])
    for y in (5, 7, 9):
        c.hline(4, y, 8, S[3])
        c.hline(4, y + 1, 8, S[1])
    c.frame_rect(3, 4, 10, 8, S[1])


@tile("conduit_h", "Floor conduit (H)", "decor", layer="decor")
def _conduit_h(c):
    c.rect(0, 6, 16, 4, S[0])
    c.hline(0, 7, 16, AM[1])
    c.hline(0, 8, 16, CY[1])
    for x in (3, 11):
        c.rect(x, 5, 2, 6, S[3])
        c.vline(x + 1, 5, 6, S[1])


@tile("conduit_v", "Floor conduit (V)", "decor", layer="decor")
def _conduit_v(c):
    c.rect(6, 0, 4, 16, S[0])
    c.vline(7, 0, 16, AM[1])
    c.vline(8, 0, 16, CY[1])
    for y in (3, 11):
        c.rect(5, y, 6, 2, S[3])
        c.hline(5, y + 1, 6, S[1])


@tile("conduit_x", "Conduit junction", "decor", layer="decor")
def _conduit_x(c):
    c.rect(0, 6, 16, 4, S[0])
    c.rect(6, 0, 4, 16, S[0])
    c.hline(0, 7, 16, AM[1])
    c.vline(8, 0, 16, CY[1])
    c.rect(5, 5, 6, 6, S[3])
    c.rect(6, 6, 4, 4, S[1])
    c.px(7, 7, GR[3])


@tile("hazard_line", "Hazard edging", "decor", layer="decor")
def _hazard_line(c):
    for x in range(16):
        c.rect(x, 12, 1, 3, AM[2] if (x // 2) % 2 == 0 else S[0])


@tile("guide_h", "Guide stripe (H)", "decor", layer="decor")
def _guide_h(c):
    c.hline(0, 7, 16, CY[1])
    c.hline(0, 8, 16, CY[2])
    c.px(4, 8, CY[3])
    c.px(12, 8, CY[3])


@tile("guide_v", "Guide stripe (V)", "decor", layer="decor")
def _guide_v(c):
    c.vline(7, 0, 16, CY[1])
    c.vline(8, 0, 16, CY[2])
    c.px(8, 4, CY[3])
    c.px(8, 12, CY[3])


@tile("stain", "Oil stain", "decor", layer="decor", variants=3)
def _stain(c):
    col = (S[0][0], S[0][1], S[0][2], 150)
    cx, cy = 5 + c.rng.randrange(6), 5 + c.rng.randrange(6)
    c.ellipse(cx - 4, cy - 2, 8, 5, col)
    c.ellipse(cx, cy - 1, 5, 4, col)
    c.px(cx + 4, cy + 3, col)


@tile("floor_light", "Floor light", "decor", layer="decor", light=(30, CY[2]))
def _floor_light(c):
    c.rect(5, 5, 6, 6, S[0])
    c.rect(6, 6, 4, 4, CY[3])
    c.rect(7, 7, 2, 2, CY[4])


@tile("frost", "Frost patch", "decor", layer="decor", variants=3)
def _frost(c):
    for _ in range(14):
        c.px(c.rng.randrange(16), c.rng.randrange(16), (IC[4][0], IC[4][1], IC[4][2], 150))
    c.line(4, 10, 9, 6, (IC[3][0], IC[3][1], IC[3][2], 160))


@tile("glyph_floor", "Carved glyphs", "decor", layer="decor", frames=4, fps=2, variants=3, light=(14, PU[2]))
def _glyph_floor(c):
    glow = (PU[1], PU[2], PU[3], PU[2])[c.frame % 4]
    shapes = (
        ((4, 4, 8, 1), (4, 4, 1, 8), (4, 11, 5, 1), (8, 7, 1, 5), (11, 4, 1, 4)),
        ((3, 7, 10, 1), (7, 3, 1, 10), (4, 4, 2, 1), (10, 10, 2, 1), (10, 4, 1, 2)),
        ((4, 3, 1, 10), (11, 3, 1, 10), (4, 8, 8, 1), (7, 4, 2, 1), (7, 12, 2, 1)),
    )[c.variant]
    for x, y, w, h in shapes:
        c.rect(x, y, w, h, glow)


# ===========================================================================
# Nature / excavation
# ===========================================================================
@tile("rubble", "Rubble", "nature", solid=True, variants=3)
def _rubble(c):
    for x, y, w, h in ((2, 8, 6, 5), (7, 5, 6, 6), (9, 10, 5, 4), (4, 11, 4, 3)):
        c.rect(x - 1, y - 1, w + 2, h + 2, BLACK)
    for x, y, w, h in ((2, 8, 6, 5), (7, 5, 6, 6), (9, 10, 5, 4), (4, 11, 4, 3)):
        c.rect(x, y, w, h, RK[2])
        c.hline(x, y, w, RK[4])
        c.hline(x, y + h - 1, w, RK[1])
    c.speckle(2, 6, 12, 8, [RK[3]], 5)


@tile("ice_spike", "Ice spire", "nature", solid=True, variants=3, tags=("cold",))
def _ice_spike(c):
    tip = 7 + (c.variant - 1)
    c.poly([(tip, 0), (3, 14), (12, 14)], BLACK)
    c.poly([(tip, 2), (5, 13), (8, 13)], IC[3])
    c.poly([(tip, 2), (8, 13), (11, 13)], IC[1])
    c.line(tip, 3, 6, 11, IC[4])
    c.hline(3, 14, 10, IC[0])
    c.px(12, 12, IC[3])
    c.px(2, 13, IC[2])


@tile("crystal", "Signal crystal", "nature", solid=True, frames=4, fps=3, light=(34, PU[2]))
def _crystal(c):
    glow = (PU[2], PU[3], PU[4], PU[3])[c.frame % 4]
    c.poly([(8, 1), (4, 9), (6, 14), (10, 14), (12, 9)], BLACK)
    c.poly([(8, 2), (5, 9), (7, 13), (8, 13)], PU[2])
    c.poly([(8, 2), (8, 13), (9, 13), (11, 9)], PU[1])
    c.line(8, 3, 6, 9, glow)
    c.px(7, 6, WHITE if c.frame == 2 else glow)
    c.px(3, 12, PU[2])
    c.px(12, 13, PU[1])


@tile("ore_cart", "Ore cart", "nature", solid=True)
def _ore_cart(c):
    c.rect(1, 5, 14, 9, BLACK)
    c.rect(2, 6, 12, 3, RK[2])
    c.speckle(2, 5, 12, 3, [RK[4], IC[3], RK[1]], 10)
    c.rect(2, 9, 12, 4, AM[1])
    c.hline(2, 9, 12, AM[2])
    c.rect(3, 13, 3, 2, S[0])
    c.rect(10, 13, 3, 2, S[0])
    c.px(4, 14, S[3])
    c.px(11, 14, S[3])


@tile("lamp", "Work lamp", "nature", solid=True, light=(52, AM[3]))
def _lamp(c):
    c.rect(7, 6, 2, 8, S[1])
    c.rect(5, 13, 6, 2, S[0])
    c.rect(4, 2, 8, 5, BLACK)
    c.rect(5, 3, 6, 3, AM[4])
    c.hline(5, 3, 6, WHITE)
    c.px(4, 4, AM[3])
    c.px(11, 4, AM[3])


# ===========================================================================
# Wall-mounted signs and screens (decor layer, drawn over a wall face)
# ===========================================================================
def _plate(c, bg, edge):
    c.rect(3, 7, 10, 7, edge)
    c.rect(4, 8, 8, 5, bg)


@tile("sign_warn", "Sign: hazard", "sign", layer="decor")
def _sign_warn(c):
    _plate(c, AM[3], BLACK)
    c.grid(["..#..", ".###.", "#####"], {"#": BLACK}, 6, 9)
    c.px(8, 10, AM[3])


@tile("sign_med", "Sign: medical", "sign", layer="decor")
def _sign_med(c):
    _plate(c, WHITE, S[1])
    c.rect(7, 8, 2, 5, RD[2])
    c.rect(5, 10, 6, 1, RD[2])


@tile("sign_eng", "Sign: engineering", "sign", layer="decor")
def _sign_eng(c):
    _plate(c, S[1], BLACK)
    c.grid(["..#.", ".##.", "####", ".##.", ".#.."], {"#": AM[3]}, 6, 8)


@tile("sign_bio", "Sign: hydroponics", "sign", layer="decor")
def _sign_bio(c):
    _plate(c, GR[0], BLACK)
    c.grid([".##..", "####.", ".###.", "..#..", "..#.."], {"#": GR[3]}, 6, 8)


@tile("sign_cmd", "Sign: command", "sign", layer="decor")
def _sign_cmd(c):
    _plate(c, NV[2], BLACK)
    c.grid(["..#..", "#####", ".###.", ".#.#."], {"#": AM[3]}, 6, 8)


@tile("sign_sci", "Sign: science", "sign", layer="decor")
def _sign_sci(c):
    _plate(c, PU[0], BLACK)
    c.circle(8, 10, 2, PU[3], 1)
    c.px(8, 10, WHITE)


@tile("screen", "Wall screen", "sign", layer="decor", frames=4, fps=2, light=(16, CY[2]))
def _screen(c):
    c.rect(2, 7, 12, 7, BLACK)
    c.rect(3, 8, 10, 5, NV[1])
    pts = (2, 3, 1, 4, 2, 0, 3, 1, 2, 3)
    for i in range(10):
        h = pts[(i + c.frame) % 10]
        c.px(3 + i, 12 - h, CY[3])
        if h > 1:
            c.px(3 + i, 13 - h, CY[1])


@tile("vent_wall", "Wall vent", "sign", layer="decor")
def _vent_wall(c):
    c.rect(3, 9, 10, 5, S[0])
    for y in (10, 12):
        c.hline(4, y, 8, S[3])


@tile("pipes_wall", "Wall pipes", "sign", layer="decor")
def _pipes_wall(c):
    for y, col in ((7, CY), (10, AM), (13, S)):
        c.hline(0, y, 16, col[2] if col is not S else S[4])
        c.hline(0, y + 1, 16, col[0] if col is not S else S[1])
    c.rect(5, 6, 2, 10, S[3])
    c.vline(6, 6, 10, S[1])


@tile("locker_wall", "Wall locker", "sign", layer="decor")
def _locker_wall(c):
    c.rect(2, 6, 12, 9, BLACK)
    c.rect(3, 7, 5, 7, S[3])
    c.rect(8, 7, 5, 7, S[3])
    c.vline(8, 7, 7, S[1])
    c.hline(3, 7, 10, S[5])
    c.px(6, 10, BLACK)
    c.px(10, 10, BLACK)
    c.hline(4, 12, 3, S[2])
    c.hline(9, 12, 3, S[2])


# ===========================================================================
# Overhead layer (drawn above characters)
# ===========================================================================
@tile("beam_h", "Ceiling beam (H)", "overhead", layer="overhead")
def _beam_h(c):
    c.rect(0, 4, 16, 5, S[0])
    c.hline(0, 4, 16, S[2])
    c.hline(0, 8, 16, BLACK)
    c.px(3, 6, S[3])
    c.px(12, 6, S[3])
    c.rect(0, 9, 16, 3, (0, 0, 0, 60))


@tile("beam_v", "Ceiling beam (V)", "overhead", layer="overhead")
def _beam_v(c):
    c.rect(5, 0, 5, 16, S[0])
    c.vline(5, 0, 16, S[2])
    c.vline(9, 0, 16, BLACK)
    c.px(7, 3, S[3])
    c.px(7, 12, S[3])
    c.rect(10, 0, 3, 16, (0, 0, 0, 60))


@tile("pipes_over", "Overhead pipes", "overhead", layer="overhead")
def _pipes_over(c):
    for y, col in ((2, AM), (8, CY)):
        c.rect(0, y, 16, 4, BLACK)
        c.hline(0, y + 1, 16, col[2])
        c.hline(0, y + 2, 16, col[1])
    c.rect(6, 1, 3, 12, S[2])
    c.vline(6, 1, 12, S[4])
    c.rect(0, 13, 16, 2, (0, 0, 0, 50))


@tile("ceiling_light", "Ceiling light", "overhead", layer="overhead", light=(60, (255, 244, 220)))
def _ceiling_light(c):
    c.rect(3, 6, 10, 4, S[1])
    c.rect(4, 7, 8, 2, (255, 248, 224))
    c.px(3, 6, S[3])


@tile("grow_light", "Grow light", "overhead", layer="overhead", light=(44, (220, 120, 255)))
def _grow_light(c):
    c.rect(1, 5, 14, 4, BLACK)
    c.rect(2, 6, 12, 2, PU[3])
    c.hline(2, 6, 12, PU[4])
    c.vline(4, 0, 5, S[2])
    c.vline(11, 0, 5, S[2])


@tile("icicles", "Icicles", "overhead", layer="overhead", variants=3)
def _icicles(c):
    for x in range(1, 16, 3):
        h = 3 + c.rng.randrange(6)
        c.vline(x, 0, h, IC[3])
        c.vline(x + 1, 0, max(1, h - 2), IC[2])
        c.px(x, h, IC[4])


# ===========================================================================
# Rasteriser
# ===========================================================================
class TileArt:
    """Renders tiles on demand and caches the surfaces."""

    def __init__(self):
        self._cache: Dict[Tuple[str, int, int, int], object] = {}
        self._missing = None

    def get(self, name: str, frame: int = 0, variant: int = 0, mask: int = 0):
        tdef = TILES.get(name)
        if tdef is None:
            return self._missing_tile()
        frame = frame % max(1, tdef.frames)
        variant = variant % max(1, tdef.variants)
        if not tdef.group:
            mask = 0
        key = (name, frame, variant, mask)
        surf = self._cache.get(key)
        if surf is None:
            from .canvas import Canvas
            c = Canvas(16, 16, seed=stable_hash(name, variant), frame=frame, variant=variant, mask=mask)
            tdef.draw(c)
            surf = c.surface
            self._cache[key] = surf
        return surf

    def frame_at(self, name: str, time: float) -> int:
        """Animation frame for a time-animated tile."""
        tdef = TILES.get(name)
        if tdef is None or tdef.frames <= 1 or tdef.fps <= 0:
            return 0
        return int(time * tdef.fps) % tdef.frames

    def _missing_tile(self):
        if self._missing is None:
            from .canvas import Canvas
            c = Canvas(16, 16)
            c.fill((255, 0, 255))
            c.rect(0, 0, 8, 8, BLACK)
            c.rect(8, 8, 8, 8, BLACK)
            self._missing = c.surface
        return self._missing


# props register themselves into TILES / PROPS on import
from . import props  # noqa: E402,F401
