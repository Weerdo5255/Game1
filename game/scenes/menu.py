"""The datapad: status, inventory, journal, crew locator, station overview
and system options, as tabs of one full-screen menu."""
from __future__ import annotations

from typing import List, Optional

import pygame

from .. import config
from ..engine import palette as P
from ..engine import ui
from ..engine.app import TEXT_SPEED_NAMES
from ..engine.scene import Scene
from ..sim import ai
from ..world.character import (ATTR_HELP, ATTR_LABELS, ATTRIBUTES, SKILL_HELP, SKILL_LABELS, SKILLS,
                               relation_label)
from .panels import (STATUS_COLORS, ChoiceScene, MessageScene, SaveLoadScene, item_lines, mods_text,
                     system_detail_lines)

W, H = config.VIEW_W, config.VIEW_H
TABS = [("status", "STATUS"), ("items", "ITEMS"), ("journal", "JOURNAL"), ("crew", "CREW"),
        ("station", "STATION"), ("system", "SYSTEM")]
BODY = pygame.Rect(4, 17, W - 8, 185)


def morale_label(value: float) -> str:
    if value >= 80:
        return "<g>In good spirits</g>"
    if value >= 55:
        return "Steady"
    if value >= 35:
        return "<y>Uneasy</y>"
    return "<r>Miserable</r>"


class Tab:
    def __init__(self, scene):
        self.scene = scene
        self.app = scene.app
        self.world = scene.world

    def open(self) -> None:
        pass

    def update(self, inp) -> None:
        pass

    def draw(self, surf, rect) -> None:
        pass

    def hints(self):
        return []


# ---------------------------------------------------------------------------
class StatusTab(Tab):
    def __init__(self, scene):
        super().__init__(scene)
        self.menu = ui.Menu(visible=15, wrap=True)

    def open(self) -> None:
        p = self.world.player
        items = []
        for a in ATTRIBUTES:
            base, eff = p.attrs[a], p.attr(a)
            items.append(ui.MenuItem(ATTR_LABELS[a], self._value(base, eff), data=("attr", a), color=P.CYAN[3]))
        for s in SKILLS:
            base, eff = p.skills.get(s, 0), p.skill(s)
            items.append(ui.MenuItem(SKILL_LABELS[s], self._value(base, eff), data=("skill", s)))
        self.menu.set_items(items)

    @staticmethod
    def _value(base: int, eff: int) -> str:
        if eff > base:
            return "%d <g>(%d)</g>" % (base, eff)
        if eff < base:
            return "%d <r>(%d)</r>" % (base, eff)
        return "%d" % base

    def update(self, inp) -> None:
        if inp.repeated("up"):
            self.menu.move(-1)
        elif inp.repeated("down"):
            self.menu.move(1)
        if inp.pressed("confirm"):
            kind, name = self.menu.selected.data
            p = self.world.player
            if kind == "skill" and p.raise_skill(name):
                ui.sound("skill_pass")
                self.open()
            else:
                ui.sound("error")

    def draw(self, surf, rect) -> None:
        font = self.app.font
        p = self.world.player
        content = self.world.content
        x, y = rect.x + 6, rect.y + 6
        frame = pygame.Rect(x, y, 36, 36)
        surf.fill(P.BLACK, frame)
        surf.fill(P.NAVY[3], frame.inflate(-2, -2))
        surf.blit(self.app.chars.portrait(p.appearance), (x + 2, y + 2))
        font.draw(surf, p.name, (x + 42, y), P.UI["select"])
        bg = next((b for b in content.backgrounds() if b.get("id") == p.background), None)
        font.draw(surf, bg.get("name", "") if bg else "", (x + 42, y + 10), P.UI["dim"])
        font.draw(surf, "Level %d" % p.level, (x + 42, y + 20), P.UI["text"])
        ui.bar(surf, (x + 84, y + 21, 60, 7), p.xp / max(1, p.xp_to_next()), P.PURPLE[2])
        y += 42
        rows = [("Health", p.hp / max(1, p.max_hp), ui.level_color(p.hp / max(1, p.max_hp)),
                 "%d/%d" % (round(p.hp), p.max_hp)),
                ("Food", 1 - p.needs["hunger"] / 100, P.AMBER[2], ""),
                ("Rest", 1 - p.needs["fatigue"] / 100, P.CYAN[2], "")]
        for label, frac, color, text in rows:
            font.draw(surf, label, (x, y), P.UI["dim"])
            ui.bar(surf, (x + 40, y + 1, 70, 7), frac, color)
            if text:
                font.draw(surf, text, (x + 114, y), P.UI["text"])
            y += 11
        font.draw(surf, "Credits  <y>%d</y>" % p.credits, (x, y + 2), P.UI["text"])
        y += 13
        over = p.encumbered
        font.draw(surf, "Load  %s%.1f / %d kg%s" % ("<r>" if over else "", p.carried_weight(), p.carry_capacity,
                                                    "</r>" if over else ""), (x, y), P.UI["text"])
        y += 14
        font.draw(surf, "CONDITION", (x, y), P.UI["accent"])
        y += 11
        effects = p.active_effects()
        if not effects:
            font.draw(surf, "Nothing unusual.", (x, y), P.UI["dim"])
        for eff_id in effects[:6]:
            edef = content.effects.get(eff_id, {})
            end = font.draw(surf, "• " + str(edef.get("name", eff_id)), (x, y),
                            P.RED[3] if edef.get("bad") else P.GREEN[3])
            m = mods_text(edef.get("mods") or {})
            if m and end.right + 6 + font.width(m) < rect.x + 172:
                font.draw(surf, m, (end.right + 6, y), P.UI["dim"])
            y += 10

        col = rect.x + 176
        font.draw(surf, "ATTRIBUTES & SKILLS", (col, rect.y + 6), P.UI["accent"])
        if p.skill_points > 0:
            font.draw(surf, "<y>%d skill point%s to spend</y>" % (p.skill_points, "s" if p.skill_points != 1 else ""),
                      (rect.right - 8, rect.y + 6), P.UI["text"], align="right")
        self.menu.draw(surf, col, rect.y + 18, rect.right - col - 6, self.app.time)
        sel = self.menu.selected
        if sel is not None:
            kind, name = sel.data
            text = ATTR_HELP[name] if kind == "attr" else "%s  <d>(uses %s)</d>" % (SKILL_HELP[name],
                                                                                    ATTR_LABELS[SKILLS[name]])
            font.draw(surf, text, (rect.x + 6, rect.bottom - 13), P.UI["text"])

    def hints(self):
        return [("E", "Raise skill")] if self.world.player.skill_points > 0 else []


# ---------------------------------------------------------------------------
class ItemsTab(Tab):
    def __init__(self, scene):
        super().__init__(scene)
        self.menu = ui.Menu(visible=15, wrap=True)

    def open(self) -> None:
        w = self.world
        p = w.player
        items = []
        for item_id, qty in p.inventory.entries():
            name = w.item_name(item_id)
            if p.is_equipped(item_id):
                name += " <c>(E)</c>"
            items.append(ui.MenuItem(name, "x%d" % qty, data=item_id))
        self.menu.set_items(items)

    def update(self, inp) -> None:
        if not self.menu.items:
            return
        result = self.menu.update(inp)
        if result == "select":
            self._actions(self.menu.selected.data)

    def _actions(self, item_id: str) -> None:
        w = self.world
        p = w.player
        idef = w.content.items.get(item_id)
        if idef is None:
            return
        options = []
        if idef.usable:
            def use() -> None:
                if w.use_item(p, item_id):
                    ui.sound("pickup")
                w.events.pump()
                self.open()
            options.append(("Use", use))
        if idef.equippable:
            if p.is_equipped(item_id):
                options.append(("Unequip", lambda: (p.unequip(idef.slot), self.open())))
            else:
                options.append(("Equip", lambda: (p.equip(item_id), ui.sound("confirm"), self.open())))
        if not idef.quest:
            def drop() -> None:
                if p.inventory.remove(item_id, 1):
                    p.check_equipment()
                    w.add_object(p.map_id, "item", p.x, p.y, item=item_id, qty=1)
                self.open()
            options.append(("Drop one", drop))
        options.append(("Cancel", None))
        self.app.push_scene(ChoiceScene(self.app, idef.name, "", options, width=130))

    def draw(self, surf, rect) -> None:
        font = self.app.font
        w = self.world
        p = w.player
        lw = 168
        if not self.menu.items:
            font.draw(surf, "Your pack is empty.", (rect.x + 10, rect.y + 10), P.UI["dim"])
        self.menu.draw(surf, rect.x + 4, rect.y + 6, lw, self.app.time)
        over = p.encumbered
        font.draw(surf, "Load %s%.1f / %d kg%s    Credits <y>%d</y>" % (
            "<r>" if over else "", p.carried_weight(), p.carry_capacity, "</r>" if over else "", p.credits),
            (rect.x + 6, rect.bottom - 13), P.UI["text"])
        sel = self.menu.selected
        if sel is None or sel.data not in w.content.items:
            return
        idef = w.content.items[sel.data]
        x = rect.x + lw + 14
        icon = pygame.transform.scale(self.app.icons.get(idef.icon, idef.color), (32, 32))
        surf.fill(P.BLACK, (x, rect.y + 6, 36, 36))
        surf.fill(P.NAVY[2], (x + 1, rect.y + 7, 34, 34))
        surf.blit(icon, (x + 2, rect.y + 8))
        font.draw(surf, idef.name, (x + 42, rect.y + 8), P.UI["select"])
        y = rect.y + 20
        for line in item_lines(idef):
            font.draw(surf, line, (x + 42, y), P.UI["text"])
            y += 10
        y = rect.y + 50
        y = ui.draw_text_block(surf, idef.desc, (x, y, rect.right - x - 8, 80), P.STEEL[5])
        if idef.use_text and idef.usable:
            font.draw(surf, "<g>Can be used.</g>", (x, y + 4), P.UI["text"])

    def hints(self):
        return [("E", "Use / equip")]


# ---------------------------------------------------------------------------
class JournalTab(Tab):
    def __init__(self, scene):
        super().__init__(scene)
        self.menu = ui.Menu(visible=15, wrap=True)

    def open(self) -> None:
        q = self.world.quests
        items = []
        for qid in q.by_status("active"):
            main = q.defs.get(qid, {}).get("main")
            items.append(ui.MenuItem(("★ " if main else "• ") + q.name(qid), data=("quest", qid)))
        for qid in q.by_status("done"):
            items.append(ui.MenuItem("✓ " + q.name(qid), data=("quest", qid), color=P.GREEN[3]))
        for qid in q.by_status("failed"):
            items.append(ui.MenuItem("✗ " + q.name(qid), data=("quest", qid), color=P.RED[3]))
        items.append(ui.MenuItem("Station log", data=("log", ""), color=P.CYAN[3]))
        if q.notes:
            items.append(ui.MenuItem("Notes", data=("notes", ""), color=P.CYAN[3]))
        self.menu.set_items(items)

    def update(self, inp) -> None:
        if inp.repeated("up"):
            self.menu.move(-1)
        elif inp.repeated("down"):
            self.menu.move(1)

    def draw(self, surf, rect) -> None:
        font = self.app.font
        w = self.world
        q = w.quests
        lw = 136
        self.menu.draw(surf, rect.x + 4, rect.y + 6, lw, self.app.time)
        sel = self.menu.selected
        if sel is None:
            return
        kind, qid = sel.data
        x = rect.x + lw + 12
        width = rect.right - x - 8
        y = rect.y + 6
        if kind == "quest":
            qdef = q.defs.get(qid, {})
            status = q.status(qid)
            font.draw(surf, q.name(qid), (x, y), P.UI["select"])
            font.draw(surf, {"active": "In progress", "done": "<g>Completed</g>", "failed": "<r>Failed</r>"}.get(status, ""),
                      (rect.right - 8, y), P.UI["dim"], align="right")
            y = ui.draw_text_block(surf, str(qdef.get("desc", "")), (x, y + 13, width, 60), P.STEEL[5]) + 5
            history = q.state.get(qid, {}).get("history", [])
            current = q.stage(qid)
            for st in history:
                text = q.stage_text(qid, st)
                if not text:
                    continue
                now = st == current and status == "active"
                mark = "<y>▶</y> " if now else "<g>✓</g> "
                for i, line in enumerate(font.wrap(text, width - 12)):
                    if y > rect.bottom - 14:
                        break
                    font.draw(surf, (mark if i == 0 else "") + line, (x if i == 0 else x + 9, y),
                              P.UI["text"] if now else P.UI["dim"])
                    y += 10
                y += 2
            outcome = (qdef.get("outcomes") or {}).get(status)
            if outcome and y < rect.bottom - 22:
                ui.draw_text_block(surf, str(outcome), (x, y + 2, width, 40), P.CYAN[3], max_lines=3)
        elif kind == "log":
            font.draw(surf, "Station log", (x, y), P.UI["select"])
            y += 13
            colors = {"announce": P.AMBER[3], "crew": P.UI["text"], "quest": P.CYAN[3], "player": P.GREEN[3],
                      "check": P.PURPLE[3]}
            lines = []
            for entry in w.log[-40:]:
                stamp = w.clock.format_stamp(entry["t"])
                wrapped = font.wrap(entry["text"], width - 52)
                for i, line in enumerate(wrapped):
                    lines.append((stamp.replace("Day ", "D") if i == 0 else "", line,
                                  colors.get(entry.get("kind"), P.UI["dim"])))
            for stamp, line, color in lines[-15:]:
                font.draw(surf, stamp, (x, y), P.STEEL[3])
                font.draw(surf, line, (x + 50, y), color)
                y += 10
            if not lines:
                font.draw(surf, "Nothing logged yet.", (x, y), P.UI["dim"])
        else:
            font.draw(surf, "Notes", (x, y), P.UI["select"])
            y += 13
            for note in q.notes[-8:]:
                y = ui.draw_text_block(surf, "• " + note["text"], (x, y, width, 40), P.UI["text"], max_lines=3) + 3


# ---------------------------------------------------------------------------
class CrewTab(Tab):
    def __init__(self, scene):
        super().__init__(scene)
        self.menu = ui.Menu(visible=15, wrap=True)

    def open(self) -> None:
        items = []
        for ch in self.world.characters.values():
            if ch.is_player or not ch.met:
                continue
            if ch.present:
                items.append(ui.MenuItem(ch.name, data=ch.id))
            else:
                items.append(ui.MenuItem(ch.name, "gone", data=ch.id, color=P.UI["disabled"]))
        self.menu.set_items(items)

    def update(self, inp) -> None:
        if inp.repeated("up"):
            self.menu.move(-1)
        elif inp.repeated("down"):
            self.menu.move(1)

    def draw(self, surf, rect) -> None:
        font = self.app.font
        w = self.world
        lw = 126
        if not self.menu.items:
            ui.draw_text_block(surf, "You have not met anyone yet. People you speak to appear here, "
                               "tracked by their wrist tags.", (rect.x + 10, rect.y + 10, 200, 60), P.UI["dim"])
            return
        self.menu.draw(surf, rect.x + 4, rect.y + 6, lw, self.app.time)
        ch = w.characters.get(self.menu.selected.data) if self.menu.selected else None
        if ch is None:
            return
        x = rect.x + lw + 12
        y = rect.y + 6
        frame = pygame.Rect(x, y, 36, 36)
        surf.fill(P.BLACK, frame)
        surf.fill(P.NAVY[3], frame.inflate(-2, -2))
        surf.blit(self.app.chars.portrait(ch.appearance), (x + 2, y + 2))
        font.draw(surf, ch.name, (x + 42, y), P.UI["select"])
        font.draw(surf, ch.title, (x + 42, y + 10), P.UI["dim"])
        rel = ch.opinion_of("player")
        font.draw(surf, "Regards you: %s" % relation_label(rel), (x + 42, y + 21), P.UI["text"])
        ui.bar(surf, (x + 42, y + 31, 100, 6), (rel + 100) / 200.0,
               P.GREEN[2] if rel >= 15 else P.RED[2] if rel <= -15 else P.STEEL[3])
        y += 44
        width = rect.right - x - 8
        if not ch.present:
            font.draw(surf, "No longer aboard Ithaca Station.", (x, y), P.UI["dim"])
            return
        gmap = w.maps.get(ch.map_id)
        where = gmap.place_name(ch.x, ch.y) if gmap else "?"
        if gmap and where != gmap.name:
            where = "%s, %s" % (where, gmap.name)
        rows = [("Where", where), ("Doing", ch.activity or "-")]
        goal = ai.goal_text(ch)
        if goal:
            rows.append(("Aim", goal))
        rows.append(("Mood", morale_label(ch.morale)))
        notes = []
        for eff in ch.active_effects():
            edef = w.content.effects.get(eff, {})
            notes.append(("<r>%s</r>" if edef.get("bad") else "<g>%s</g>") % edef.get("name", eff))
        if notes:
            rows.append(("State", ", ".join(notes)))
        wants = [k for k in sorted(ch.wants) if k in w.content.items]
        if wants:
            rows.append(("Needs", ", ".join("%s x%d" % (w.item_name(k), ch.wants[k]) for k in wants)))
        for label, value in rows:
            font.draw(surf, label, (x, y), P.UI["accent"])
            for line in font.wrap(value, width - 40)[:2]:
                font.draw(surf, line, (x + 38, y), P.UI["text"])
                y += 10
            y += 2
        if ch.bio:
            ui.draw_text_block(surf, ch.bio, (x, y + 4, width, rect.bottom - y - 8), P.STEEL[4], max_lines=4)


# ---------------------------------------------------------------------------
class StationTab(Tab):
    def __init__(self, scene):
        super().__init__(scene)
        self.menu = ui.Menu(visible=11, wrap=True)

    def open(self) -> None:
        nets = self.world.systems
        items = []
        ordered = nets.generators() + nets.consumers()
        for s in ordered:
            items.append(ui.MenuItem(s.name, "%d%%" % round(s.condition), data=s.id,
                                     color=STATUS_COLORS.get(s.status)))
        self.menu.set_items(items)

    def update(self, inp) -> None:
        if inp.repeated("up"):
            self.menu.move(-1)
        elif inp.repeated("down"):
            self.menu.move(1)

    def draw(self, surf, rect) -> None:
        font = self.app.font
        w = self.world
        nets = w.systems
        x, y = rect.x + 6, rect.y + 6
        supply, demand = nets.power_supply, nets.power_demand
        font.draw(surf, "Power", (x, y), P.UI["accent"])
        ui.bar(surf, (x + 36, y + 1, 100, 7), supply / max(1.0, demand), P.AMBER[2] if nets.brownout else P.GREEN[2])
        font.draw(surf, "%.0f / %.0f kW%s" % (supply, demand, "  <y>BROWNOUT</y>" if nets.brownout else ""),
                  (x + 140, y), P.UI["text"])
        y += 11
        for name, rdef in nets.resource_defs.items():
            frac = nets.resource_frac(name)
            font.draw(surf, str(rdef.get("name", name))[:9], (x, y), P.UI["accent"])
            ui.bar(surf, (x + 36, y + 1, 100, 7), frac, ui.level_color(frac))
            font.draw(surf, "%.0f %s" % (nets.resource(name), rdef.get("unit", "")), (x + 140, y), P.UI["text"])
            y += 11
        font.draw(surf, "Crew aboard: %d" % w.crew_count(), (rect.right - 8, rect.y + 6), P.UI["dim"], align="right")
        y += 4
        surf.fill(P.UI["border_dim"], (rect.x + 4, y, rect.w - 8, 1))
        y += 5
        lw = 140
        self.menu.draw(surf, rect.x + 4, y, lw, self.app.time)
        sel = self.menu.selected
        s = nets.get(sel.data) if sel else None
        if s is None:
            return
        dx = rect.x + lw + 12
        font.draw(surf, s.name, (dx, y), P.UI["select"])
        font.draw(surf, s.status.upper(), (rect.right - 8, y), STATUS_COLORS.get(s.status, P.WHITE), align="right")
        yy = y + 12
        ui.bar(surf, (dx, yy, 120, 7), s.condition / 100.0, ui.level_color(s.condition / 100.0))
        font.draw(surf, "%d%% integrity" % round(s.condition), (dx + 126, yy - 1), P.UI["text"])
        yy += 11
        for line in system_detail_lines(w, s):
            font.draw(surf, line, (dx, yy), P.UI["text"])
            yy += 10
        gmap = w.maps.get(s.map)
        if gmap is not None:
            font.draw(surf, "<d>Located in %s</d>" % gmap.name, (dx, yy), P.UI["text"])
            yy += 10
        ui.draw_text_block(surf, s.desc, (dx, yy + 3, rect.right - dx - 8, rect.bottom - yy - 6), P.STEEL[4],
                           max_lines=max(1, (rect.bottom - yy - 6) // 11))


# ---------------------------------------------------------------------------
class SystemTab(Tab):
    def __init__(self, scene):
        super().__init__(scene)
        self.menu = ui.Menu(visible=12, wrap=True)

    def open(self) -> None:
        opts = self.app.options
        self.menu.set_items([
            ui.MenuItem("Save game", data="save"),
            ui.MenuItem("Load game", data="load"),
            ui.MenuItem("Text speed", TEXT_SPEED_NAMES[int(opts.get("text_speed", 1)) % 4], data="text"),
            ui.MenuItem("Window scale", "x%d" % int(opts.get("scale", 3)), data="scale"),
            ui.MenuItem("Sound volume", "%d%%" % round(float(opts.get("volume", 0.6)) * 100), data="volume"),
            ui.MenuItem("Lighting effects", "On" if opts.get("lighting", True) else "Off", data="lighting"),
            ui.MenuItem("Fullscreen", "On" if self.app.fullscreen else "Off", data="fullscreen"),
            ui.MenuItem("Controls", data="controls"),
            ui.MenuItem("Quit to title", data="title"),
        ])

    def update(self, inp) -> None:
        result = self.menu.update(inp)
        if result != "select":
            return
        app = self.app
        opts = app.options
        action = self.menu.selected.data
        scene = self.scene
        if action == "save":
            app.push_scene(SaveLoadScene(app, "save", self.world, scene.play))
        elif action == "load":
            app.push_scene(SaveLoadScene(app, "load", self.world, scene.play))
        elif action == "text":
            opts["text_speed"] = (int(opts.get("text_speed", 1)) + 1) % 4
        elif action == "scale":
            app.set_scale(int(opts.get("scale", 3)) % 6 + 1)
        elif action == "volume":
            vol = round(float(opts.get("volume", 0.6)) + 0.2, 1)
            opts["volume"] = 0.0 if vol > 1.0 else vol
            app.audio.set_volume(opts["volume"])
        elif action == "lighting":
            opts["lighting"] = not opts.get("lighting", True)
            scene.play.renderer.lighting = bool(opts["lighting"])
        elif action == "fullscreen":
            app.toggle_fullscreen()
        elif action == "controls":
            app.push_scene(MessageScene(app, CONTROLS_TEXT, "Controls"))
        elif action == "title":
            def leave() -> None:
                from .title import TitleScene
                app.fade(lambda: app.set_scene(TitleScene(app)), 0.3)
            app.push_scene(ChoiceScene(app, "Quit to title", "Unsaved progress will be lost.",
                                       [("Quit", leave), ("Stay", None)]))
        app.save_options()
        self.open()

    def draw(self, surf, rect) -> None:
        self.menu.draw(surf, rect.x + 60, rect.y + 20, rect.w - 120, self.app.time)
        self.app.font.draw(surf, self.world.content.title, (rect.centerx, rect.bottom - 26), P.UI["accent"],
                           align="center")
        self.app.font.draw(surf, "F5 quick save   F9 quick load   F11 fullscreen   F12 screenshot",
                           (rect.centerx, rect.bottom - 14), P.UI["dim"], align="center")

    def hints(self):
        return [("E", "Select")]


CONTROLS_TEXT = ("<y>Arrows / WASD</y> move, hold <y>Shift</y> to run. <y>E / Enter / Space</y> talk, use, confirm. "
                 "<y>Esc / Tab</y> datapad. <y>I</y> items, <y>J</y> journal, <y>C</y> crew, <y>M</y> station. "
                 "<y>T</y> wait. <y>F5 / F9</y> quick save and load. <y>F3</y> debug overlay.")


class DatapadScene(Scene):
    name = "datapad"

    def __init__(self, app, world, play, tab: str = "status"):
        super().__init__(app)
        self.world = world
        self.play = play
        self.tabs = {"status": StatusTab(self), "items": ItemsTab(self), "journal": JournalTab(self),
                     "crew": CrewTab(self), "station": StationTab(self), "system": SystemTab(self)}
        self.index = max(0, [t[0] for t in TABS].index(tab) if tab in self.tabs else 0)

    @property
    def tab(self) -> Tab:
        return self.tabs[TABS[self.index][0]]

    def enter(self) -> None:
        self.world.busy = True
        self.tab.open()

    def resume(self) -> None:
        self.input.reset()
        self.tab.open()

    def _switch(self, delta: int) -> None:
        self.index = (self.index + delta) % len(TABS)
        ui.sound("move")
        self.tab.open()

    def update(self, dt: float) -> None:
        inp = self.input
        if inp.pressed("cancel") or inp.pressed("menu"):
            ui.sound("cancel")
            self.close()
            return
        if inp.pressed("left") or inp.pressed("tab_prev"):
            self._switch(-1)
            return
        if inp.pressed("right") or inp.pressed("tab_next"):
            self._switch(1)
            return
        self.tab.update(inp)

    def draw(self, surf) -> None:
        font = self.app.font
        surf.fill(P.NAVY[0])
        x = 6
        for i, (_key, label) in enumerate(TABS):
            w = font.width(label) + 12
            active = i == self.index
            surf.fill(P.UI["border"] if active else P.UI["border_dim"], (x, 3, w, 13))
            surf.fill(P.UI["panel"] if active else P.NAVY[0], (x + 1, 4, w - 2, 13 if active else 11))
            font.draw(surf, label, (x + 6, 6), P.UI["select"] if active else P.UI["dim"])
            x += w + 2
        clock = self.world.clock
        font.draw(surf, "Day %d  %s" % (clock.day, clock.time_str()), (W - 8, 6), P.UI["text"], align="right")
        ui.panel(surf, BODY)
        surf.fill(P.UI["panel"], (7, 16, 0, 0))
        self.tab.draw(surf, BODY)
        ui.key_hints(surf, list(self.tab.hints()) + [("←→", "Tab"), ("X", "Close")])
