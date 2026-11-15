"""The application: window, main loop and scene stack.

The game draws into a small fixed-size framebuffer (``config.VIEW_W`` x
``config.VIEW_H``) which is then scaled to the window using the largest
integer factor that fits, with letterboxing.  ``App(headless=True)`` runs
without a visible window or audio so tests and tools can drive real scenes.
"""
from __future__ import annotations

import json
import os
from typing import Callable, List, Optional

from .. import config

DEFAULT_OPTIONS = {
    "scale": config.DEFAULT_SCALE,
    "fullscreen": False,
    "volume": 0.6,
    "text_speed": 1,       # 0 slow, 1 normal, 2 fast, 3 instant
    "lighting": True,
}
TEXT_SPEEDS = [28.0, 55.0, 110.0, 100000.0]
TEXT_SPEED_NAMES = ["Slow", "Normal", "Fast", "Instant"]


def options_path() -> str:
    return os.path.join(config.save_dir(), "options.json")


def load_options() -> dict:
    opts = dict(DEFAULT_OPTIONS)
    try:
        with open(options_path(), "r", encoding="utf8") as fh:
            data = json.load(fh)
        if isinstance(data, dict):
            opts.update({k: v for k, v in data.items() if k in DEFAULT_OPTIONS})
    except (OSError, ValueError):
        pass
    return opts


def save_options(opts: dict) -> None:
    try:
        os.makedirs(config.save_dir(), exist_ok=True)
        with open(options_path(), "w", encoding="utf8") as fh:
            json.dump(opts, fh, indent=1)
    except OSError:
        pass


class App:
    def __init__(self, headless: bool = False, scale: Optional[int] = None, audio: bool = True,
                 caption: str = "Tethys Deep", content=None, persist_options: bool = True):
        if headless:
            os.environ["SDL_VIDEODRIVER"] = "dummy"
            os.environ["SDL_AUDIODRIVER"] = "dummy"
        os.environ.setdefault("PYGAME_HIDE_SUPPORT_PROMPT", "1")
        import pygame
        from . import ui
        from .audio import Audio
        from .font import get_font
        from .input import Input
        from ..art.chars import CharArt
        from ..art.icons import IconArt
        from ..art.tiles import TileArt

        self.pygame = pygame
        self.headless = headless
        self.persist_options = persist_options and not headless
        self.options = load_options() if self.persist_options else dict(DEFAULT_OPTIONS)
        if scale:
            self.options["scale"] = int(scale)
        pygame.init()
        self.fullscreen = bool(self.options.get("fullscreen")) and not headless
        self.window = None
        self._apply_video_mode()
        pygame.display.set_caption(caption)
        self.frame = pygame.Surface((config.VIEW_W, config.VIEW_H))
        self.clock = pygame.time.Clock()
        self.input = Input()
        self.audio = Audio(enabled=audio and not headless, volume=float(self.options.get("volume", 0.6)))
        ui.set_audio(self.audio)
        self.font = get_font(1)
        self.font_big = get_font(2)
        self.tiles = TileArt()
        self.chars = CharArt()
        self.icons = IconArt()
        self.content = content
        self.scenes: List = []
        self.running = True
        self.time = 0.0
        self.frame_count = 0
        self._fade = None
        self._view_rect = pygame.Rect(0, 0, config.VIEW_W, config.VIEW_H)
        self._view_scale = 1
        try:
            icon = pygame.transform.scale(self.tiles.get("crystal"), (32, 32))
            pygame.display.set_icon(icon)
        except pygame.error:
            pass

    # -- video ---------------------------------------------------------------
    def _apply_video_mode(self) -> None:
        pygame = self.pygame
        if self.fullscreen:
            self.window = pygame.display.set_mode((0, 0), pygame.FULLSCREEN)
        else:
            s = max(1, int(self.options.get("scale", config.DEFAULT_SCALE)))
            self.window = pygame.display.set_mode((config.VIEW_W * s, config.VIEW_H * s), pygame.RESIZABLE)

    def toggle_fullscreen(self) -> None:
        if self.headless:
            return
        self.fullscreen = not self.fullscreen
        self.options["fullscreen"] = self.fullscreen
        self._apply_video_mode()
        self.save_options()

    def set_scale(self, scale: int) -> None:
        self.options["scale"] = max(1, min(8, int(scale)))
        if not self.fullscreen and not self.headless:
            self._apply_video_mode()
        self.save_options()

    def save_options(self) -> None:
        if self.persist_options:
            save_options(self.options)

    @property
    def text_cps(self) -> float:
        return TEXT_SPEEDS[max(0, min(len(TEXT_SPEEDS) - 1, int(self.options.get("text_speed", 1))))]

    # -- scene stack ---------------------------------------------------------
    @property
    def scene(self):
        return self.scenes[-1] if self.scenes else None

    def push_scene(self, scene) -> None:
        self.scenes.append(scene)
        scene.enter()

    def pop_scene(self, scene=None) -> None:
        """Remove ``scene`` (default: the top one) and everything above it."""
        if not self.scenes:
            return
        if scene is None:
            scene = self.scenes[-1]
        if scene not in self.scenes:
            return
        while self.scenes:
            top = self.scenes.pop()
            top.exit()
            if top is scene:
                break
        if self.scenes:
            self.scenes[-1].resume()

    def set_scene(self, scene) -> None:
        """Replace the whole stack with ``scene``."""
        while self.scenes:
            self.scenes.pop().exit()
        self.push_scene(scene)

    def fade(self, midpoint: Optional[Callable[[], None]] = None, duration: float = 0.28,
             color=(8, 8, 14), hold: float = 0.0) -> None:
        """Fade out, run ``midpoint`` while the screen is covered, fade in."""
        if self.headless and duration > 0:
            duration = 0.0
        if duration <= 0:
            if midpoint:
                midpoint()
            return
        self._fade = {"t": 0.0, "dur": duration, "phase": "out", "fn": midpoint,
                      "color": color, "hold": hold}

    @property
    def fading(self) -> bool:
        return self._fade is not None

    # -- main loop -----------------------------------------------------------
    def run(self) -> None:
        pygame = self.pygame
        while self.running and self.scenes:
            dt = min(0.1, self.clock.tick(config.FPS) / 1000.0)
            self.frame_step(dt, pygame.event.get())
        pygame.quit()

    def quit(self) -> None:
        self.running = False

    def frame_step(self, dt: float, events=()) -> None:
        """Advance the application by one frame."""
        pygame = self.pygame
        self.time += dt
        self.frame_count += 1
        inp = self.input
        if events is not None:
            inp.begin_frame()
        for event in events or ():
            if event.type == pygame.QUIT:
                self.running = False
            elif event.type == pygame.WINDOWFOCUSLOST:
                inp.reset()
            elif event.type == pygame.VIDEORESIZE and not self.fullscreen:
                pass
            inp.handle_event(event)
        if not self.headless:
            mx, my = pygame.mouse.get_pos()
            inp.mouse_pos = ((mx - self._view_rect.x) // max(1, self._view_scale),
                             (my - self._view_rect.y) // max(1, self._view_scale))
        inp.update(dt)
        if inp.pressed("fullscreen"):
            self.toggle_fullscreen()
        if inp.pressed("screenshot"):
            self.screenshot()

        if self._fade is not None:
            self._update_fade(dt)
        elif self.scenes:
            self.scenes[-1].update(dt)
        self.draw()
        self.present()

    def _update_fade(self, dt: float) -> None:
        f = self._fade
        f["t"] += dt
        if f["phase"] == "out" and f["t"] >= f["dur"]:
            f["phase"] = "hold"
            f["t"] = 0.0
            fn, f["fn"] = f["fn"], None
            if fn:
                fn()
        elif f["phase"] == "hold" and f["t"] >= f["hold"]:
            f["phase"] = "in"
            f["t"] = 0.0
        elif f["phase"] == "in" and f["t"] >= f["dur"]:
            # a midpoint callback may have started another fade
            if self._fade is f:
                self._fade = None

    def draw(self) -> None:
        if not self.scenes:
            return
        start = len(self.scenes) - 1
        while start > 0 and self.scenes[start].transparent:
            start -= 1
        for scene in self.scenes[start:]:
            scene.draw(self.frame)
        f = self._fade
        if f is not None:
            if f["phase"] == "out":
                a = min(1.0, f["t"] / f["dur"])
            elif f["phase"] == "hold":
                a = 1.0
            else:
                a = 1.0 - min(1.0, f["t"] / f["dur"])
            # quantised alpha gives the fade a stepped, console-like feel
            a = round(a * 6) / 6.0
            if a > 0:
                veil = self.pygame.Surface(self.frame.get_size())
                veil.fill(f["color"])
                veil.set_alpha(int(255 * a))
                self.frame.blit(veil, (0, 0))

    def present(self) -> None:
        pygame = self.pygame
        win = self.window
        if win is None:
            return
        ww, wh = win.get_size()
        scale = max(1, min(ww // config.VIEW_W, wh // config.VIEW_H))
        w, h = config.VIEW_W * scale, config.VIEW_H * scale
        x, y = (ww - w) // 2, (wh - h) // 2
        self._view_rect = pygame.Rect(x, y, w, h)
        self._view_scale = scale
        if (w, h) != (ww, wh):
            win.fill((0, 0, 0))
        if scale == 1:
            win.blit(self.frame, (x, y))
        else:
            win.blit(pygame.transform.scale(self.frame, (w, h)), (x, y))
        pygame.display.flip()

    def screenshot(self, path: Optional[str] = None, scale: int = 3) -> str:
        pygame = self.pygame
        if path is None:
            os.makedirs(config.save_dir(), exist_ok=True)
            n = 1
            while os.path.exists(os.path.join(config.save_dir(), "screenshot_%03d.png" % n)):
                n += 1
            path = os.path.join(config.save_dir(), "screenshot_%03d.png" % n)
        img = self.frame if scale == 1 else pygame.transform.scale(
            self.frame, (config.VIEW_W * scale, config.VIEW_H * scale))
        pygame.image.save(img, path)
        return path
