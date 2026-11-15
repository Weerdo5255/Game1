"""Multi-tile props: reactors, tanks, beds, the shuttle ...

A prop is painted once as a single big picture and then cut into 16x16 tiles
named ``<prop>.<col>.<row>``.  The rows at the bottom (``base_rows``) are
solid and live on the ``objects`` layer; anything above them goes on the
``overhead`` layer so characters can walk behind tall machinery.

The map editor places a whole prop in one click using ``PROPS[name].parts``.
"""
from __future__ import annotations

from functools import partial
from typing import Dict, Optional, Sequence, Tuple

from ..engine import palette as P
from .tiles import (AM, BG, BLACK, BR, CY, GR, IC, NV, PROPS, PU, RD, RK, S, TILES, WHITE,
                    PropDef, TileDef, stable_hash)

_BIG: Dict[Tuple[str, int], object] = {}


def _draw_part(name: str, fn, w: int, h: int, tx: int, ty: int, c) -> None:
    key = (name, c.frame)
    big = _BIG.get(key)
    if big is None:
        from .canvas import Canvas
        bc = Canvas(w * 16, h * 16, seed=stable_hash(name), frame=c.frame)
        fn(bc)
        big = _BIG[key] = bc.surface
    c.surface.blit(big, (0, 0), (tx * 16, ty * 16, 16, 16))


def prop(name: str, label: str, category: str, w: int, h: int, base_rows: int = 1,
         frames: int = 1, fps: float = 0.0, light=None, light_at: Optional[Tuple[int, int]] = None,
         solid: bool = True, layer: Optional[str] = None, solid_map: Optional[Sequence[str]] = None,
         tags: Tuple[str, ...] = ()):
    """Register a multi-tile prop; decorates its drawing function."""
    def deco(fn):
        pdef = PropDef(name=name, label=label, category=category, w=w, h=h)
        if light_at is None:
            lx, ly = w // 2, max(0, h - base_rows)
        else:
            lx, ly = light_at
        for ty in range(h):
            for tx in range(w):
                part = "%s.%d.%d" % (name, tx, ty)
                is_base = ty >= h - base_rows
                part_layer = layer or ("objects" if is_base else "overhead")
                if solid_map is not None:
                    part_solid = solid_map[ty][tx] == "X"
                else:
                    part_solid = solid and is_base
                TILES[part] = TileDef(
                    name=part, label="%s [%d,%d]" % (label, tx, ty), category="prop",
                    layer=part_layer, solid=part_solid,
                    draw=partial(_draw_part, name, fn, w, h, tx, ty),
                    frames=frames, fps=fps,
                    light=light if (tx, ty) == (lx, ly) else None,
                    tags=tags, prop=name, hidden=True)
                pdef.parts.append((tx, ty, part_layer, part))
        PROPS[name] = pdef
        return fn
    return deco


# ---------------------------------------------------------------------------
# Storage
# ---------------------------------------------------------------------------
@prop("locker", "Locker", "furniture", 1, 2)
def _locker(c):
    c.rect(1, 2, 14, 29, BLACK)
    c.rect(2, 3, 12, 3, S[4])
    c.hline(2, 3, 12, S[5])
    c.rect(2, 6, 12, 23, S[3])
    c.vline(8, 6, 23, S[1])
    c.vline(2, 6, 23, S[4])
    for y in (9, 11):
        c.hline(4, y, 3, S[1])
        c.hline(10, y, 3, S[1])
    c.rect(4, 15, 3, 2, CY[1])
    c.rect(10, 15, 3, 2, AM[2])
    c.px(6, 20, BLACK)
    c.px(10, 20, BLACK)
    c.rect(2, 27, 12, 2, S[2])


@prop("shelf", "Storage shelf", "furniture", 1, 2)
def _shelf(c):
    c.rect(1, 2, 14, 29, BLACK)
    c.rect(2, 3, 12, 26, S[1])
    c.rect(2, 3, 12, 2, S[3])
    boxes = ((AM[2], CY[2], S[4]), (GR[2], S[4], RD[2]), (S[3], AM[3], CY[1]))
    for row, y in enumerate((6, 14, 22)):
        c.hline(2, y + 6, 12, S[3])
        c.hline(2, y + 7, 12, S[0])
        x = 3
        for i, col in enumerate(boxes[row]):
            w = (3, 4, 3)[(i + row) % 3]
            hgt = (4, 5, 3)[(i + row) % 3]
            c.rect(x, y + 6 - hgt, w, hgt, col)
            c.hline(x, y + 6 - hgt, w, P.lighten(col, 0.35))
            x += w + 1


@prop("server", "Server rack", "machine", 1, 2, frames=4, fps=4, light=(18, CY[2]))
def _server(c):
    c.rect(1, 2, 14, 29, BLACK)
    c.rect(2, 3, 12, 3, S[3])
    c.rect(2, 6, 12, 23, NV[0])
    c.vline(2, 6, 23, S[1])
    for row, y in enumerate(range(8, 28, 4)):
        c.hline(4, y, 9, S[1])
        for i, x in enumerate((4, 6, 8, 11)):
            on = (row * 3 + i * 5 + c.frame * (1 + i % 2)) % 4
            col = (CY[3], GR[3], NV[2], AM[3])[on] if i < 3 else (RD[2] if on == 0 else NV[2])
            c.px(x, y + 1, col)
    c.rect(2, 28, 12, 1, S[2])


# ---------------------------------------------------------------------------
# Tanks
# ---------------------------------------------------------------------------
def _tank(c, liq):
    c.rect(3, 1, 10, 4, BLACK)
    c.rect(4, 2, 8, 2, S[4])
    c.rect(2, 4, 12, 24, BLACK)
    c.rect(3, 5, 10, 22, liq[1])
    level = 7 + (c.frame % 2)
    c.rect(3, 5, 10, level - 5, NV[1])
    c.hline(3, level, 10, liq[3])
    c.vline(4, level + 1, 26 - level, liq[2])
    c.vline(5, level + 2, 8, liq[3])
    c.vline(12, 5, 22, liq[0])
    for i in range(3):
        by = 25 - ((c.frame * 5 + i * 7) % 16)
        if by > level:
            c.px(7 + i * 2, by, liq[4])
    c.rect(1, 27, 14, 4, BLACK)
    c.rect(2, 28, 12, 2, S[2])
    c.hline(2, 28, 12, S[4])
    c.px(4, 29, liq[3])


@prop("tank_water", "Water tank", "machine", 1, 2, frames=4, fps=3)
def _tank_water(c):
    _tank(c, IC)


@prop("tank_coolant", "Coolant tank", "machine", 1, 2, frames=4, fps=3, light=(18, CY[2]))
def _tank_coolant(c):
    _tank(c, CY)


@prop("tank_algae", "Algae tank", "machine", 1, 2, frames=4, fps=3, light=(18, GR[2]))
def _tank_algae(c):
    _tank(c, GR)


# ---------------------------------------------------------------------------
# Beds and pods
# ---------------------------------------------------------------------------
def _bed(c, blanket):
    c.rect(1, 1, 14, 30, BLACK)
    c.rect(2, 2, 12, 3, S[3])
    c.hline(2, 2, 12, S[5])
    c.rect(2, 5, 12, 23, S[5])
    c.rect(4, 6, 8, 5, WHITE)
    c.hline(4, 10, 8, S[4])
    c.rect(2, 13, 12, 15, blanket[1])
    c.hline(2, 13, 12, blanket[2])
    c.hline(2, 15, 12, blanket[0])
    c.vline(13, 13, 15, blanket[0])
    c.rect(4, 20, 2, 2, blanket[2])
    c.rect(2, 28, 12, 2, S[2])


@prop("bed", "Bunk", "furniture", 1, 2, base_rows=2, tags=("bed",))
def _bed_teal(c):
    _bed(c, P.CLOTH_COLORS["teal"])


@prop("bed_red", "Bunk (red)", "furniture", 1, 2, base_rows=2, tags=("bed",))
def _bed_red(c):
    _bed(c, P.CLOTH_COLORS["red"])


@prop("bed_amber", "Bunk (amber)", "furniture", 1, 2, base_rows=2, tags=("bed",))
def _bed_amber(c):
    _bed(c, P.CLOTH_COLORS["amber"])


@prop("medbed", "Med bed", "furniture", 1, 2, base_rows=2, tags=("bed",), light=(16, CY[2]))
def _medbed(c):
    c.rect(1, 1, 14, 30, BLACK)
    c.rect(2, 2, 12, 26, WHITE)
    c.rect(2, 2, 12, 4, S[3])
    c.rect(4, 3, 8, 2, CY[3])
    c.rect(4, 7, 8, 4, S[5])
    c.rect(2, 13, 12, 14, S[5])
    c.hline(2, 13, 12, CY[2])
    c.vline(2, 6, 22, S[4])
    c.vline(13, 6, 22, S[4])
    c.rect(2, 28, 12, 2, S[2])
    c.px(12, 26, GR[3])


@prop("cryo_pod", "Cryo pod", "machine", 1, 2, frames=2, fps=1, light=(18, IC[2]))
def _cryo(c):
    c.rect(2, 1, 12, 30, BLACK)
    c.rect(3, 2, 10, 27, S[3])
    c.hline(3, 2, 10, S[5])
    c.rect(4, 5, 8, 18, IC[2])
    c.dither(4, 5, 8, 18, IC[3], c.frame)
    c.circle(8, 10, 2, IC[1])
    c.rect(6, 13, 4, 8, IC[1])
    c.line(5, 8, 5, 14, IC[4])
    c.rect(4, 24, 8, 3, S[1])
    c.px(6, 25, GR[3] if c.frame else GR[1])
    c.px(9, 25, CY[3])
    c.rect(3, 28, 10, 2, S[1])


@prop("dispenser", "Galley dispenser", "machine", 1, 2, frames=2, fps=1, light=(20, CY[2]))
def _dispenser(c):
    c.rect(1, 1, 14, 30, BLACK)
    c.rect(2, 2, 12, 27, S[3])
    c.vline(2, 2, 27, S[4])
    c.rect(3, 3, 10, 5, CY[1])
    c.hline(4, 4, 8, CY[4] if c.frame else CY[3])
    c.hline(4, 6, 5, CY[3])
    c.rect(3, 10, 10, 7, NV[0])
    for i, y in enumerate((11, 13, 15)):
        c.hline(4, y, 6, (AM[3], GR[3], CY[3])[i])
        c.px(11, y, WHITE)
    c.rect(4, 20, 8, 6, BLACK)
    c.rect(5, 23, 6, 3, AM[2])
    c.hline(5, 23, 6, AM[4])
    c.rect(2, 27, 12, 2, S[2])


# ---------------------------------------------------------------------------
# Heavy machinery
# ---------------------------------------------------------------------------
@prop("fabricator", "Fabricator", "machine", 2, 2, frames=4, fps=4, light=(24, AM[2]))
def _fabricator(c):
    c.rect(1, 3, 30, 28, BLACK)
    c.rect(2, 4, 28, 14, S[2])
    c.hline(2, 4, 28, S[4])
    c.rect(4, 6, 16, 11, S[1])
    c.rect(5, 7, 14, 9, NV[0])
    c.hline(7, 14, 10, S[3])
    grow = 1 + c.frame
    c.rect(10, 14 - grow, 4, grow, AM[2])
    lx = 8 + c.frame * 3
    c.vline(lx, 8, 13 - grow - 7 if 13 - grow - 7 > 0 else 1, AM[4])
    c.px(lx, 13 - grow, WHITE)
    c.rect(21, 6, 8, 11, S[1])
    c.rect(22, 7, 6, 3, CY[1])
    c.hline(23, 8, 3, CY[4])
    for i, x in enumerate((22, 24, 26)):
        c.px(x, 12, (GR[3], AM[3], RD[2])[(i + c.frame) % 3])
        c.px(x, 14, S[3])
    c.rect(2, 18, 28, 11, S[3])
    c.hline(2, 18, 28, S[1])
    c.hline(2, 19, 28, S[4])
    c.rect(5, 21, 14, 6, BLACK)
    c.rect(6, 22, 12, 4, S[1])
    c.rect(9, 23, 5, 2, S[5])
    for y in (22, 24, 26):
        c.hline(22, y, 6, S[1])
    for x in range(2, 30, 4):
        c.rect(x, 29, 2, 1, AM[2])


@prop("reactor", "Fusion reactor", "machine", 3, 3, base_rows=2, frames=4, fps=5,
      light=(72, CY[2]), light_at=(1, 1))
def _reactor(c):
    f = c.frame
    c.ellipse(4, 37, 40, 10, (0, 0, 0, 80))
    c.ellipse(5, 33, 38, 13, BLACK)
    c.ellipse(6, 34, 36, 11, S[1])
    c.ellipse(8, 34, 32, 8, S[2])
    c.rect(10, 9, 28, 31, BLACK)
    c.hgrad(11, 9, 26, 30, [S[1], S[2], S[3], S[4], S[4], S[3], S[3], S[2], S[1], S[0]])
    c.ellipse(10, 3, 28, 12, BLACK)
    c.ellipse(11, 4, 26, 10, S[4])
    c.ellipse(14, 5, 20, 7, S[3])
    c.ellipse(19, 6, 10, 4, S[1])
    c.px(23, 7, CY[4] if f % 2 else CY[2])
    c.px(24, 8, CY[2])
    for y in (15, 35):
        c.rect(10, y, 28, 2, S[0])
        for x in range(11, 37, 4):
            c.rect(x, y, 2, 1, AM[2])
    glow = (CY[2], CY[3], CY[4], CY[3])[f]
    c.rect(16, 18, 16, 16, BLACK)
    c.rect(17, 19, 14, 14, CY[0])
    c.rect(18, 20, 12, 12, CY[1])
    c.rect(21, 20, 6, 12, glow)
    c.vline(23, 20, 12, WHITE if f == 2 else CY[4])
    c.vline(24, 20, 12, CY[4])
    for i in range(3):
        y = 21 + ((f * 3 + i * 4) % 10)
        c.px(20 if i % 2 else 27, y, CY[4])
    c.hline(17, 25, 14, BLACK)
    c.rect(4, 17, 4, 22, BLACK)
    c.vline(5, 18, 20, AM[2])
    c.vline(6, 18, 20, AM[1])
    c.rect(40, 17, 4, 22, BLACK)
    c.vline(41, 18, 20, CY[2])
    c.vline(42, 18, 20, CY[0])
    c.rect(6, 20, 5, 3, S[2])
    c.rect(37, 20, 5, 3, S[2])
    c.px(13, 12, RD[3] if f % 2 else RD[1])
    c.px(34, 12, RD[1] if f % 2 else RD[3])


@prop("comm_dish", "Comms dish", "machine", 2, 2, frames=2, fps=1.2)
def _comm_dish(c):
    c.rect(14, 16, 4, 12, BLACK)
    c.rect(15, 16, 2, 12, S[2])
    c.rect(9, 26, 14, 5, BLACK)
    c.rect(10, 27, 12, 3, S[2])
    c.hline(10, 27, 12, S[4])
    c.ellipse(2, 2, 28, 19, BLACK)
    c.ellipse(3, 3, 26, 17, S[4])
    c.ellipse(5, 5, 22, 13, S[3])
    c.ellipse(9, 7, 14, 8, S[2])
    c.line(6, 9, 11, 5, S[5])
    c.line(16, 11, 23, 4, BLACK)
    c.line(16, 12, 23, 5, S[1])
    c.rect(22, 2, 3, 3, BLACK)
    c.px(23, 3, RD[3] if c.frame else RD[1])


@prop("holo_table", "Holo table", "machine", 2, 2, frames=4, fps=4, light=(34, CY[2]))
def _holo_table(c):
    f = c.frame
    c.ellipse(1, 17, 30, 14, BLACK)
    c.ellipse(2, 18, 28, 12, S[2])
    c.ellipse(4, 19, 24, 8, NV[1])
    c.ellipse(7, 20, 18, 5, CY[0])
    c.ellipse(2, 18, 28, 12, S[4], 1)
    for y in range(8, 22, 2):
        w = 2 + (y - 8) // 2
        c.hline(16 - w, y, w * 2, (CY[1][0], CY[1][1], CY[1][2], 70))
    c.circle(16, 9, 7, (CY[1][0], CY[1][1], CY[1][2], 150))
    c.circle(16, 9, 7, CY[3], 1)
    c.hline(10, 9, 13, CY[2])
    c.hline(11, 6, 11, CY[2])
    c.hline(11, 12, 11, CY[2])
    x = 11 + (f * 3) % 10
    c.vline(x, 4, 11, CY[3])
    c.px(13 + f, 7, WHITE)


@prop("drill", "Bore rig", "machine", 2, 2, frames=2, fps=5)
def _drill(c):
    f = c.frame
    c.rect(2, 24, 28, 7, BLACK)
    c.rect(3, 25, 26, 5, S[1])
    for x in range(4 + f, 28, 4):
        c.px(x, 27, S[3])
    c.rect(4, 9, 16, 16, BLACK)
    c.rect(5, 10, 14, 14, AM[2])
    c.hline(5, 10, 14, AM[4])
    c.rect(5, 20, 14, 4, AM[1])
    for x in range(5, 19, 4):
        c.rect(x, 21, 2, 2, BLACK)
    c.rect(7, 12, 7, 6, BLACK)
    c.rect(8, 13, 5, 4, NV[2])
    c.px(9, 14, CY[3])
    c.rect(19, 14, 4, 5, S[2])
    c.poly([(22, 7), (22, 25), (31, 16)], BLACK)
    c.poly([(23, 10), (23, 22), (29, 16)], S[3])
    for i in range(3):
        x = 24 + i * 2
        y0 = 12 + i + f
        c.line(x, y0, x, y0 + 5 - i * 2, S[5] if i % 2 == f else S[1])
    c.rect(6, 7, 3, 3, BLACK)
    c.px(7, 8, AM[4] if f else AM[1])


@prop("artifact", "The artifact", "nature", 2, 3, frames=4, fps=3,
      light=(84, PU[2]), light_at=(0, 1))
def _artifact(c):
    f = c.frame
    c.ellipse(2, 37, 28, 10, BLACK)
    c.ellipse(3, 38, 26, 8, RK[1])
    c.speckle(5, 39, 22, 5, [IC[2], RK[3]], 12)
    c.poly([(16, 1), (8, 10), (6, 43), (26, 43), (24, 10)], BLACK)
    c.poly([(16, 3), (10, 10), (8, 41), (16, 41)], (34, 20, 60))
    c.poly([(16, 3), (22, 10), (24, 41), (16, 41)], (20, 12, 40))
    c.line(16, 3, 16, 41, PU[1])
    glow = (PU[2], PU[3], PU[4], PU[3])[f]
    dim = (PU[1], PU[1], PU[2], PU[1])[f]
    marks = ((11, 14, 3, 1), (12, 16, 1, 2), (10, 21, 4, 1), (10, 23, 1, 2), (13, 23, 1, 3),
             (10, 29, 2, 1), (12, 31, 2, 1), (9, 35, 5, 1), (11, 37, 1, 2))
    for x, y, w, h in marks:
        c.rect(x, y, w, h, glow)
        c.rect(32 - x - w, y + 1, w, h, dim)
    c.px(16, 2, WHITE if f == 2 else PU[4])
    for dx, dy in ((-2, 0), (2, 0), (0, -1)):
        c.px(16 + dx, 2 + dy, (PU[3][0], PU[3][1], PU[3][2], 120 if f == 2 else 50))


@prop("shuttle", "Shuttle", "machine", 4, 5, base_rows=5, frames=2, fps=1.5,
      solid_map=(".XX.", ".XX.", "XXXX", "XXXX", ".XX."), light=(30, AM[2]), light_at=(1, 4))
def _shuttle(c):
    f = c.frame
    c.ellipse(8, 10, 52, 68, (0, 0, 0, 70))
    c.poly([(32, 26), (2, 58), (2, 68), (24, 63), (40, 63), (62, 68), (62, 58)], BLACK)
    c.poly([(32, 29), (4, 59), (4, 66), (24, 61), (40, 61), (60, 66), (60, 59)], S[2])
    c.line(31, 30, 5, 59, S[4])
    c.line(33, 30, 59, 59, S[3])
    c.rect(8, 60, 6, 2, AM[2])
    c.rect(50, 60, 6, 2, AM[2])
    for x0 in (17, 37):
        c.rect(x0, 58, 11, 18, BLACK)
        c.rect(x0 + 1, 59, 9, 13, S[1])
        c.vline(x0 + 1, 59, 13, S[3])
        c.rect(x0 + 2, 72, 7, 3, AM[3] if f else AM[2])
        c.px(x0 + 5, 74, WHITE if f else AM[4])
    c.ellipse(20, 2, 24, 70, BLACK)
    c.ellipse(21, 3, 22, 68, S[4])
    c.ellipse(27, 4, 15, 66, S[3])
    c.ellipse(33, 6, 9, 60, S[2])
    c.line(24, 20, 24, 52, S[5])
    c.ellipse(25, 10, 14, 15, BLACK)
    c.ellipse(26, 11, 12, 13, NV[2])
    c.ellipse(27, 12, 10, 8, NV[3])
    c.line(28, 17, 31, 13, CY[3])
    c.px(33, 14, CY[4])
    c.rect(22, 32, 20, 3, AM[2])
    c.hline(22, 32, 20, AM[3])
    c.hline(23, 42, 18, S[2])
    c.hline(23, 54, 18, S[2])
    c.frame_rect(28, 44, 8, 8, S[2])
    c.px(34, 48, S[1])
    for i in range(4):
        c.px(26 + i * 2, 37, S[1])
    c.px(3, 62, RD[3] if f else RD[1])
    c.px(60, 62, GR[3] if f else GR[1])
    c.px(32, 3, WHITE if f else S[3])


@prop("viewscreen", "Wide viewscreen", "sign", 3, 1, frames=4, fps=2, solid=False, layer="decor",
      light=(26, CY[2]), light_at=(1, 0))
def _viewscreen(c):
    f = c.frame
    c.rect(1, 6, 46, 9, BLACK)
    c.rect(2, 7, 44, 7, NV[1])
    c.circle(12, 10, 3, AM[2])
    c.hline(7, 10, 11, AM[4])
    c.px(12 + (f % 2), 8, AM[4])
    for i in range(18):
        h = (1, 3, 2, 4, 2, 1, 3, 5, 2, 1)[(i + f) % 10]
        c.vline(22 + i, 13 - h, h, CY[2] if i % 3 else CY[3])
    c.px(44, 8, RD[3] if f % 2 else RD[1])
    c.hline(22, 8, 8, CY[1])
