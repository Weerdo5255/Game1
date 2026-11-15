"""A hand-drawn proportional bitmap font.

No font files are shipped with the game: every glyph is defined below as a
tiny picture ("#" = pixel) and rendered into surfaces on demand.  Capital
letters are 7 pixels tall, lower-case letters have a 5 pixel x-height and
descenders may use two extra rows, so a line is 10 pixels high.

Text may contain colour tags such as ``<y>amber</y>`` (see
``palette.TEXT_COLORS``).  ``<<`` produces a literal ``<``.
"""
from __future__ import annotations

from typing import Dict, List, Optional, Sequence, Tuple

import pygame

from . import palette

GLYPH_ROWS = 9          # 7 body rows + 2 descender rows
LINE_HEIGHT = 10
SPACE_WIDTH = 3
MONO_ADVANCE = 6

# Each glyph: rows separated by "/".  Missing trailing rows are blank.
_G: Dict[str, str] = {
    "!": "#/#/#/#/#/./#",
    '"': "#.#/#.#",
    "#": ".#.#./.#.#./#####/.#.#./#####/.#.#./.#.#.",
    "$": "..#../.####/#.#../.###./..#.#/####./..#..",
    "%": "##..#/##..#/...#./..#../.#.../#..##/#..##",
    "&": ".##../#..#./#.#../.#.../#.#.#/#..#./.##.#",
    "'": "#/#",
    "(": "..#/.#./#../#../#../.#./..#",
    ")": "#../.#./..#/..#/..#/.#./#..",
    "*": "...../..#../#.#.#/.###./#.#.#/..#..",
    "+": "...../..#../..#../#####/..#../..#..",
    ",": "../../../../../../.#/.#/#.",
    "-": ".../.../.../###",
    ".": "././././././#",
    "/": "....#/....#/...#./..#../.#.../#..../#....",
    "0": ".###./#...#/#..##/#.#.#/##..#/#...#/.###.",
    "1": ".#./##./.#./.#./.#./.#./###",
    "2": ".###./#...#/....#/...#./..#../.#.../#####",
    "3": ".###./#...#/....#/..##./....#/#...#/.###.",
    "4": "...#./..##./.#.#./#..#./#####/...#./...#.",
    "5": "#####/#..../####./....#/....#/#...#/.###.",
    "6": "..##./.#.../#..../####./#...#/#...#/.###.",
    "7": "#####/....#/...#./..#../.#.../.#.../.#...",
    "8": ".###./#...#/#...#/.###./#...#/#...#/.###.",
    "9": ".###./#...#/#...#/.####/....#/...#./.##..",
    ":": "././#/././#",
    ";": "../../.#/../../.#/.#/#.",
    "<": "...#/..#./.#../#.../.#../..#./...#",
    "=": "..../..../####/..../####",
    ">": "#.../.#../..#./...#/..#./.#../#...",
    "?": ".###./#...#/....#/...#./..#../...../..#..",
    "@": ".###./#...#/#.###/#.#.#/#.###/#..../.###.",
    "A": ".###./#...#/#...#/#####/#...#/#...#/#...#",
    "B": "####./#...#/#...#/####./#...#/#...#/####.",
    "C": ".###./#...#/#..../#..../#..../#...#/.###.",
    "D": "####./#...#/#...#/#...#/#...#/#...#/####.",
    "E": "#####/#..../#..../####./#..../#..../#####",
    "F": "#####/#..../#..../####./#..../#..../#....",
    "G": ".###./#...#/#..../#.###/#...#/#...#/.###.",
    "H": "#...#/#...#/#...#/#####/#...#/#...#/#...#",
    "I": "###/.#./.#./.#./.#./.#./###",
    "J": "..###/...#./...#./...#./...#./#..#./.##..",
    "K": "#...#/#..#./#.#../##.../#.#../#..#./#...#",
    "L": "#..../#..../#..../#..../#..../#..../#####",
    "M": "#...#/##.##/#.#.#/#.#.#/#...#/#...#/#...#",
    "N": "#...#/##..#/#.#.#/#..##/#...#/#...#/#...#",
    "O": ".###./#...#/#...#/#...#/#...#/#...#/.###.",
    "P": "####./#...#/#...#/####./#..../#..../#....",
    "Q": ".###./#...#/#...#/#...#/#.#.#/#..#./.##.#",
    "R": "####./#...#/#...#/####./#.#../#..#./#...#",
    "S": ".####/#..../#..../.###./....#/....#/####.",
    "T": "#####/..#../..#../..#../..#../..#../..#..",
    "U": "#...#/#...#/#...#/#...#/#...#/#...#/.###.",
    "V": "#...#/#...#/#...#/.#.#./.#.#./.#.#./..#..",
    "W": "#...#/#...#/#...#/#.#.#/#.#.#/##.##/#...#",
    "X": "#...#/#...#/.#.#./..#../.#.#./#...#/#...#",
    "Y": "#...#/#...#/.#.#./..#../..#../..#../..#..",
    "Z": "#####/....#/...#./..#../.#.../#..../#####",
    "[": "##/#./#./#./#./#./##",
    "\\": "#..../#..../.#.../..#../...#./....#/....#",
    "]": "##/.#/.#/.#/.#/.#/##",
    "^": ".#./#.#",
    "_": "..../..../..../..../..../..../..../####",
    "`": "#./.#",
    "a": "..../..../.##./...#/.###/#..#/.###",
    "b": "#.../#.../###./#..#/#..#/#..#/###.",
    "c": "..../..../.###/#.../#.../#.../.###",
    "d": "...#/...#/.###/#..#/#..#/#..#/.###",
    "e": "..../..../.##./#..#/####/#.../.###",
    "f": ".##/#../###/#../#../#../#..",
    "g": "..../..../.###/#..#/#..#/#..#/.###/...#/.##.",
    "h": "#.../#.../###./#..#/#..#/#..#/#..#",
    "i": "#/./#/#/#/#/#",
    "j": ".#/../.#/.#/.#/.#/.#/.#/#.",
    "k": "#.../#.../#..#/#.#./##../#.#./#..#",
    "l": "#./#./#./#./#./#./.#",
    "m": "...../...../####./#.#.#/#.#.#/#.#.#/#.#.#",
    "n": "..../..../###./#..#/#..#/#..#/#..#",
    "o": "..../..../.##./#..#/#..#/#..#/.##.",
    "p": "..../..../###./#..#/#..#/#..#/###./#.../#...",
    "q": "..../..../.###/#..#/#..#/#..#/.###/...#/...#",
    "r": "..../..../#.##/##../#.../#.../#...",
    "s": "..../..../.###/#.../.##./...#/###.",
    "t": ".#./.#./###/.#./.#./.#./..#",
    "u": "..../..../#..#/#..#/#..#/#..#/.###",
    "v": "...../...../#...#/#...#/.#.#./.#.#./..#..",
    "w": "...../...../#...#/#...#/#.#.#/#.#.#/.#.#.",
    "x": ".../.../#.#/#.#/.#./#.#/#.#",
    "y": "..../..../#..#/#..#/#..#/#..#/.###/...#/.##.",
    "z": "..../..../####/...#/.##./#.../####",
    "{": "..#/.#./.#./#../.#./.#./..#",
    "|": "#/#/#/#/#/#/#",
    "}": "#../.#./.#./..#/.#./.#./#..",
    "~": "...../...../.#.../#.#.#/...#.",
    # --- extras used by the UI ---------------------------------------------
    "→": "...../..#../...#./#####/...#./..#..",          # right arrow
    "←": "...../..#../.#.../#####/.#.../..#..",          # left arrow
    "↑": "..#../.###./#.#.#/..#../..#../..#..",          # up arrow
    "↓": "..#../..#../..#../#.#.#/.###./..#..",          # down arrow
    "▶": "#.../##../###./####/###./##../#...",           # play / cursor
    "◀": "...#/..##/.###/####/.###/..##/...#",
    "▲": "...../...../..#../.###./#####",
    "▼": "...../...../#####/.###./..#..",
    "•": ".../.../.#./###/.#.",                          # bullet
    "…": "...../...../...../...../...../...../#.#.#",    # ellipsis
    "—": "...../...../...../#####",                      # em dash
    "★": "..#../..#../#####/.###./.#.#./#...#",          # star
    "♥": ".#.#./#####/#####/.###./..#..",                # heart
    "✓": "...../....#/...#./#.#../.#...",                # check mark
    "✗": "...../#...#/.#.#./..#../.#.#./#...#",          # cross mark
    "°": ".#./#.#/.#.",                                  # degree
    "■": "...../####./####./####./####.",                # filled square
    "□": "...../####./#..#./#..#./####.",                # hollow square
    "♦": "...../..#../.###./#####/.###./..#..",          # diamond
    "⚠": "..#../..#../.#.#./.#.#./#.#.#/#...#/#####",    # warning
}
# typographic characters that should fall back to plain ASCII glyphs
_ALIASES = {
    "‘": "'", "’": "'", "“": '"', "”": '"',
    "–": "-", " ": " ", "\t": " ",
}


def _build_masks() -> Dict[str, pygame.mask.Mask]:
    masks: Dict[str, pygame.mask.Mask] = {}
    for ch, spec in _G.items():
        rows = spec.split("/")
        width = max(len(r) for r in rows)
        mask = pygame.mask.Mask((width, GLYPH_ROWS))
        for y, row in enumerate(rows[:GLYPH_ROWS]):
            for x, px in enumerate(row):
                if px == "#":
                    mask.set_at((x, y), 1)
        masks[ch] = mask
    return masks


Span = Tuple[str, Tuple[int, int, int]]


def parse_markup(text: str, color=palette.WHITE) -> List[Span]:
    """Split ``text`` into (chunk, colour) spans, resolving ``<y>..</y>`` tags."""
    spans: List[Span] = []
    stack = [tuple(color)]
    buf: List[str] = []
    i, n = 0, len(text)

    def flush() -> None:
        if buf:
            spans.append(("".join(buf), stack[-1]))
            buf.clear()

    while i < n:
        ch = text[i]
        if ch == "<":
            if text.startswith("<<", i):
                buf.append("<")
                i += 2
                continue
            end = text.find(">", i)
            if end != -1:
                tag = text[i + 1:end]
                if tag in palette.TEXT_COLORS:
                    flush()
                    stack.append(palette.TEXT_COLORS[tag])
                    i = end + 1
                    continue
                if tag.startswith("/") and (tag[1:] in palette.TEXT_COLORS or tag == "/"):
                    flush()
                    if len(stack) > 1:
                        stack.pop()
                    i = end + 1
                    continue
        buf.append(ch)
        i += 1
    flush()
    return spans


def strip_markup(text: str) -> str:
    return "".join(chunk for chunk, _ in parse_markup(text))


class BitmapFont:
    """Renders text with the built-in pixel font at an integer scale."""

    def __init__(self, scale: int = 1):
        self.scale = max(1, int(scale))
        self._masks = _build_masks()
        self._glyphs: Dict[Tuple[str, Tuple[int, int, int]], pygame.Surface] = {}
        self._cache: Dict[tuple, pygame.Surface] = {}
        self.line_height = LINE_HEIGHT * self.scale

    # -- measuring -----------------------------------------------------------
    def _mask(self, ch: str) -> Optional[pygame.mask.Mask]:
        ch = _ALIASES.get(ch, ch)
        if ch == " ":
            return None
        return self._masks.get(ch) or self._masks["?"]

    def char_width(self, ch: str, mono: bool = False) -> int:
        """Advance of one character in unscaled pixels (including spacing)."""
        if mono:
            return MONO_ADVANCE
        mask = self._mask(ch)
        if mask is None:
            return SPACE_WIDTH
        return mask.get_size()[0] + 1

    def width(self, text: str, mono: bool = False) -> int:
        """Pixel width of ``text`` (markup tags are not counted)."""
        plain = strip_markup(text) if "<" in text else text
        if not plain:
            return 0
        total = sum(self.char_width(ch, mono) for ch in plain)
        return (total - (0 if mono else 1)) * self.scale

    def size(self, text: str, mono: bool = False) -> Tuple[int, int]:
        return self.width(text, mono), self.line_height

    # -- rendering -----------------------------------------------------------
    def _glyph(self, ch: str, color) -> Optional[pygame.Surface]:
        mask = self._mask(ch)
        if mask is None:
            return None
        key = (_ALIASES.get(ch, ch), tuple(color))
        surf = self._glyphs.get(key)
        if surf is None:
            surf = mask.to_surface(setcolor=tuple(color) + (255,), unsetcolor=(0, 0, 0, 0))
            if self.scale != 1:
                w, h = surf.get_size()
                surf = pygame.transform.scale(surf, (w * self.scale, h * self.scale))
            self._glyphs[key] = surf
        return surf

    def render(self, text: str, color=palette.WHITE, shadow=None, mono: bool = False) -> pygame.Surface:
        """Return a surface containing one line of text (cached)."""
        key = (text, tuple(color), tuple(shadow) if shadow else None, mono)
        cached = self._cache.get(key)
        if cached is not None:
            return cached
        s = self.scale
        spans = parse_markup(text, color) if "<" in text else [(text, tuple(color))]
        width = max(1, self.width(text, mono))
        pad = s if shadow else 0
        surf = pygame.Surface((width + pad, GLYPH_ROWS * s + pad), pygame.SRCALPHA)
        passes = [(shadow, pad)] if shadow else []
        passes.append((None, 0))
        for override, offset in passes:
            x = 0
            for chunk, span_color in spans:
                col = override or span_color
                for ch in chunk:
                    glyph = self._glyph(ch, col)
                    adv = self.char_width(ch, mono) * s
                    if glyph is not None:
                        gx = x
                        if mono:
                            gx += (MONO_ADVANCE * s - glyph.get_width()) // 2
                        surf.blit(glyph, (gx + offset, offset))
                    x += adv
        if len(self._cache) > 1500:
            self._cache.clear()
        self._cache[key] = surf
        return surf

    def draw(self, target: pygame.Surface, text: str, pos, color=palette.WHITE, shadow=None,
             align: str = "left", mono: bool = False) -> pygame.Rect:
        """Blit ``text`` at ``pos``; ``align`` is left / center / right."""
        if not text:
            return pygame.Rect(pos[0], pos[1], 0, self.line_height)
        surf = self.render(text, color, shadow, mono)
        x, y = int(pos[0]), int(pos[1])
        w = self.width(text, mono)
        if align == "center":
            x -= w // 2
        elif align == "right":
            x -= w
        target.blit(surf, (x, y))
        return pygame.Rect(x, y, w, self.line_height)

    # -- layout --------------------------------------------------------------
    def wrap(self, text: str, max_width: int, mono: bool = False) -> List[str]:
        """Word-wrap ``text`` to ``max_width`` pixels.

        Colour tags survive wrapping: a tag left open at the end of a line is
        re-opened at the start of the next one.
        """
        lines: List[str] = []
        for paragraph in text.split("\n"):
            words = paragraph.split(" ")
            current = ""
            open_tags: List[str] = []
            for word in words:
                trial = word if not current else current + " " + word
                if current and self.width(trial, mono) > max_width:
                    lines.append(current + "".join("</%s>" % t for t in reversed(open_tags)))
                    current = "".join("<%s>" % t for t in open_tags) + word
                else:
                    current = trial
                open_tags = _open_tags(current)
            lines.append(current + "".join("</%s>" % t for t in reversed(open_tags)))
        return lines

    def draw_wrapped(self, target, text, rect, color=palette.WHITE, shadow=None,
                     line_gap: int = 0, max_lines: Optional[int] = None) -> int:
        """Draw wrapped text inside ``rect``; returns the height used."""
        rect = pygame.Rect(rect)
        y = rect.y
        lines = self.wrap(text, rect.w)
        if max_lines is not None:
            lines = lines[:max_lines]
        for line in lines:
            self.draw(target, line, (rect.x, y), color, shadow)
            y += self.line_height + line_gap
        return y - rect.y


def _open_tags(text: str) -> List[str]:
    """Colour tags still open at the end of ``text``."""
    tags: List[str] = []
    i = 0
    while True:
        i = text.find("<", i)
        if i == -1:
            break
        if text.startswith("<<", i):
            i += 2
            continue
        end = text.find(">", i)
        if end == -1:
            break
        tag = text[i + 1:end]
        if tag in palette.TEXT_COLORS:
            tags.append(tag)
        elif tag.startswith("/") and tags:
            tags.pop()
        i = end + 1
    return tags


def visible_prefix(text: str, count: int) -> str:
    """First ``count`` visible characters of marked-up text (for typewriter
    effects) with any open colour tag closed again."""
    out: List[str] = []
    shown = 0
    i, n = 0, len(text)
    while i < n and shown < count:
        if text[i] == "<":
            if text.startswith("<<", i):
                out.append("<<")
                shown += 1
                i += 2
                continue
            end = text.find(">", i)
            if end != -1:
                tag = text[i + 1:end]
                if tag in palette.TEXT_COLORS or tag.startswith("/"):
                    out.append(text[i:end + 1])
                    i = end + 1
                    continue
        out.append(text[i])
        shown += 1
        i += 1
    result = "".join(out)
    return result + "".join("</%s>" % t for t in reversed(_open_tags(result)))


_FONTS: Dict[int, BitmapFont] = {}


def get_font(scale: int = 1) -> BitmapFont:
    """Shared font instance for a given integer scale."""
    font = _FONTS.get(scale)
    if font is None:
        font = _FONTS[scale] = BitmapFont(scale)
    return font
