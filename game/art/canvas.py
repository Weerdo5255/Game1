"""A tiny pixel-art drawing surface.

All procedural art in the game (tiles, props, icons, sprites) is painted
through this class, which wraps a ``pygame.Surface`` with the handful of
operations pixel art needs: single pixels, rectangles, lines, dithering,
speckle noise and "ASCII art" grids.  Drawing code receives a ``Canvas`` and
never touches pygame directly, which keeps the art definitions importable
without a display.
"""
from __future__ import annotations

import random
from typing import Dict, Iterable, Optional, Sequence, Tuple

import pygame

Color = Tuple[int, ...]


class Canvas:
    def __init__(self, w: int, h: int, seed: int = 0, frame: int = 0, variant: int = 0, mask: int = 0):
        self.w = w
        self.h = h
        self.surface = pygame.Surface((w, h), pygame.SRCALPHA)
        self.rng = random.Random(seed)
        self.frame = frame        # animation frame being drawn
        self.variant = variant    # random variation index
        self.mask = mask          # autotile neighbour bits (see tiles.py)

    # -- primitives ----------------------------------------------------------
    def fill(self, color: Color) -> None:
        self.surface.fill(color)

    def px(self, x: int, y: int, color: Color) -> None:
        if 0 <= x < self.w and 0 <= y < self.h:
            self.surface.set_at((int(x), int(y)), color)

    def get(self, x: int, y: int) -> Color:
        if 0 <= x < self.w and 0 <= y < self.h:
            return tuple(self.surface.get_at((int(x), int(y))))
        return (0, 0, 0, 0)

    def rect(self, x: int, y: int, w: int, h: int, color: Color) -> None:
        if w > 0 and h > 0:
            self.surface.fill(color, pygame.Rect(x, y, w, h))

    def frame_rect(self, x: int, y: int, w: int, h: int, color: Color) -> None:
        """One pixel wide rectangle outline."""
        self.rect(x, y, w, 1, color)
        self.rect(x, y + h - 1, w, 1, color)
        self.rect(x, y, 1, h, color)
        self.rect(x + w - 1, y, 1, h, color)

    def hline(self, x: int, y: int, w: int, color: Color) -> None:
        self.rect(x, y, w, 1, color)

    def vline(self, x: int, y: int, h: int, color: Color) -> None:
        self.rect(x, y, 1, h, color)

    def line(self, x0: int, y0: int, x1: int, y1: int, color: Color) -> None:
        pygame.draw.line(self.surface, color, (x0, y0), (x1, y1))

    def circle(self, cx: float, cy: float, r: float, color: Color, width: int = 0) -> None:
        pygame.draw.circle(self.surface, color, (cx, cy), r, width)

    def ellipse(self, x: int, y: int, w: int, h: int, color: Color, width: int = 0) -> None:
        pygame.draw.ellipse(self.surface, color, pygame.Rect(x, y, w, h), width)

    def poly(self, points: Sequence[Tuple[int, int]], color: Color) -> None:
        pygame.draw.polygon(self.surface, color, points)

    def clear(self, x: int, y: int, w: int, h: int) -> None:
        """Punch a transparent hole."""
        self.rect(x, y, w, h, (0, 0, 0, 0))

    # -- texture helpers -----------------------------------------------------
    def dither(self, x: int, y: int, w: int, h: int, color: Color, phase: int = 0) -> None:
        """Checkerboard dither of ``color`` over whatever is already there."""
        for j in range(y, y + h):
            for i in range(x, x + w):
                if (i + j + phase) % 2 == 0:
                    self.px(i, j, color)

    def speckle(self, x: int, y: int, w: int, h: int, colors: Sequence[Color], count: int) -> None:
        """Scatter ``count`` random single pixels chosen from ``colors``."""
        for _ in range(count):
            self.px(x + self.rng.randrange(w), y + self.rng.randrange(h), self.rng.choice(list(colors)))

    def vgrad(self, x: int, y: int, w: int, h: int, colors: Sequence[Color]) -> None:
        """Vertical bands running through ``colors`` top to bottom."""
        n = len(colors)
        for j in range(h):
            self.rect(x, y + j, w, 1, colors[min(n - 1, j * n // max(1, h))])

    def hgrad(self, x: int, y: int, w: int, h: int, colors: Sequence[Color]) -> None:
        n = len(colors)
        for i in range(w):
            self.rect(x + i, y, 1, h, colors[min(n - 1, i * n // max(1, w))])

    def grid(self, rows: Iterable[str], pal: Dict[str, Optional[Color]], ox: int = 0, oy: int = 0) -> None:
        """Paint an ASCII picture: each character is looked up in ``pal``.

        Characters missing from the palette (and '.' / ' ') are transparent.
        """
        for j, row in enumerate(rows):
            for i, ch in enumerate(row):
                color = pal.get(ch)
                if color is not None:
                    self.px(ox + i, oy + j, color)

    def outline(self, color: Color) -> None:
        """Add a one pixel outline around everything opaque."""
        src = self.surface.copy()
        mask = pygame.mask.from_surface(src, 1)
        w, h = self.w, self.h
        for y in range(h):
            for x in range(w):
                if mask.get_at((x, y)):
                    continue
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    nx, ny = x + dx, y + dy
                    if 0 <= nx < w and 0 <= ny < h and mask.get_at((nx, ny)):
                        self.surface.set_at((x, y), color)
                        break

    def blit(self, other, x: int = 0, y: int = 0) -> None:
        surf = other.surface if isinstance(other, Canvas) else other
        self.surface.blit(surf, (x, y))

    def sub(self, x: int, y: int, w: int, h: int) -> pygame.Surface:
        """Copy of a region as a new surface."""
        out = pygame.Surface((w, h), pygame.SRCALPHA)
        out.blit(self.surface, (0, 0), pygame.Rect(x, y, w, h))
        return out

    def flipped(self, flip_x: bool = True, flip_y: bool = False) -> pygame.Surface:
        return pygame.transform.flip(self.surface, flip_x, flip_y)


def with_alpha(color: Color, alpha: int) -> Color:
    return (color[0], color[1], color[2], alpha)
