"""Title screen and new-game setup (name, background, appearance)."""
from __future__ import annotations

import random
from typing import List

import pygame

from .. import config, save
from ..art import chars as chargen
from ..engine import palette as P
from ..engine import ui
from ..engine.font import get_font
from ..engine.scene import Scene
from ..world.character import ATTR_LABELS, SKILL_LABELS
from .render import MapRenderer

W, H = config.VIEW_W, config.VIEW_H


class _Sky:
    """Minimal stand-in for a map so the renderer can draw the backdrop."""
    props = {"backdrop": "saturn"}


class TitleScene(Scene):
    name = "title"

    def __init__(self, app):
        super().__init__(app)
        self.renderer = MapRenderer(app.tiles, app.chars, app.icons)
        self.menu = ui.Menu(visible=6)
        self.message = ""

    def enter(self) -> None:
        self._build()

    def resume(self) -> None:
        self.input.reset()
        self._build()

    def _build(self) -> None:
        items = []
        if save.latest_slot() is not None:
            items.append(ui.MenuItem("Continue", data="continue"))
        items.append(ui.MenuItem("New game", data="new"))
        items.append(ui.MenuItem("Load game", data="load", enabled=save.latest_slot() is not None))
        items.append(ui.MenuItem("Quit", data="quit"))
        self.menu.set_items(items, keep_index=False)

    def _load(self, slot: str) -> None:
        from .play import PlayScene
        app = self.app
        try:
            world = save.load_game(app.content, slot)
        except (OSError, ValueError, KeyError) as exc:
            self.message = "<r>Could not load: %s</r>" % exc
            return
        app.fade(lambda: app.set_scene(PlayScene(app, world)), 0.35)

    def update(self, dt: float) -> None:
        result = self.menu.update(self.input)
        if result != "select":
            return
        action = self.menu.selected.data
        app = self.app
        if action == "continue":
            slot = save.latest_slot()
            if slot:
                self._load(slot)
        elif action == "new":
            app.push_scene(NewGameScene(app))
        elif action == "load":
            from .panels import SaveLoadScene
            app.push_scene(SaveLoadScene(app, "load", on_load=self._load))
        elif action == "quit":
            app.quit()

    def draw(self, surf) -> None:
        app = self.app
        t = app.time
        self.renderer.draw_backdrop(surf, _Sky, int(t * 6), int(t * 2) % 40, t)
        title = str(app.content.title if app.content else "TETHYS DEEP").upper()
        huge = get_font(3)
        tw = huge.width(title)
        x = (W - tw) // 2
        huge.draw(surf, title, (x + 2, 44), P.NAVY[0])
        huge.draw(surf, title, (x, 42), P.CYAN[3], shadow=P.CYAN[0])
        surf.fill(P.CYAN[1], (x, 76, tw, 1))
        subtitle = str((app.content.game.get("subtitle") if app.content else "") or "")
        app.font.draw(surf, subtitle, (W // 2, 81), P.STEEL[4], shadow=P.BLACK, align="center")
        rect = pygame.Rect(W // 2 - 56, 118, 112, len(self.menu.items) * 11 + 12)
        inner = ui.panel(surf, rect)
        self.menu.draw(surf, inner.x, inner.y + 1, inner.w, t)
        if self.message:
            app.font.draw(surf, self.message, (W // 2, 186), P.WHITE, align="center")
        if app.content is not None and app.content.problems:
            app.font.draw(surf, "<r>%d content file(s) failed to load - run the validator</r>" %
                          len(app.content.problems), (W // 2, 196), P.WHITE, align="center")
        app.font.draw(surf, "Arrows move   E select   F11 fullscreen", (W // 2, H - 11), P.STEEL[3], align="center")


APPEARANCE_ROWS = [
    ("skin", "Skin", lambda: list(P.SKIN_TONES)),
    ("hair", "Hair", lambda: [h for h in chargen.HAIR_STYLES if h != "helmet"]),
    ("hair_color", "Hair colour", lambda: list(P.HAIR_COLORS)),
    ("outfit", "Outfit", lambda: list(chargen.OUTFIT_STYLES)),
    ("primary", "Main colour", lambda: list(P.CLOTH_COLORS)),
    ("secondary", "Trim colour", lambda: list(P.CLOTH_COLORS)),
    ("legs", "Trousers", lambda: list(P.CLOTH_COLORS)),
]


def random_appearance(rng=random) -> dict:
    return {
        "body": "human",
        "skin": rng.choice(list(P.SKIN_TONES)[:7]),
        "hair": rng.choice([h for h in chargen.HAIR_STYLES if h not in ("helmet", "cap")]),
        "hair_color": rng.choice(list(P.HAIR_COLORS)),
        "outfit": rng.choice(["uniform", "jumpsuit", "uniform", "vest"]),
        "primary": rng.choice(list(P.CLOTH_COLORS)),
        "secondary": rng.choice(list(P.CLOTH_COLORS)),
        "legs": rng.choice(["slate", "black", "navy", "brown", "steel"]),
        "extras": [e for e in chargen.EXTRAS if rng.random() < 0.15],
    }


class NewGameScene(Scene):
    name = "newgame"

    def __init__(self, app):
        super().__init__(app)
        game = app.content.game
        pdef = game.get("player") or {}
        self.step = 0
        self.entry = ui.TextEntry(str(pdef.get("name", "Rin Okabe")), max_len=18)
        self.backgrounds: List[dict] = app.content.backgrounds()
        self.bg_menu = ui.Menu([ui.MenuItem(str(b.get("name", b.get("id")))) for b in self.backgrounds], visible=6)
        self.appearance = chargen.normalize(pdef.get("appearance"))
        self.row = 0

    def _start(self) -> None:
        from ..world.world import World
        from .play import PlayScene
        app = self.app
        bg = self.backgrounds[self.bg_menu.index] if self.backgrounds else {}
        setup = {"name": self.entry.value.strip() or "Auditor", "background": bg.get("id"),
                 "appearance": dict(self.appearance)}

        def go() -> None:
            world = World.new_game(app.content, setup)
            intro = app.content.game.get("intro")
            if intro and intro in app.content.dialogues:
                world.request("dialogue", dialogue=intro)
            app.set_scene(PlayScene(app, world))
        app.fade(go, 0.5)

    def update(self, dt: float) -> None:
        inp = self.input
        if self.step == 0:
            self.entry.update(inp)
            if pygame.K_RETURN in inp.keys_pressed or pygame.K_KP_ENTER in inp.keys_pressed:
                if self.entry.value.strip():
                    ui.sound("confirm")
                    self.step = 1 if self.backgrounds else 2
            elif pygame.K_ESCAPE in inp.keys_pressed:
                ui.sound("cancel")
                self.close()
            return
        if self.step == 1:
            result = self.bg_menu.update(inp)
            if result == "select":
                self.step = 2
            elif result == "cancel":
                self.step = 0
            return
        rows = APPEARANCE_ROWS
        if inp.repeated("up"):
            self.row = (self.row - 1) % (len(rows) + 2)
            ui.sound("move")
        elif inp.repeated("down"):
            self.row = (self.row + 1) % (len(rows) + 2)
            ui.sound("move")
        if self.row < len(rows):
            key, _label, values_fn = rows[self.row]
            values = values_fn()
            delta = 1 if (inp.repeated("right") or inp.pressed("confirm")) else -1 if inp.repeated("left") else 0
            if delta:
                cur = self.appearance.get(key)
                i = values.index(cur) if cur in values else 0
                self.appearance[key] = values[(i + delta) % len(values)]
                ui.sound("move")
        elif self.row == len(rows):
            if inp.pressed("confirm") or inp.pressed("left") or inp.pressed("right"):
                extras = list(chargen.EXTRAS)
                combos = [[], [extras[0]], [extras[1]], extras]
                cur = sorted(self.appearance.get("extras") or [])
                i = combos.index(cur) if cur in combos else 0
                self.appearance["extras"] = combos[(i + 1) % len(combos)]
                ui.sound("move")
        elif inp.pressed("confirm"):
            ui.sound("confirm")
            self._start()
            return
        if inp.pressed("tab_next"):
            self.appearance = random_appearance()
            ui.sound("pickup")
        if inp.pressed("cancel"):
            ui.sound("cancel")
            self.step = 1 if self.backgrounds else 0

    def draw(self, surf) -> None:
        app = self.app
        font = app.font
        t = app.time
        surf.fill(P.NAVY[0])
        ui.panel(surf, (8, 8, W - 16, H - 28), title="New game")
        font.draw(surf, "Name", (20, 24), P.UI["accent"])
        self.entry.draw(surf, 60, 22, 130, t, focused=self.step == 0)
        if self.step == 0:
            ui.key_hints(surf, [("Type", "your name"), ("Enter", "Next"), ("Esc", "Back")])
            ui.draw_text_block(surf, str(app.content.game.get("premise", "")), (20, 46, W - 40, 120), P.STEEL[4])
            return
        font.draw(surf, "Background", (20, 44), P.UI["accent"])
        if self.step == 1:
            self.bg_menu.draw(surf, 20, 56, 130, t)
            bg = self.backgrounds[self.bg_menu.index]
            x = 164
            font.draw(surf, str(bg.get("name", "")), (x, 56), P.UI["select"])
            y = ui.draw_text_block(surf, str(bg.get("desc", "")), (x, 68, W - x - 22, 60), P.UI["text"]) + 4
            bonuses = ["%s +%d" % (ATTR_LABELS.get(k, k), v) for k, v in (bg.get("attrs") or {}).items()]
            bonuses += ["%s +%d" % (SKILL_LABELS.get(k, k), v) for k, v in (bg.get("skills") or {}).items()]
            y = ui.draw_text_block(surf, "<g>" + ", ".join(bonuses) + "</g>", (x, y, W - x - 22, 40)) + 2
            gear = [app.content.items[k].name for k in (bg.get("items") or {}) if k in app.content.items]
            if gear:
                ui.draw_text_block(surf, "<d>Starts with: " + ", ".join(gear) + "</d>", (x, y, W - x - 22, 40))
            ui.key_hints(surf, [("E", "Choose"), ("X", "Back")])
            return
        bg = self.backgrounds[self.bg_menu.index] if self.backgrounds else {}
        font.draw(surf, str(bg.get("name", "")), (90, 44), P.UI["text"])
        font.draw(surf, "Appearance", (20, 60), P.UI["accent"])
        y = 72
        rows = APPEARANCE_ROWS
        for i, (key, label, _values) in enumerate(rows):
            selected = i == self.row
            if selected:
                surf.fill(P.NAVY[3], (18, y - 1, 190, 11))
            font.draw(surf, label, (22, y), P.UI["select"] if selected else P.UI["text"])
            font.draw(surf, "◀ %s ▶" % str(self.appearance.get(key)).replace("_", " "), (204, y),
                      P.UI["select"] if selected else P.UI["dim"], align="right")
            y += 11
        selected = self.row == len(rows)
        if selected:
            surf.fill(P.NAVY[3], (18, y - 1, 190, 11))
        extras = ", ".join(self.appearance.get("extras") or []) or "none"
        font.draw(surf, "Extras", (22, y), P.UI["select"] if selected else P.UI["text"])
        font.draw(surf, "◀ %s ▶" % extras, (204, y), P.UI["select"] if selected else P.UI["dim"], align="right")
        y += 15
        selected = self.row == len(rows) + 1
        if selected:
            surf.fill(P.NAVY[3], (18, y - 1, 190, 11))
            ui.cursor(surf, 20, y, t)
        font.draw(surf, "Begin", (30, y), P.UI["select"] if selected else P.GREEN[3])
        # preview
        px, py = 250, 62
        ui.panel(surf, (px - 10, py - 8, 118, 112))
        direction = ("down", "left", "up", "right")[int(t * 0.5) % 4]
        sprite = app.chars.sprite(self.appearance, direction, int(t * 6) % 4)
        surf.blit(pygame.transform.scale(sprite, (48, 72)), (px, py + 4))
        portrait = pygame.transform.scale(app.chars.portrait(self.appearance), (48, 48))
        surf.fill(P.NAVY[3], (px + 52, py + 4, 50, 50))
        surf.blit(portrait, (px + 53, py + 5))
        font.draw(surf, self.entry.value, (px + 49, py + 84), P.UI["select"], align="center")
        ui.key_hints(surf, [("←→", "Change"), ("R", "Randomise"), ("E", "Select"), ("X", "Back")])
