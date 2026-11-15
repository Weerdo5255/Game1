"""Ending slides.

``data/story/endings.json`` maps ending ids to a title and a list of slides.
Every slide may carry an ``if`` condition, so one ending reads differently
depending on what happened::

    "viable": {
      "title": "Ithaca Holds",
      "slides": [
        {"text": "The tug docks at 08:00 ..."},
        {"if": "flag('dex_arrested')", "text": "Dex Marlow leaves in restraints."}
      ]
    },
    "epilogue": {"slides": [ ... appended to every ending ... ]}
"""
from __future__ import annotations

from typing import List

from .. import config, save
from ..engine import palette as P
from ..engine import ui
from ..engine.scene import Scene
from .render import MapRenderer

W, H = config.VIEW_W, config.VIEW_H


def ending_slides(world, ending_id: str) -> List[str]:
    """Resolve the text of every slide that applies (also used by tests)."""
    endings = world.content.endings
    ending = endings.get(ending_id) or {}
    slides: List[str] = []
    groups = [ending.get("slides") or []]
    if not ending.get("no_epilogue"):
        groups.append((endings.get("epilogue") or {}).get("slides") or [])
    for group in groups:
        for slide in group:
            if world.script.eval(slide.get("if")):
                slides.append(world.text(str(slide.get("text", ""))))
    return slides


class _Sky:
    props = {"backdrop": "saturn"}


class EndingScene(Scene):
    name = "ending"

    def __init__(self, app, world, ending_id: str):
        super().__init__(app)
        self.world = world
        self.ending_id = ending_id
        ending = world.content.endings.get(ending_id) or {}
        self.title = str(ending.get("title", "The End"))
        self.slides = ending_slides(world, ending_id) or ["The end."]
        self.index = 0
        self.writer = None
        self.done = False
        self.renderer = MapRenderer(app.tiles, app.chars, app.icons)
        self._make_writer()

    def enter(self) -> None:
        self.app.audio.ambient(None)

    def _make_writer(self) -> None:
        self.writer = ui.Typewriter(self.slides[self.index], W - 80, lines_per_page=9,
                                    cps=min(self.app.text_cps, 60.0), font=self.app.font)

    def update(self, dt: float) -> None:
        inp = self.input
        if self.done:
            if inp.pressed("confirm") or inp.pressed("cancel"):
                from .title import TitleScene
                self.app.fade(lambda: self.app.set_scene(TitleScene(self.app)), 0.6)
            return
        self.writer.update(dt)
        if inp.pressed("confirm") or inp.pressed("cancel"):
            if not self.writer.page_done:
                self.writer.skip()
            elif self.writer.next_page():
                ui.sound("move")
            elif self.index < len(self.slides) - 1:
                self.index += 1
                self._make_writer()
                ui.sound("move")
            else:
                self.done = True
                ui.sound("quest")

    def draw(self, surf) -> None:
        app = self.app
        t = app.time
        self.renderer.draw_backdrop(surf, _Sky, int(t * 3), 0, t)
        ui.shade(surf, 120)
        app.font_big.draw(surf, self.title.upper(), (W // 2, 18), P.CYAN[3], shadow=P.BLACK, align="center")
        font = app.font
        if not self.done:
            self.writer.draw(surf, 40, 58, P.WHITE, shadow=P.BLACK, line_gap=2)
            font.draw(surf, "%d / %d" % (self.index + 1, len(self.slides)), (W // 2, H - 22), P.STEEL[3],
                      align="center")
            if self.writer.page_done and int(t * 3) % 2 == 0:
                font.draw(surf, "▼", (W - 46, H - 40), P.UI["select"])
            return
        w = self.world
        q = w.quests
        lines = [
            "%s  -  Day %d, %s" % (w.player.name, w.clock.day, w.clock.time_str()),
            "Quests completed: %d     failed: %d" % (len(q.by_status("done")), len(q.by_status("failed"))),
            "Time played: %s" % save.format_playtime(w.playtime),
            "",
            "Thank you for playing.",
        ]
        y = 70
        for line in lines:
            font.draw(surf, line, (W // 2, y), P.WHITE, shadow=P.BLACK, align="center")
            y += 13
        font.draw(surf, "Press E to return to the title", (W // 2, H - 24), P.UI["select"], align="center")
