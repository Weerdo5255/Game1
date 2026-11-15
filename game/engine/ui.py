"""Low-resolution UI widgets shared by every scene.

Everything here draws straight onto the 384x216 virtual framebuffer with the
bitmap font: panels, value bars, list menus, a typewriter text box, toast
notifications and a one-line text entry field.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Callable, List, Optional, Sequence, Tuple

import pygame

from . import palette as P
from .font import BitmapFont, get_font, strip_markup, visible_prefix

_audio = None


def set_audio(audio) -> None:
    """Give the widgets a sound device (the App calls this once)."""
    global _audio
    _audio = audio


def sound(name: str) -> None:
    if _audio is not None:
        _audio.play(name)


# ---------------------------------------------------------------------------
# Drawing helpers
# ---------------------------------------------------------------------------
def panel(surf, rect, title: Optional[str] = None, fill=None, border=None,
          title_color=None, font: Optional[BitmapFont] = None) -> pygame.Rect:
    """Draw a bordered window and return its inner content rectangle."""
    rect = pygame.Rect(rect)
    fill = fill or P.UI["panel"]
    border = border or P.UI["border"]
    x, y, w, h = rect
    surf.fill(P.BLACK, (x + 1, y, w - 2, h))
    surf.fill(P.BLACK, (x, y + 1, w, h - 2))
    surf.fill(border, (x + 2, y + 1, w - 4, h - 2))
    surf.fill(border, (x + 1, y + 2, w - 2, h - 4))
    surf.fill(fill, (x + 2, y + 2, w - 4, h - 4))
    surf.fill(P.lerp(fill, P.WHITE, 0.08), (x + 2, y + 2, w - 4, 1))
    surf.fill(P.lerp(fill, P.BLACK, 0.25), (x + 2, y + h - 3, w - 4, 1))
    # corner studs
    for cx, cy in ((x + 2, y + 2), (x + w - 3, y + 2), (x + 2, y + h - 3), (x + w - 3, y + h - 3)):
        surf.set_at((cx, cy), border)
    if title:
        font = font or get_font()
        tw = font.width(title)
        surf.fill(P.BLACK, (x + 6, y, tw + 8, 9))
        surf.fill(border, (x + 7, y, tw + 6, 9))
        surf.fill(P.NAVY[0], (x + 8, y, tw + 4, 8))
        font.draw(surf, title, (x + 10, y), title_color or P.UI["accent"])
    return pygame.Rect(x + 5, y + 5 + (4 if title else 0), w - 10, h - 10 - (4 if title else 0))


def bar(surf, rect, frac: float, color, back=None, border=None) -> None:
    """A segmented value bar (frac 0..1)."""
    rect = pygame.Rect(rect)
    frac = max(0.0, min(1.0, frac))
    surf.fill(border or P.BLACK, rect)
    inner = rect.inflate(-2, -2)
    surf.fill(back or P.NAVY[0], inner)
    w = int(round(inner.w * frac))
    if w > 0:
        surf.fill(color, (inner.x, inner.y, w, inner.h))
        surf.fill(P.lerp(tuple(color), P.WHITE, 0.35), (inner.x, inner.y, w, 1))
    for sx in range(inner.x + 4, inner.right, 5):
        surf.fill(border or P.BLACK, (sx, inner.y, 1, inner.h))


def level_color(frac: float):
    """Green / amber / red for a 0..1 'health' style value."""
    if frac >= 0.6:
        return P.UI["good"]
    if frac >= 0.3:
        return P.UI["warn"]
    return P.UI["bad"]


def shade(surf, alpha: int = 140, color=P.BLACK) -> None:
    """Darken the whole surface (used behind pop-up windows)."""
    veil = pygame.Surface(surf.get_size(), pygame.SRCALPHA)
    veil.fill((color[0], color[1], color[2], alpha))
    surf.blit(veil, (0, 0))


def key_hints(surf, hints: Sequence[Tuple[str, str]], y: Optional[int] = None,
              font: Optional[BitmapFont] = None, align: str = "right") -> None:
    """Row of "[Key] action" hints along the bottom of the screen."""
    font = font or get_font()
    parts = ["<y>%s</y> %s" % (k, v) for k, v in hints]
    text = "   ".join(parts)
    w = font.width(text)
    sw, sh = surf.get_size()
    y = sh - 11 if y is None else y
    x = sw - w - 5 if align == "right" else (sw - w) // 2 if align == "center" else 5
    font.draw(surf, text, (x, y), P.UI["dim"], shadow=P.BLACK)


def cursor(surf, x: int, y: int, t: float, color=None, font: Optional[BitmapFont] = None) -> None:
    font = font or get_font()
    bob = 1 if int(t * 4) % 2 else 0
    font.draw(surf, "▶", (x + bob, y), color or P.UI["select"])


# ---------------------------------------------------------------------------
# Menu
# ---------------------------------------------------------------------------
@dataclass
class MenuItem:
    label: str
    right: str = ""            # right-aligned value text
    enabled: bool = True
    data: Any = None
    color: Any = None
    hint: str = ""             # longer description for a detail pane


class Menu:
    """A vertical list with a cursor, scrolling and optional wrap-around."""

    def __init__(self, items: Sequence = (), visible: int = 8, wrap: bool = True):
        self.items: List[MenuItem] = []
        self.visible = visible
        self.wrap = wrap
        self.index = 0
        self.scroll = 0
        self.set_items(items)

    def set_items(self, items: Sequence, keep_index: bool = True) -> None:
        old = self.index if keep_index else 0
        self.items = [i if isinstance(i, MenuItem) else MenuItem(str(i)) for i in items]
        self.index = max(0, min(old, len(self.items) - 1))
        self._clamp_scroll()

    @property
    def selected(self) -> Optional[MenuItem]:
        if 0 <= self.index < len(self.items):
            return self.items[self.index]
        return None

    def _clamp_scroll(self) -> None:
        if self.index < self.scroll:
            self.scroll = self.index
        elif self.index >= self.scroll + self.visible:
            self.scroll = self.index - self.visible + 1
        self.scroll = max(0, min(self.scroll, max(0, len(self.items) - self.visible)))

    def move(self, delta: int) -> None:
        n = len(self.items)
        if n == 0:
            return
        new = self.index + delta
        if self.wrap:
            new %= n
        else:
            new = max(0, min(n - 1, new))
        if new != self.index:
            self.index = new
            self._clamp_scroll()
            sound("move")

    def update(self, inp) -> Optional[str]:
        """Handle navigation; returns "select", "cancel" or None."""
        if inp.repeated("up"):
            self.move(-1)
        elif inp.repeated("down"):
            self.move(1)
        if inp.pressed("confirm"):
            item = self.selected
            if item is not None and item.enabled:
                sound("confirm")
                return "select"
            sound("error")
        elif inp.pressed("cancel"):
            sound("cancel")
            return "cancel"
        return None

    def height(self, font: Optional[BitmapFont] = None) -> int:
        font = font or get_font()
        return min(len(self.items), self.visible) * (font.line_height + 1)

    def draw(self, surf, x: int, y: int, w: int, t: float = 0.0, focused: bool = True,
             font: Optional[BitmapFont] = None) -> None:
        font = font or get_font()
        row_h = font.line_height + 1
        for row in range(self.visible):
            i = self.scroll + row
            if i >= len(self.items):
                break
            item = self.items[i]
            ry = y + row * row_h
            selected = i == self.index
            if selected and focused:
                surf.fill(P.NAVY[3], (x, ry - 1, w, row_h))
                surf.fill(P.NAVY[4], (x, ry - 1, w, 1))
                cursor(surf, x + 1, ry, t, font=font)
            if not item.enabled:
                color = P.UI["disabled"]
            elif selected and focused:
                color = P.UI["select"]
            else:
                color = item.color or P.UI["text"]
            label = item.label if item.enabled else strip_markup(item.label)
            font.draw(surf, label, (x + 8, ry), color)
            if item.right:
                font.draw(surf, item.right, (x + w - 3, ry), color if not item.enabled else (item.color or P.UI["dim"]),
                          align="right")
        if self.scroll > 0:
            font.draw(surf, "▲", (x + w - 2, y - 7), P.UI["accent"], align="right")
        if self.scroll + self.visible < len(self.items):
            font.draw(surf, "▼", (x + w - 2, y + self.visible * row_h - 2), P.UI["accent"], align="right")


# ---------------------------------------------------------------------------
# Typewriter text
# ---------------------------------------------------------------------------
class Typewriter:
    """Wraps text into pages and reveals it a character at a time."""

    def __init__(self, text: str, width: int, lines_per_page: int = 4, cps: float = 55.0,
                 font: Optional[BitmapFont] = None):
        self.font = font or get_font()
        lines = self.font.wrap(text, width)
        self.pages: List[List[str]] = [lines[i:i + lines_per_page]
                                       for i in range(0, max(1, len(lines)), lines_per_page)] or [[""]]
        self.page = 0
        self.cps = cps
        self.shown = 0.0
        self._lengths = [[len(strip_markup(l)) for l in page] for page in self.pages]

    @property
    def page_length(self) -> int:
        return sum(self._lengths[self.page])

    @property
    def page_done(self) -> bool:
        return self.shown >= self.page_length

    @property
    def last_page(self) -> bool:
        return self.page >= len(self.pages) - 1

    def update(self, dt: float) -> int:
        """Advance; returns how many new characters appeared."""
        if self.page_done:
            return 0
        before = int(self.shown)
        self.shown = min(float(self.page_length), self.shown + dt * self.cps)
        return int(self.shown) - before

    def skip(self) -> None:
        self.shown = float(self.page_length)

    def next_page(self) -> bool:
        if self.last_page:
            return False
        self.page += 1
        self.shown = 0.0
        return True

    def draw(self, surf, x: int, y: int, color=None, shadow=None, line_gap: int = 1) -> None:
        remaining = int(self.shown)
        for line, length in zip(self.pages[self.page], self._lengths[self.page]):
            if remaining <= 0:
                break
            text = line if remaining >= length else visible_prefix(line, remaining)
            self.font.draw(surf, text, (x, y), color or P.UI["text"], shadow)
            remaining -= length
            y += self.font.line_height + line_gap


# ---------------------------------------------------------------------------
# Toast notifications
# ---------------------------------------------------------------------------
class Toasts:
    """Short messages that stack in a corner and fade away."""

    def __init__(self, lifetime: float = 3.2, limit: int = 5):
        self.lifetime = lifetime
        self.limit = limit
        self.items: List[List] = []      # [text, color, age]

    def add(self, text: str, color=None) -> None:
        if self.items and self.items[-1][0] == text and self.items[-1][2] < 1.0:
            return
        self.items.append([text, color or P.UI["text"], 0.0])
        if len(self.items) > self.limit:
            self.items.pop(0)

    def update(self, dt: float) -> None:
        for item in self.items:
            item[2] += dt
        self.items = [i for i in self.items if i[2] < self.lifetime]

    def clear(self) -> None:
        self.items = []

    def draw(self, surf, right: int, top: int, font: Optional[BitmapFont] = None) -> None:
        font = font or get_font()
        y = top
        for text, color, age in self.items:
            w = font.width(text) + 8
            slide = int(max(0.0, 1.0 - age * 6.0) * 24)
            x = right - w + slide
            box = pygame.Surface((w, 12), pygame.SRCALPHA)
            fade = 1.0 if age < self.lifetime - 0.5 else max(0.0, (self.lifetime - age) / 0.5)
            box.fill((P.NAVY[0][0], P.NAVY[0][1], P.NAVY[0][2], int(215 * fade)))
            box.fill((P.CYAN[1][0], P.CYAN[1][1], P.CYAN[1][2], int(255 * fade)), (0, 0, 2, 12))
            surf.blit(box, (x, y))
            if fade > 0.4:
                font.draw(surf, text, (x + 5, y + 1), color)
            y += 13


# ---------------------------------------------------------------------------
# Text entry
# ---------------------------------------------------------------------------
class TextEntry:
    def __init__(self, value: str = "", max_len: int = 16, allowed: Optional[Callable[[str], bool]] = None):
        self.value = value
        self.max_len = max_len
        self.allowed = allowed or (lambda ch: ch.isprintable() and ch not in "<>{}")

    def update(self, inp) -> None:
        if inp.text:
            for ch in inp.text:
                if len(self.value) < self.max_len and self.allowed(ch):
                    self.value += ch
        if pygame.K_BACKSPACE in inp.keys_pressed and self.value:
            self.value = self.value[:-1]

    def draw(self, surf, x: int, y: int, w: int, t: float, focused: bool = True,
             font: Optional[BitmapFont] = None) -> None:
        font = font or get_font()
        surf.fill(P.BLACK, (x, y, w, 13))
        surf.fill(P.UI["select"] if focused else P.UI["border_dim"], (x + 1, y + 1, w - 2, 11))
        surf.fill(P.NAVY[0], (x + 2, y + 2, w - 4, 9))
        end = font.draw(surf, self.value, (x + 4, y + 2), P.UI["text"])
        if focused and int(t * 2.5) % 2 == 0:
            surf.fill(P.UI["select"], (end.right + 1 if self.value else x + 4, y + 3, 1, 7))


def wrap_lines(text: str, width: int, font: Optional[BitmapFont] = None) -> List[str]:
    return (font or get_font()).wrap(text, width)


def draw_text_block(surf, text: str, rect, color=None, font: Optional[BitmapFont] = None,
                    max_lines: Optional[int] = None, line_gap: int = 1) -> int:
    """Wrapped paragraph inside ``rect``; returns the y coordinate below it."""
    font = font or get_font()
    rect = pygame.Rect(rect)
    y = rect.y
    lines = font.wrap(text, rect.w)
    if max_lines is not None:
        lines = lines[:max_lines]
    for line in lines:
        font.draw(surf, line, (rect.x, y), color or P.UI["text"])
        y += font.line_height + line_gap
    return y
