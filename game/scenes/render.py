"""Map renderer: tile layers, characters, lighting and the space backdrop.

Used by the overworld scene, by the map editor's preview and by the
``mapshot`` tool, so a map always looks the same everywhere.

Draw order: backdrop -> ground -> decor -> objects -> object sprites ->
characters (sorted by y) -> overhead -> lighting -> speech bubbles.
"""
from __future__ import annotations

import math
import random
from typing import Dict, Iterable, List, Optional, Tuple

import pygame

from .. import config
from ..art import tiles as tiledefs
from ..engine import palette as P
from ..engine.font import get_font

T = config.TILE

DEFAULT_AMBIENT = (218, 222, 236)
EMERGENCY_AMBIENT = (96, 42, 50)


class MapRenderer:
    def __init__(self, tile_art, char_art, icon_art):
        self.tiles = tile_art
        self.chars = char_art
        self.icons = icon_art
        self.font = get_font(1)
        self._stars: List[pygame.Surface] = []
        self._planet: Optional[pygame.Surface] = None
        self._lights: Dict[Tuple[int, Tuple[int, int, int]], pygame.Surface] = {}
        self._dark: Optional[pygame.Surface] = None
        self._shadow = self._make_shadow()
        self._wall_shade = self._make_wall_shade()
        self.lighting = True

    # -- cached bits ---------------------------------------------------------------
    @staticmethod
    def _make_shadow() -> pygame.Surface:
        s = pygame.Surface((12, 5), pygame.SRCALPHA)
        pygame.draw.ellipse(s, (0, 0, 0, 90), (0, 0, 12, 5))
        return s

    @staticmethod
    def _make_wall_shade() -> pygame.Surface:
        s = pygame.Surface((T, 4), pygame.SRCALPHA)
        s.fill((0, 0, 0, 70), (0, 0, T, 1))
        s.fill((0, 0, 0, 44), (0, 1, T, 1))
        for x in range(0, T, 2):
            s.set_at((x, 2), (0, 0, 0, 40))
            s.set_at((x + 1, 3), (0, 0, 0, 24))
        return s

    def _star_layers(self) -> List[pygame.Surface]:
        if not self._stars:
            rng = random.Random(1977)
            for count, colors in ((70, [P.NAVY[3], P.NAVY[4], P.STEEL[3]]),
                                  (26, [P.STEEL[5], P.WHITE, P.ICE[3], P.AMBER[4]])):
                layer = pygame.Surface((256, 256), pygame.SRCALPHA)
                for _ in range(count):
                    layer.set_at((rng.randrange(256), rng.randrange(256)), rng.choice(colors))
                if count < 40:
                    for _ in range(5):
                        x, y = rng.randrange(2, 254), rng.randrange(2, 254)
                        c = rng.choice([P.WHITE, P.ICE[4]])
                        layer.set_at((x, y), c)
                        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                            layer.set_at((x + dx, y + dy), P.NAVY[4])
                self._stars.append(layer)
        return self._stars

    def _planet_sprite(self) -> pygame.Surface:
        """A ringed gas giant for the sky (drawn once)."""
        if self._planet is None:
            w, h = 150, 84
            s = pygame.Surface((w, h), pygame.SRCALPHA)
            cx, cy, r = 75, 42, 30
            bands = [(196, 160, 104), (220, 186, 128), (176, 140, 92), (232, 204, 150),
                     (204, 170, 112), (160, 126, 86), (214, 180, 124)]
            ring_back = pygame.Surface((w, h), pygame.SRCALPHA)
            for rw, rh, col in ((146, 30, (170, 150, 120, 150)), (128, 25, (208, 190, 150, 190)),
                                (110, 20, (150, 132, 108, 150))):
                pygame.draw.ellipse(ring_back, col, (cx - rw // 2, cy - rh // 2, rw, rh), 3)
            s.blit(ring_back, (0, 0))
            for y in range(-r, r + 1):
                half = int(math.sqrt(max(0, r * r - y * y)))
                col = bands[((y + r) * len(bands)) // (2 * r + 1)]
                for x in range(-half, half + 1):
                    shade = 1.0 - max(0.0, (x + y * 0.4 + 10) / (r * 1.8)) * 0.75
                    dither = ((x + y) % 2) * 0.06
                    k = max(0.15, min(1.0, shade + dither))
                    s.set_at((cx + x, cy + y), (int(col[0] * k), int(col[1] * k), int(col[2] * k)))
            ring_front = pygame.Surface((w, h), pygame.SRCALPHA)
            ring_front.blit(ring_back, (0, 0))
            ring_front.fill((0, 0, 0, 0), (0, 0, w, cy + 2))
            s.blit(ring_front, (0, 0))
            self._planet = s
        return self._planet

    def _light_sprite(self, radius: int, color) -> pygame.Surface:
        key = (radius, tuple(color))
        spr = self._lights.get(key)
        if spr is None:
            size = radius * 2
            spr = pygame.Surface((size, size))
            spr.fill((0, 0, 0))
            steps = 5
            for i in range(steps):
                k = (i + 1) / steps
                r = int(radius * (1.0 - i / steps))
                c = (int(color[0] * k * 0.55), int(color[1] * k * 0.55), int(color[2] * k * 0.55))
                pygame.draw.circle(spr, c, (radius, radius), r)
            self._lights[key] = spr
        return spr

    # -- helpers -------------------------------------------------------------------
    @staticmethod
    def camera_for(gmap, focus_px: float, focus_py: float, view_w: int, view_h: int) -> Tuple[int, int]:
        """Camera top-left so that the focus point is centred, clamped to the map."""
        mw, mh = gmap.width * T, gmap.height * T
        if mw <= view_w:
            cx = -(view_w - mw) // 2
        else:
            cx = int(max(0, min(mw - view_w, focus_px - view_w // 2)))
        if mh <= view_h:
            cy = -(view_h - mh) // 2
        else:
            cy = int(max(0, min(mh - view_h, focus_py - view_h // 2)))
        return cx, cy

    def ambient_for(self, gmap, light_level: float = 1.0):
        base = tuple((gmap.props.get("ambient") or DEFAULT_AMBIENT)[:3])
        if gmap.props.get("own_light"):
            return base
        k = max(0.0, min(1.0, light_level))
        return P.lerp(EMERGENCY_AMBIENT, base, k)

    # -- drawing -------------------------------------------------------------------
    def draw_backdrop(self, surf, gmap, cam_x: int, cam_y: int, time: float) -> None:
        backdrop = gmap.props.get("backdrop", "space")
        if backdrop == "none":
            surf.fill(P.BLACK)
            return
        if backdrop == "ice":
            surf.fill((16, 26, 50))
            return
        surf.fill(P.SPACE)
        w, h = surf.get_size()
        for i, layer in enumerate(self._star_layers()):
            par = 0.08 + 0.1 * i
            ox = int(-(cam_x * par + time * (0.5 + i))) % 256
            oy = int(-(cam_y * par)) % 256
            for y in range(oy - 256, h, 256):
                for x in range(ox - 256, w, 256):
                    surf.blit(layer, (x, y))
        if backdrop == "saturn":
            px = int(w * 0.52 - cam_x * 0.05)
            py = int(h * 0.12 - cam_y * 0.05)
            surf.blit(self._planet_sprite(), (px, py))

    def draw_layer(self, surf, gmap, layer: str, cam_x: int, cam_y: int, time: float) -> None:
        rows = gmap.layers[layer]
        w, h = surf.get_size()
        x0 = max(0, cam_x // T)
        y0 = max(0, cam_y // T)
        x1 = min(gmap.width, (cam_x + w) // T + 1)
        y1 = min(gmap.height, (cam_y + h) // T + 1)
        get = self.tiles.get
        defs = tiledefs.TILES
        ground = layer == "ground"
        for y in range(y0, y1):
            row = rows[y]
            sy = y * T - cam_y
            for x in range(x0, x1):
                name = row[x]
                if not name:
                    continue
                tdef = defs.get(name)
                if tdef is None:
                    surf.blit(get(name), (x * T - cam_x, sy))
                    continue
                frame = int(time * tdef.fps) % tdef.frames if tdef.fps > 0 and tdef.frames > 1 else 0
                variant = ((x * 73856093) ^ (y * 19349663)) % tdef.variants if tdef.variants > 1 else 0
                mask = gmap.mask(layer, x, y) if tdef.group else 0
                surf.blit(get(name, frame, variant, mask), (x * T - cam_x, sy))
                if ground and y > 0 and not tdef.solid:
                    above = defs.get(gmap.layers["ground"][y - 1][x])
                    if above is not None and above.solid and above.group == "wall":
                        surf.blit(self._wall_shade, (x * T - cam_x, sy))

    def draw_objects(self, surf, gmap, cam_x: int, cam_y: int, time: float, world=None) -> None:
        w, h = surf.get_size()
        for obj in gmap.objects:
            sx, sy = obj.x * T - cam_x, obj.y * T - cam_y
            if sx < -T or sy < -T or sx > w or sy > h:
                continue
            if obj.type == "item":
                item_id = obj.get("item")
                idef = world.content.items.get(item_id) if world is not None else None
                bob = int(math.sin(time * 3.0 + obj.x) * 1.5)
                if idef is not None:
                    surf.blit(self._shadow, (sx + 2, sy + 11))
                    surf.blit(self.icons.get(idef.icon, idef.color), (sx, sy - 2 + bob))
                continue
            sprite = obj.get("sprite")
            if not sprite and obj.type == "door":
                sprite = "door"
            if sprite and sprite in tiledefs.TILES:
                tdef = tiledefs.TILES[sprite]
                if obj.type in ("door", "portal") and tdef.frames >= 3 and tdef.fps == 0:
                    frame = 0 if obj.anim < 0.34 else 1 if obj.anim < 0.75 else 2
                else:
                    frame = self.tiles.frame_at(sprite, time)
                surf.blit(self.tiles.get(sprite, frame), (sx, sy))
                if obj.type in ("door", "portal") and tdef.category == "door" and obj.get("locked"):
                    lamp = P.RED[3] if int(time * 2) % 2 else P.RED[1]
                    surf.fill(lamp, (sx + 7, sy + 2, 2, 2))

    def draw_characters(self, surf, world, chars: Iterable, cam_x: int, cam_y: int, time: float) -> None:
        w, h = surf.get_size()
        order = sorted(chars, key=lambda c: (c.pixel_pos()[1], c.is_player))
        for ch in order:
            if ch.asleep:
                bed = world.find_object(ch.ai.get("bed", "")) if world is not None and ch.ai.get("bed") else None
                if bed is not None and bed.map_id == ch.map_id:
                    bx, by = bed.x * T - cam_x, bed.y * T - cam_y
                    surf.blit(self.chars.sleeping(ch.appearance), (bx, by + 1))
                    self._zzz(surf, bx + 11, by - 2, time)
                    continue
            px, py = ch.pixel_pos()
            sx, sy = int(px) - cam_x, int(py) - cam_y
            if sx < -T * 2 or sy < -T * 2 or sx > w + T or sy > h + T:
                continue
            surf.blit(self._shadow, (sx + 2, sy + 12))
            frame = 0
            if ch.moving:
                frame = int(ch.anim_time * 7.0) % 4
            drone = ch.appearance.get("body") == "drone"
            hover = int(math.sin(time * 4.0 + (hash(ch.id) & 7)) * 1.5) - 2 if drone else 0
            if drone and not ch.moving:
                frame = int(time * 3.0) % 4
            sprite = self.chars.sprite(ch.appearance, ch.facing, frame)
            surf.blit(sprite, (sx, sy - (config.CHAR_H - T) + hover))
            if ch.asleep:
                self._zzz(surf, sx + 11, sy - 12, time)

    def _zzz(self, surf, x: int, y: int, time: float) -> None:
        phase = int(time * 1.5) % 3
        self.font.draw(surf, "z", (x + phase, y - phase * 2), P.ICE[4], shadow=P.BLACK)
        if phase:
            self.font.draw(surf, "z", (x + 4 + phase, y - 5 - phase), P.ICE[3], shadow=P.BLACK)

    def draw_lighting(self, surf, gmap, cam_x: int, cam_y: int, ambient,
                      extra_lights: Iterable[Tuple[float, float, int, Tuple[int, int, int]]] = ()) -> None:
        if not self.lighting:
            return
        w, h = surf.get_size()
        if self._dark is None or self._dark.get_size() != (w, h):
            self._dark = pygame.Surface((w, h))
        dark = self._dark
        dark.fill(ambient)
        brightness = sum(ambient) / 3.0
        if brightness < 232:
            for lx, ly, radius, color in list(gmap.lights()) + list(extra_lights):
                sx, sy = lx - cam_x, ly - cam_y
                if sx < -radius or sy < -radius or sx > w + radius or sy > h + radius:
                    continue
                dark.blit(self._light_sprite(int(radius), color), (int(sx - radius), int(sy - radius)),
                          special_flags=pygame.BLEND_RGB_ADD)
        surf.blit(dark, (0, 0), special_flags=pygame.BLEND_RGB_MULT)

    def draw_barks(self, surf, chars: Iterable, cam_x: int, cam_y: int) -> None:
        w, _h = surf.get_size()
        for ch in chars:
            if not ch.bark or ch.bark_time <= 0:
                continue
            px, py = ch.pixel_pos()
            text = ch.bark if len(ch.bark) <= 46 else ch.bark[:44] + "…"
            tw = self.font.width(text)
            bx = int(px) - cam_x + 8 - tw // 2 - 3
            bx = max(2, min(w - tw - 8, bx))
            by = int(py) - cam_y - 26
            pygame.draw.rect(surf, P.BLACK, (bx - 1, by - 1, tw + 8, 13))
            pygame.draw.rect(surf, P.WHITE, (bx, by, tw + 6, 11))
            surf.set_at((bx, by), P.BLACK)
            surf.set_at((bx + tw + 5, by), P.BLACK)
            tail_x = max(bx + 3, min(bx + tw, int(px) - cam_x + 7))
            surf.fill(P.WHITE, (tail_x, by + 11, 2, 1))
            surf.fill(P.BLACK, (tail_x - 1, by + 11, 1, 2))
            surf.fill(P.BLACK, (tail_x + 2, by + 11, 1, 2))
            surf.fill(P.WHITE, (tail_x, by + 12, 1, 1))
            self.font.draw(surf, text, (bx + 3, by + 1), P.NAVY[1])

    def draw(self, surf, gmap, cam_x: int, cam_y: int, time: float, world=None,
             chars: Optional[List] = None, light_level: float = 1.0, extra_lights=()) -> None:
        """Render the whole scene for one map."""
        self.draw_backdrop(surf, gmap, cam_x, cam_y, time)
        for layer in ("ground", "decor", "objects"):
            self.draw_layer(surf, gmap, layer, cam_x, cam_y, time)
        self.draw_objects(surf, gmap, cam_x, cam_y, time, world)
        chars = chars or []
        self.draw_characters(surf, world, chars, cam_x, cam_y, time)
        self.draw_layer(surf, gmap, "overhead", cam_x, cam_y, time)
        ambient = self.ambient_for(gmap, light_level)
        lights = list(extra_lights)
        for ch in chars:
            px, py = ch.pixel_pos()
            lights.append((px + 8, py + 4, 30, (120, 120, 130)))
        self.draw_lighting(surf, gmap, cam_x, cam_y, ambient, lights)
        self.draw_barks(surf, chars, cam_x, cam_y)

    def render_whole_map(self, gmap, world=None, chars: Optional[List] = None, time: float = 0.0,
                         light_level: float = 1.0) -> pygame.Surface:
        """Full-size picture of a map (editor thumbnails, ``mapshot``)."""
        surf = pygame.Surface((gmap.width * T, gmap.height * T))
        self.draw(surf, gmap, 0, 0, time, world, chars, light_level)
        return surf
