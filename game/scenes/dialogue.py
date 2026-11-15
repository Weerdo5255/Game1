"""Dialogue window: portrait, typewriter text and the choice list.

All conversation logic lives in :class:`game.story.dialogue.DialogueRunner`;
this scene only presents it and feeds the player's picks back in.
"""
from __future__ import annotations

from typing import List, Optional, Tuple

import pygame

from .. import config
from ..engine import palette as P
from ..engine import ui
from ..engine.font import strip_markup
from ..engine.scene import Scene
from ..story.dialogue import DialogueRunner
from ..world.character import SKILL_LABELS

BOX = pygame.Rect(4, 150, config.VIEW_W - 8, 62)
CHOICE_W = 292
MAX_CHOICE_LINES = 10


class DialogueScene(Scene):
    transparent = True
    name = "dialogue"

    def __init__(self, app, world, dialogue_id: str, npc_id: str = "", obj_id: str = "",
                 node: Optional[str] = None):
        super().__init__(app)
        self.world = world
        self.runner = DialogueRunner(world, dialogue_id, npc_id, obj_id, node)
        self.writer: Optional[ui.Typewriter] = None
        self.speaker = {"id": "", "name": "", "appearance": None, "blip": "blip"}
        self.index = 0
        self.first = 0
        self._key = None
        self._blip = 0
        self._banner: Optional[Tuple[str, bool]] = None
        self._banner_time = 0.0
        self._lines: List[List[str]] = []

    def enter(self) -> None:
        self.world.busy = True
        self.runner.start()
        self._sync()

    def resume(self) -> None:
        self.input.reset()
        self._key = None
        self._sync()

    # ------------------------------------------------------------------
    def _play_scene(self):
        for scene in self.app.scenes:
            if scene.name == "play":
                return scene
        return None

    def _sync(self) -> None:
        """Rebuild the text writer whenever the runner moved to a new page."""
        r = self.runner
        if r.finished:
            return
        key = (r.node_id, r.page, id(r.pages))
        if key == self._key:
            return
        self._key = key
        self.speaker = r.speaker()
        has_portrait = self.speaker.get("appearance") is not None
        width = BOX.w - 16 - (40 if has_portrait else 0)
        self.writer = ui.Typewriter(r.text, width, lines_per_page=4, cps=self.app.text_cps, font=self.app.font)
        self.index = 0
        self.first = 0
        self._lines = []

    def _handle_requests(self) -> bool:
        """Windows a dialogue may open on top of itself (trade, containers)."""
        world, app = self.world, self.app
        keep = []
        opened = False
        for req in world.requests:
            kind = req.get("type")
            if kind == "sound":
                app.audio.play(str(req.get("name", "")))
            elif kind == "fade":
                play = self._play_scene()
                if play is not None:
                    play._snap_camera()
            elif kind == "trade" and not opened:
                from .panels import TradeScene
                npc = world.characters.get(str(req.get("npc", "")))
                if npc is not None:
                    app.push_scene(TradeScene(app, world, npc))
                    opened = True
            elif kind == "container" and not opened:
                from .panels import ContainerScene
                obj = world.find_object(str(req.get("obj", "")))
                if obj is not None:
                    app.push_scene(ContainerScene(app, world, obj))
                    opened = True
            else:
                keep.append(req)
        world.requests[:] = keep
        return opened

    def _choices(self):
        w = self.writer
        if w is None or not w.page_done or not w.last_page:
            return []
        return self.runner.choices()

    def update(self, dt: float) -> None:
        inp = self.input
        r = self.runner
        self.world.busy = True
        if self._handle_requests():
            return
        play = self._play_scene()
        if play is not None:
            play._collect_toasts()
            play.toasts.update(dt)
        if self._banner_time > 0:
            self._banner_time -= dt
        if r.finished:
            self.close()
            return
        self._sync()
        w = self.writer
        if w is None:
            self.close()
            return
        shown = w.update(dt)
        if shown:
            self._blip += shown
            if self._blip >= 3 and self.app.text_cps < 1000:
                self._blip = 0
                self.app.audio.play(self.speaker.get("blip") or "blip", 0.5)
        choices = self._choices()
        if choices:
            if inp.repeated("up"):
                self.index = (self.index - 1) % len(choices)
                ui.sound("move")
            elif inp.repeated("down"):
                self.index = (self.index + 1) % len(choices)
                ui.sound("move")
            if inp.pressed("confirm"):
                if choices[self.index].enabled:
                    ui.sound("confirm")
                    r.choose(self.index)
                    self._after_choice()
                else:
                    ui.sound("error")
            return
        if inp.pressed("confirm") or inp.pressed("cancel"):
            if not w.page_done:
                w.skip()
            elif w.next_page():
                ui.sound("move")
            else:
                ui.sound("move")
                r.advance()
                if r.finished:
                    self.close()
                else:
                    self._sync()

    def _after_choice(self) -> None:
        r = self.runner
        check = r.last_check
        if check:
            label = SKILL_LABELS.get(check["skill"], check["skill"]).upper()
            self._banner = ("%s CHECK: %s" % (label, "SUCCESS" if check["success"] else "FAILED"),
                            bool(check["success"]))
            self._banner_time = 1.8
            self.app.audio.play("skill_pass" if check["success"] else "skill_fail")
            r.last_check = None
        if r.finished:
            self.close()
        else:
            self._sync()

    # ------------------------------------------------------------------
    def draw(self, surf) -> None:
        font = self.app.font
        r = self.runner
        w = self.writer
        if w is None or r.finished:
            return
        sp = self.speaker
        narrator = not sp.get("name") and sp.get("appearance") is None
        inner = ui.panel(surf, BOX, title=sp.get("name") or None,
                         border=P.UI["border"] if not narrator else P.STEEL[3])
        x = inner.x + 2
        if sp.get("appearance") is not None:
            frame = pygame.Rect(BOX.x + 6, BOX.y + 13, 36, 36)
            surf.fill(P.BLACK, frame)
            surf.fill(P.NAVY[3], frame.inflate(-2, -2))
            surf.fill(P.NAVY[2], (frame.x + 1, frame.y + 20, 34, 15))
            surf.blit(self.app.chars.portrait(sp["appearance"]), (frame.x + 2, frame.y + 2))
            x = frame.right + 6
        w.draw(surf, x, BOX.y + 12, P.STEEL[5] if narrator else P.UI["text"])
        choices = self._choices()
        if choices:
            self._draw_choices(surf, choices)
        elif w.page_done and int(self.app.time * 3) % 2 == 0:
            font.draw(surf, "▼", (BOX.right - 12, BOX.bottom - 12), P.UI["select"])
        if self._banner_time > 0 and self._banner:
            text, ok = self._banner
            tw = font.width(text)
            bx = (config.VIEW_W - tw) // 2
            surf.fill(P.BLACK, (bx - 8, 40, tw + 16, 16))
            surf.fill(P.GREEN[1] if ok else P.RED[1], (bx - 7, 41, tw + 14, 14))
            font.draw(surf, text, (bx, 44), P.WHITE, shadow=P.BLACK)

    def _draw_choices(self, surf, choices) -> None:
        font = self.app.font
        text_w = CHOICE_W - 22
        blocks: List[Tuple[List[str], bool]] = []
        for ch in choices:
            label = ch.text
            if ch.tag:
                if "%" in ch.tag:
                    color = "y"
                else:
                    color = "g" if ch.tag_ok else "r"
                label = "<%s>[%s]</%s> %s" % (color, ch.tag, color, label)
            lines = font.wrap(label, text_w)[:3]
            blocks.append((lines, ch.enabled))
        # keep the selected choice on screen
        self.first = min(self.first, self.index)
        while True:
            total = sum(len(b[0]) for b in blocks[self.first:self.index + 1])
            if total <= MAX_CHOICE_LINES or self.first >= self.index:
                break
            self.first += 1
        visible = []
        total = 0
        for i in range(self.first, len(blocks)):
            n = len(blocks[i][0])
            if total + n > MAX_CHOICE_LINES and visible:
                break
            visible.append(i)
            total += n
        h = total * 11 + 9
        rect = pygame.Rect(config.VIEW_W - CHOICE_W - 4, BOX.y - h - 2, CHOICE_W, h)
        ui.panel(surf, rect, border=P.UI["select"])
        y = rect.y + 5
        for i in visible:
            lines, enabled = blocks[i]
            selected = i == self.index
            if selected:
                surf.fill(P.NAVY[3], (rect.x + 3, y - 1, rect.w - 6, len(lines) * 11))
                ui.cursor(surf, rect.x + 5, y, self.app.time, font=font)
            for line in lines:
                if not enabled:
                    font.draw(surf, strip_markup(line), (rect.x + 14, y), P.UI["disabled"])
                else:
                    font.draw(surf, line, (rect.x + 14, y), P.UI["select"] if selected else P.UI["text"])
                y += 11
        if self.first > 0:
            font.draw(surf, "▲", (rect.right - 10, rect.y + 2), P.UI["accent"])
        if visible and visible[-1] < len(blocks) - 1:
            font.draw(surf, "▼", (rect.right - 10, rect.bottom - 9), P.UI["accent"])
