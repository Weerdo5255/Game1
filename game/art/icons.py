"""Procedural 16x16 item icons.

Item definitions pick an icon by name and optionally a tint (any key of
``palette.CLOTH_COLORS``)::

    "keycard_eng": {"icon": "card", "color": "amber", ...}

Add an icon by writing one more ``@icon`` function.
"""
from __future__ import annotations

from typing import Callable, Dict, Tuple

from ..engine import palette as P

S = P.STEEL
BLACK = P.BLACK
WHITE = P.WHITE

ICONS: Dict[str, Callable] = {}


def icon(name: str):
    def deco(fn):
        ICONS[name] = fn
        return fn
    return deco


@icon("generic")
def _generic(c, t):
    c.rect(3, 4, 10, 9, BLACK)
    c.rect(4, 5, 8, 7, t[1])
    c.hline(4, 5, 8, t[2])
    c.rect(4, 10, 8, 2, t[0])


@icon("ration")
def _ration(c, t):
    c.rect(2, 4, 12, 9, BLACK)
    c.rect(3, 5, 10, 7, P.BEIGE[3])
    c.hline(3, 5, 10, P.BEIGE[4])
    c.rect(3, 8, 10, 2, t[1])
    c.hline(3, 11, 10, P.BEIGE[1])
    c.px(5, 9, WHITE)
    c.px(7, 9, WHITE)


@icon("parts")
def _parts(c, t):
    c.circle(8, 8, 6, BLACK)
    c.circle(8, 8, 5, S[3])
    for dx, dy in ((0, -6), (0, 5), (-6, 0), (5, 0), (-4, -4), (4, -4), (-4, 4), (4, 4)):
        c.rect(7 + dx, 7 + dy, 2, 2, S[4] if dy <= 0 else S[2])
    c.circle(8, 8, 2.5, BLACK)
    c.circle(8, 8, 1.5, t[1])
    c.px(6, 5, S[5])


@icon("cell")
def _cell(c, t):
    c.rect(5, 1, 6, 2, S[1])
    c.rect(3, 3, 10, 12, BLACK)
    c.rect(4, 4, 8, 10, S[2])
    c.rect(5, 6, 6, 6, t[1])
    c.rect(5, 6, 2, 6, t[2])
    c.hline(4, 4, 8, S[4])
    c.px(9, 8, WHITE)
    c.px(8, 9, WHITE)
    c.px(9, 9, WHITE)
    c.px(8, 10, WHITE)


@icon("tool")
def _tool(c, t):
    c.line(4, 12, 11, 5, BLACK)
    c.line(5, 13, 12, 6, BLACK)
    c.line(4, 13, 11, 6, S[4])
    c.rect(10, 2, 4, 4, BLACK)
    c.rect(11, 3, 2, 2, S[4])
    c.clear(12, 2, 2, 2)
    c.rect(3, 11, 3, 3, t[1])
    c.px(3, 11, t[2])


@icon("card")
def _card(c, t):
    c.rect(2, 4, 12, 9, BLACK)
    c.rect(3, 5, 10, 7, t[1])
    c.hline(3, 5, 10, t[2])
    c.rect(3, 7, 10, 1, BLACK)
    c.rect(4, 9, 3, 2, P.AMBER[3])
    c.hline(8, 10, 4, t[0])


@icon("chip")
def _chip(c, t):
    c.rect(4, 4, 8, 8, BLACK)
    c.rect(5, 5, 6, 6, t[1])
    c.rect(6, 6, 4, 4, t[0])
    c.px(7, 7, t[2])
    for i in (5, 7, 9):
        c.px(i, 3, S[4])
        c.px(i, 12, S[4])
        c.px(3, i, S[4])
        c.px(12, i, S[4])


@icon("stim")
def _stim(c, t):
    c.line(3, 13, 12, 4, BLACK)
    c.line(4, 13, 13, 4, BLACK)
    c.line(4, 12, 11, 5, S[5])
    c.line(5, 12, 9, 8, t[1])
    c.rect(11, 2, 3, 3, S[3])
    c.px(2, 14, S[4])
    c.px(3, 13, S[4])


@icon("medkit")
def _medkit(c, t):
    c.rect(5, 2, 6, 3, S[1])
    c.clear(6, 3, 4, 2)
    c.rect(2, 5, 12, 9, BLACK)
    c.rect(3, 6, 10, 7, WHITE)
    c.rect(3, 11, 10, 2, S[4])
    c.rect(7, 7, 2, 5, P.RED[2])
    c.rect(5, 9, 6, 1, P.RED[2])


@icon("datapad")
def _datapad(c, t):
    c.rect(3, 2, 10, 13, BLACK)
    c.rect(4, 3, 8, 11, S[2])
    c.rect(5, 4, 6, 7, P.NAVY[0])
    c.hline(6, 5, 4, t[2])
    c.hline(6, 7, 3, t[1])
    c.hline(6, 9, 4, t[1])
    c.px(8, 12, S[4])


@icon("suit")
def _suit(c, t):
    c.rect(5, 1, 6, 5, BLACK)
    c.rect(6, 2, 4, 3, P.CYAN[2])
    c.rect(2, 6, 12, 5, BLACK)
    c.rect(3, 7, 10, 3, t[1])
    c.rect(4, 6, 8, 9, BLACK)
    c.rect(5, 7, 6, 7, t[1])
    c.hline(5, 7, 6, t[2])
    c.rect(7, 9, 2, 2, t[0])
    c.vline(8, 12, 2, BLACK)


@icon("amplifier")
def _amplifier(c, t):
    c.rect(2, 6, 12, 8, BLACK)
    c.rect(3, 7, 10, 6, S[2])
    c.hline(3, 7, 10, S[4])
    c.circle(6, 10, 2, t[1])
    c.px(6, 10, t[2])
    c.rect(10, 9, 2, 3, P.NAVY[0])
    c.px(10, 9, P.GREEN[3])
    c.vline(11, 2, 4, S[4])
    c.px(11, 1, t[2])
    c.vline(5, 3, 3, S[4])


@icon("charge")
def _charge(c, t):
    c.rect(3, 5, 10, 9, BLACK)
    c.rect(4, 6, 8, 7, P.RED[1])
    c.hline(4, 6, 8, P.RED[3])
    for x in (5, 7, 9):
        c.rect(x, 8, 1, 4, P.AMBER[3])
    c.line(8, 5, 11, 2, S[4])
    c.px(12, 1, P.AMBER[4])


@icon("seal")
def _seal(c, t):
    c.circle(8, 8, 6, BLACK)
    c.circle(8, 8, 5, t[1])
    c.circle(8, 8, 3, BLACK)
    c.circle(8, 8, 2, (0, 0, 0, 0))
    c.clear(7, 7, 2, 2)
    c.px(5, 5, t[2])
    c.px(6, 4, t[2])
    c.px(11, 11, t[0])


@icon("scrap")
def _scrap(c, t):
    c.poly([(2, 12), (5, 5), (9, 7), (13, 4), (14, 11), (8, 14)], BLACK)
    c.poly([(4, 11), (6, 7), (9, 9), (12, 6), (12, 10), (8, 12)], S[3])
    c.line(6, 8, 8, 11, S[5])
    c.px(11, 8, t[1])
    c.px(5, 10, S[1])


@icon("ore")
def _ore(c, t):
    c.poly([(3, 12), (4, 6), (8, 3), (12, 6), (13, 12), (8, 14)], BLACK)
    c.poly([(5, 11), (5, 7), (8, 5), (11, 7), (11, 11), (8, 12)], P.ROCK[2])
    c.px(7, 7, t[2])
    c.px(9, 9, t[1])
    c.px(6, 10, t[2])
    c.px(10, 7, P.ROCK[4])


@icon("key")
def _key(c, t):
    c.circle(5, 5, 3.5, BLACK)
    c.circle(5, 5, 2.5, t[1])
    c.clear(4, 4, 2, 2)
    c.line(7, 7, 13, 13, BLACK)
    c.line(7, 6, 13, 12, t[1])
    c.px(11, 12, t[1])
    c.px(12, 13, t[1])
    c.px(9, 10, t[1])


@icon("credits")
def _credits(c, t):
    c.circle(8, 8, 6, BLACK)
    c.circle(8, 8, 5, P.AMBER[2])
    c.circle(8, 8, 3.5, P.AMBER[3])
    c.rect(7, 5, 2, 6, P.AMBER[1])
    c.px(5, 5, P.AMBER[4])


@icon("plant")
def _plant(c, t):
    c.vline(8, 7, 7, P.GREEN[1])
    c.ellipse(3, 3, 6, 5, P.GREEN[2])
    c.ellipse(8, 5, 6, 5, P.GREEN[3])
    c.px(5, 4, P.GREEN[4])
    c.px(6, 11, P.GREEN[2])
    c.rect(5, 13, 6, 2, P.BROWN[2])


@icon("manual")
def _manual(c, t):
    c.rect(3, 2, 10, 12, BLACK)
    c.rect(4, 3, 8, 10, t[1])
    c.vline(4, 3, 10, t[0])
    c.hline(6, 5, 5, WHITE)
    c.hline(6, 7, 4, t[2])
    c.rect(5, 12, 7, 1, WHITE)


@icon("baton")
def _baton(c, t):
    c.line(3, 13, 12, 4, BLACK)
    c.line(4, 13, 13, 4, BLACK)
    c.line(4, 12, 12, 4, S[1])
    c.line(9, 7, 12, 4, t[1])
    c.px(13, 3, P.CYAN[4])
    c.px(12, 2, P.CYAN[3])
    c.px(14, 4, P.CYAN[3])
    c.rect(3, 11, 2, 2, S[4])


@icon("flask")
def _flask(c, t):
    c.rect(6, 1, 4, 3, S[4])
    c.poly([(6, 4), (9, 4), (13, 13), (2, 13)], BLACK)
    c.poly([(7, 5), (8, 5), (11, 12), (4, 12)], P.ICE[3])
    c.poly([(6, 9), (9, 9), (11, 12), (4, 12)], t[1])
    c.px(6, 10, t[2])


@icon("note")
def _note(c, t):
    c.rect(3, 2, 10, 12, BLACK)
    c.rect(4, 3, 8, 10, P.BEIGE[4])
    for y in (5, 7, 9, 11):
        c.hline(5, y, 6 if y != 11 else 3, P.BEIGE[1])
    c.px(11, 3, P.BEIGE[2])


@icon("coil")
def _coil(c, t):
    c.rect(2, 5, 12, 7, BLACK)
    c.rect(3, 6, 10, 5, S[1])
    for x in range(4, 12, 2):
        c.vline(x, 6, 5, t[1])
        c.px(x, 6, t[2])
    c.rect(1, 7, 2, 3, S[4])
    c.rect(13, 7, 2, 3, S[4])


@icon("lens")
def _lens(c, t):
    c.circle(8, 8, 6, BLACK)
    c.circle(8, 8, 5, S[3])
    c.circle(8, 8, 3.5, t[0])
    c.circle(8, 8, 2.5, t[1])
    c.px(7, 7, WHITE)
    c.px(9, 9, t[2])


@icon("badge")
def _badge(c, t):
    c.poly([(8, 1), (14, 4), (13, 10), (8, 14), (3, 10), (2, 4)], BLACK)
    c.poly([(8, 3), (12, 5), (11, 9), (8, 12), (5, 9), (4, 5)], t[1])
    c.line(8, 3, 5, 5, t[2])
    c.rect(7, 6, 2, 3, P.AMBER[3])


@icon("shard")
def _shard(c, t):
    c.poly([(8, 1), (4, 8), (7, 14), (10, 14), (12, 7)], BLACK)
    c.poly([(8, 3), (6, 8), (8, 12)], t[2])
    c.poly([(8, 3), (8, 12), (9, 12), (10, 7)], t[1])
    c.px(7, 7, WHITE)


class IconArt:
    def __init__(self):
        self._cache: Dict[Tuple[str, str], object] = {}

    def get(self, name: str, color: str = "steel"):
        key = (name, color)
        surf = self._cache.get(key)
        if surf is None:
            from .canvas import Canvas
            c = Canvas(16, 16)
            tint = P.CLOTH_COLORS.get(color, P.CLOTH_COLORS["steel"])
            ICONS.get(name, ICONS["generic"])(c, tint)
            surf = self._cache[key] = c.surface
        return surf
