"""Keyboard handling: raw keys are translated into named *actions*.

Scenes never look at key codes; they ask ``input.pressed("confirm")`` or
``input.held("left")``.  That keeps bindings in one place and lets tests and
the playthrough bot drive the game with :meth:`Input.inject`.
"""
from __future__ import annotations

from typing import Dict, List, Optional, Set, Tuple

import pygame

DEFAULT_BINDINGS: Dict[str, List[int]] = {
    "up": [pygame.K_UP, pygame.K_w],
    "down": [pygame.K_DOWN, pygame.K_s],
    "left": [pygame.K_LEFT, pygame.K_a],
    "right": [pygame.K_RIGHT, pygame.K_d],
    "confirm": [pygame.K_RETURN, pygame.K_SPACE, pygame.K_e, pygame.K_z, pygame.K_KP_ENTER],
    "cancel": [pygame.K_ESCAPE, pygame.K_x, pygame.K_BACKSPACE],
    "menu": [pygame.K_ESCAPE, pygame.K_TAB],
    "run": [pygame.K_LSHIFT, pygame.K_RSHIFT],
    "tab_prev": [pygame.K_q, pygame.K_PAGEUP, pygame.K_LEFTBRACKET],
    "tab_next": [pygame.K_r, pygame.K_PAGEDOWN, pygame.K_RIGHTBRACKET],
    "inventory": [pygame.K_i],
    "journal": [pygame.K_j],
    "crew": [pygame.K_c],
    "station": [pygame.K_m],
    "wait": [pygame.K_t],
    "quicksave": [pygame.K_F5],
    "quickload": [pygame.K_F9],
    "debug": [pygame.K_F3],
    "fullscreen": [pygame.K_F11],
    "screenshot": [pygame.K_F12],
}

# actions that auto-repeat while held (menu navigation)
REPEATING = ("up", "down", "left", "right")
REPEAT_DELAY = 0.32
REPEAT_RATE = 0.075


class Input:
    def __init__(self, bindings: Optional[Dict[str, List[int]]] = None):
        self.bindings = {k: list(v) for k, v in (bindings or DEFAULT_BINDINGS).items()}
        self._key_to_actions: Dict[int, List[str]] = {}
        for action, keys in self.bindings.items():
            for key in keys:
                self._key_to_actions.setdefault(key, []).append(action)
        self._held: Dict[str, int] = {}          # action -> number of keys down
        self._pressed: Set[str] = set()
        self._released: Set[str] = set()
        self._repeat: Dict[str, float] = {}
        self._repeat_fired: Set[str] = set()
        self._injected_held: Set[str] = set()
        self.keys_pressed: Set[int] = set()       # raw keys that went down this frame
        self.text: str = ""                       # characters typed this frame
        self.mouse_pos: Tuple[int, int] = (0, 0)  # in virtual framebuffer pixels
        self.mouse_pressed: Set[int] = set()
        self.mouse_released: Set[int] = set()
        self.mouse_held: Set[int] = set()
        self.wheel: int = 0
        self.any_pressed = False

    # -- frame lifecycle -----------------------------------------------------
    def begin_frame(self) -> None:
        self._pressed.clear()
        self._released.clear()
        self._repeat_fired.clear()
        self.keys_pressed.clear()
        self.mouse_pressed.clear()
        self.mouse_released.clear()
        self.text = ""
        self.wheel = 0
        self.any_pressed = False

    def handle_event(self, event) -> None:
        if event.type == pygame.KEYDOWN:
            self.keys_pressed.add(event.key)
            self.any_pressed = True
            for action in self._key_to_actions.get(event.key, ()):
                self._held[action] = self._held.get(action, 0) + 1
                self._pressed.add(action)
                self._repeat[action] = -REPEAT_DELAY
        elif event.type == pygame.KEYUP:
            for action in self._key_to_actions.get(event.key, ()):
                n = self._held.get(action, 0) - 1
                if n <= 0:
                    self._held.pop(action, None)
                    self._released.add(action)
                    self._repeat.pop(action, None)
                else:
                    self._held[action] = n
        elif event.type == pygame.TEXTINPUT:
            self.text += event.text
        elif event.type == pygame.MOUSEBUTTONDOWN:
            self.mouse_pressed.add(event.button)
            self.mouse_held.add(event.button)
            self.any_pressed = True
        elif event.type == pygame.MOUSEBUTTONUP:
            self.mouse_released.add(event.button)
            self.mouse_held.discard(event.button)
        elif event.type == pygame.MOUSEWHEEL:
            self.wheel += event.y

    def update(self, dt: float) -> None:
        """Advance auto-repeat timers (call once per frame after events)."""
        for action in REPEATING:
            if self.held(action) and action in self._repeat:
                self._repeat[action] += dt
                if self._repeat[action] >= REPEAT_RATE:
                    self._repeat[action] -= REPEAT_RATE
                    self._repeat_fired.add(action)

    def reset(self) -> None:
        """Forget everything that is held (used when the window loses focus
        or a scene changes, so a key held across the change doesn't stick)."""
        self._held.clear()
        self._repeat.clear()
        self._injected_held.clear()
        self.mouse_held.clear()
        self.begin_frame()

    # -- queries -------------------------------------------------------------
    def pressed(self, action: str) -> bool:
        """True on the frame the action's key went down."""
        return action in self._pressed

    def repeated(self, action: str) -> bool:
        """Like :meth:`pressed` but also fires repeatedly while held."""
        return action in self._pressed or action in self._repeat_fired

    def held(self, action: str) -> bool:
        return action in self._held or action in self._injected_held

    def released(self, action: str) -> bool:
        return action in self._released

    def consume(self, action: str) -> None:
        """Swallow a press so scenes further down the stack don't see it."""
        self._pressed.discard(action)
        self._repeat_fired.discard(action)

    def direction(self) -> Optional[str]:
        """The single direction currently held, preferring the newest."""
        for action in ("up", "down", "left", "right"):
            if action in self._pressed:
                self._last_dir = action
        last = getattr(self, "_last_dir", None)
        if last and self.held(last):
            return last
        for action in ("up", "down", "left", "right"):
            if self.held(action):
                self._last_dir = action
                return action
        return None

    # -- synthetic input (tests, demo bot) ------------------------------------
    def inject(self, action: str) -> None:
        """Simulate a tap of ``action`` for the current frame."""
        self._pressed.add(action)
        self.any_pressed = True

    def hold(self, action: str, down: bool = True) -> None:
        if down:
            self._injected_held.add(action)
            self._pressed.add(action)
        else:
            self._injected_held.discard(action)

    def type_text(self, text: str) -> None:
        self.text += text
