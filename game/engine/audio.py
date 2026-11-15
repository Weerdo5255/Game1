"""Synthesised sound effects.

Like the graphics, the audio is generated in code: every effect is a short
list of oscillator segments rendered into a sample buffer the first time it
is played.  If no audio device is available (headless tests, servers) the
whole class silently does nothing.
"""
from __future__ import annotations

import math
import random
from array import array
from typing import Dict, List, Optional, Tuple

import pygame

# (waveform, start Hz, end Hz, seconds, volume)
Segment = Tuple[str, float, float, float, float]

SOUNDS: Dict[str, List[Segment]] = {
    "move": [("square", 620, 620, 0.025, 0.25)],
    "confirm": [("square", 660, 660, 0.035, 0.3), ("square", 990, 990, 0.05, 0.3)],
    "cancel": [("square", 440, 300, 0.07, 0.3)],
    "blip": [("square", 480, 480, 0.012, 0.16)],
    "blip_low": [("square", 300, 300, 0.012, 0.18)],
    "blip_high": [("square", 720, 720, 0.012, 0.14)],
    "blip_robot": [("tri", 900, 600, 0.014, 0.2)],
    "door": [("noise", 0, 0, 0.05, 0.18), ("tri", 180, 320, 0.12, 0.25)],
    "pickup": [("square", 660, 660, 0.04, 0.25), ("square", 880, 880, 0.04, 0.25),
               ("square", 1320, 1320, 0.07, 0.25)],
    "error": [("square", 170, 150, 0.14, 0.3)],
    "alert": [("square", 880, 880, 0.09, 0.28), ("square", 660, 660, 0.09, 0.28),
              ("square", 880, 880, 0.09, 0.28), ("square", 660, 660, 0.12, 0.28)],
    "save": [("tri", 520, 520, 0.05, 0.3), ("tri", 660, 660, 0.05, 0.3),
             ("tri", 780, 780, 0.05, 0.3), ("tri", 1040, 1040, 0.12, 0.3)],
    "quest": [("square", 520, 520, 0.08, 0.26), ("square", 660, 660, 0.08, 0.26),
              ("square", 780, 780, 0.16, 0.26)],
    "skill_pass": [("tri", 700, 1100, 0.12, 0.3)],
    "skill_fail": [("tri", 400, 200, 0.16, 0.3)],
    "repair": [("noise", 0, 0, 0.04, 0.2), ("square", 240, 240, 0.04, 0.2),
               ("noise", 0, 0, 0.04, 0.2), ("square", 320, 320, 0.06, 0.2)],
    "power_down": [("tri", 500, 60, 0.6, 0.35)],
    "sleep": [("sine", 440, 220, 0.5, 0.25)],
    "hum": [("hum", 55, 55, 2.0, 0.22)],
}


class Audio:
    def __init__(self, enabled: bool = True, volume: float = 0.6):
        self.enabled = False
        self.volume = volume
        self._cache: Dict[str, object] = {}
        self._ambient = None
        self._rate = 22050
        self._channels = 1
        if not enabled:
            return
        try:
            if not pygame.mixer.get_init():
                pygame.mixer.init(22050, -16, 1, 512)
            info = pygame.mixer.get_init()
            if info and info[1] == -16:
                self._rate, _, self._channels = info
                self.enabled = True
        except pygame.error:
            self.enabled = False

    def _render(self, segments: List[Segment]) -> bytes:
        rate = self._rate
        rng = random.Random(7)
        out = array("h")
        for wave, f0, f1, dur, vol in segments:
            n = max(1, int(rate * dur))
            phase = 0.0
            attack = max(1, int(0.004 * rate))
            release = max(1, int(0.012 * rate))
            for i in range(n):
                t = i / n
                phase += (f0 + (f1 - f0) * t) / rate
                frac = phase % 1.0
                if wave == "square":
                    v = 1.0 if frac < 0.5 else -1.0
                elif wave == "tri":
                    v = 4.0 * abs(frac - 0.5) - 1.0
                elif wave == "sine":
                    v = math.sin(2 * math.pi * phase)
                elif wave == "noise":
                    v = rng.uniform(-1.0, 1.0)
                elif wave == "hum":
                    # seamless loop: whole number of cycles of each partial
                    v = (0.6 * math.sin(2 * math.pi * phase)
                         + 0.3 * math.sin(4 * math.pi * phase)
                         + 0.1 * math.sin(2 * math.pi * 3 * t)) * (0.85 + 0.15 * math.sin(2 * math.pi * t))
                else:
                    v = 0.0
                env = 1.0 if wave == "hum" else min(1.0, i / attack, (n - i) / release)
                sample = int(max(-1.0, min(1.0, v * env * vol)) * 32000)
                for _ in range(self._channels):
                    out.append(sample)
        return out.tobytes()

    def _sound(self, name: str):
        snd = self._cache.get(name)
        if snd is None and name in SOUNDS:
            snd = pygame.mixer.Sound(buffer=self._render(SOUNDS[name]))
            self._cache[name] = snd
        return snd

    def play(self, name: str, volume: float = 1.0) -> None:
        if not self.enabled or self.volume <= 0:
            return
        try:
            snd = self._sound(name)
            if snd is not None:
                snd.set_volume(self.volume * volume)
                snd.play()
        except pygame.error:
            self.enabled = False

    def ambient(self, name: Optional[str], volume: float = 0.5) -> None:
        """Loop a background drone (``None`` stops it)."""
        if not self.enabled:
            return
        try:
            if self._ambient is not None:
                self._ambient.stop()
                self._ambient = None
            if name:
                snd = self._sound(name)
                if snd is not None:
                    snd.set_volume(self.volume * volume)
                    snd.play(loops=-1)
                    self._ambient = snd
                    self._ambient_volume = volume
        except pygame.error:
            self.enabled = False

    def set_volume(self, volume: float) -> None:
        self.volume = max(0.0, min(1.0, volume))
        if self._ambient is not None:
            self._ambient.set_volume(self.volume * getattr(self, "_ambient_volume", 0.5))
