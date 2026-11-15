"""Character creator.

    python character_creator.py                          # the editor window
    python character_creator.py --new "Kira Vance" --role engineer
    python character_creator.py --new "Bolt" --role drone --map hub --pos 20,12
    python character_creator.py --list | --roles | --delete kira

**Window.**  Pick a character on the left (or press New), change anything in
the middle, watch the sprite and portrait update on the right.  Click the
little map to choose where they start.  Save writes ``data/characters/<id>.json``;
the next game you start - or load - has them walking around doing their job.

**Command line.**  ``--new`` builds a complete character from a role preset
in one step: stats, a random look that suits the role, a free bunk, a job, a
schedule and a start position next to their workplace.

Characters without a dialogue file of their own use ``data/dialogue/generic.json``,
so a new crew member can be talked to (and asked what they are up to, and
given things they are looking for) straight away.
"""
from __future__ import annotations

import argparse
import copy
import json
import os
import random
import subprocess
import sys
from typing import Any, Dict, List, Optional, Tuple

import pygame

from game import config
from game.art import chars as chargen_art
from game.art.icons import IconArt
from game.art.tiles import TileArt
from game.content import Content, write_json
from game.engine import palette as P
from game.scenes.render import MapRenderer
from game.tools import chargen, gui
from game.world.character import ATTR_LABELS, ATTRIBUTES, SKILL_LABELS, SKILLS
from game.world.items import format_item_list, parse_item_list

APPEARANCE_FIELDS = [
    ("body", "Body", lambda: list(chargen_art.BODIES)),
    ("skin", "Skin", lambda: list(P.SKIN_TONES)),
    ("hair", "Hair", lambda: list(chargen_art.HAIR_STYLES)),
    ("hair_color", "Hair colour", lambda: list(P.HAIR_COLORS)),
    ("outfit", "Outfit", lambda: list(chargen_art.OUTFIT_STYLES)),
    ("primary", "Main colour", lambda: list(P.CLOTH_COLORS)),
    ("secondary", "Trim colour", lambda: list(P.CLOTH_COLORS)),
    ("legs", "Trousers", lambda: list(P.CLOTH_COLORS)),
]
EXTRA_SETS = [[], ["glasses"], ["beard"], ["beard", "glasses"]]
NONE = "(none)"
GENERIC = "(generic)"


class CharacterCreator(gui.ToolApp):
    title = "Tethys Deep - Character Creator"

    def __init__(self, char_id: Optional[str] = None, headless: bool = False, content: Optional[Content] = None):
        super().__init__(headless)
        self.content = content or Content.load()
        self.maps = self.content.fresh_maps()
        self.char_art = chargen_art.CharArt()
        self.renderer = MapRenderer(TileArt(), self.char_art, IconArt())
        self.rng = random.Random()
        self.data: Dict[str, Any] = {}
        self.orig_id: Optional[str] = None       # id on disk (None for a new character)
        self.dirty = False
        self.problems: List[str] = []
        self._thumbs: Dict[str, pygame.Surface] = {}
        self.labels: List[Tuple[str, gui.Widget]] = []
        self._build()
        self.layout()
        ids = sorted(self.content.characters)
        if char_id in self.content.characters:
            self.load(char_id)
        elif ids:
            self.load(ids[0])
        else:
            self.new_character()

    # ------------------------------------------------------------------
    # widgets
    # ------------------------------------------------------------------
    def _build(self) -> None:
        B, TI, CY, SP = gui.Button, gui.TextInput, gui.Cycler, gui.Spinner
        self.char_list = gui.ListBox((0, 0, 1, 1), [], self._pick_char)
        self.btn_new = B((0, 0, 1, 1), "New", self.cmd_new)
        self.btn_copy = B((0, 0, 1, 1), "Copy", self.cmd_copy)
        self.btn_delete = B((0, 0, 1, 1), "Delete", self.cmd_delete)

        def text(key: str, numeric: bool = False) -> gui.TextInput:
            return TI((0, 0, 1, 1), "", on_commit=lambda v, k=key: self._set_text(k, v), numeric=numeric)

        self.f_name = text("name")
        self.f_id = text("id")
        self.f_title = text("title")
        self.f_faction = text("faction")
        self.f_bio = text("bio")
        roles = sorted(self.content.archetypes) or ["civilian"]
        self.f_role = CY((0, 0, 1, 1), roles, on_change=lambda v: self._set("role", v))
        self.btn_role = B((0, 0, 1, 1), "Apply role preset", self.cmd_apply_role)

        self.app_cyclers: Dict[str, gui.Cycler] = {}
        for key, _label, options in APPEARANCE_FIELDS:
            self.app_cyclers[key] = CY((0, 0, 1, 1), options(), on_change=lambda v, k=key: self._set_look(k, v))
        self.f_extras = CY((0, 0, 1, 1), EXTRA_SETS, on_change=lambda v: self._set_look("extras", list(v)),
                           fmt=lambda v: ", ".join(v) if v else "none")
        self.btn_random = B((0, 0, 1, 1), "Randomise look", self.cmd_random_look)

        self.attr_spin = {a: SP((0, 0, 1, 1), 5, 1, 10, on_change=lambda v, k=a: self._set_stat("attrs", k, v))
                          for a in ATTRIBUTES}
        self.skill_spin = {s: SP((0, 0, 1, 1), 0, 0, 10, on_change=lambda v, k=s: self._set_stat("skills", k, v))
                           for s in SKILLS}

        self.f_map = CY((0, 0, 1, 1), self.content.map_ids or [""], on_change=self._set_map)
        beds = [NONE] + sorted(o.id for g in self.maps.values() for o in g.objects if o.type == "bed")
        self.f_bed = CY((0, 0, 1, 1), beds, on_change=lambda v: self._set_home(v))
        systems = [NONE] + list((self.content.systems.get("systems") or {}))
        self.f_job = CY((0, 0, 1, 1), systems, on_change=lambda v: self._set_job(v))
        self.f_schedule = CY((0, 0, 1, 1), sorted(self.content.schedules) or ["day"],
                             on_change=lambda v: self._set("schedule", v))
        self.f_dialogue = CY((0, 0, 1, 1), [GENERIC] + sorted(self.content.dialogues),
                             on_change=lambda v: self._set("dialogue", "" if v == GENERIC else v))
        self.btn_dialogue = B((0, 0, 1, 1), "Write dialogue file", self.cmd_make_dialogue)
        self.f_inventory = text("inventory")
        self.f_credits = text("credits", numeric=True)
        self.f_access = text("access")
        self.f_traits = text("traits")
        self.btn_goals = B((0, 0, 1, 1), "Goals...", self.cmd_goals)
        self.btn_add_goal = B((0, 0, 1, 1), "+ Goal from template", self.cmd_add_goal)

        self.btn_save = B((0, 0, 1, 1), "Save", self.cmd_save)
        self.btn_revert = B((0, 0, 1, 1), "Revert", self.cmd_revert)
        self.btn_play = B((0, 0, 1, 1), "Play here", self.cmd_play)

        self.widgets = [self.char_list, self.btn_new, self.btn_copy, self.btn_delete,
                        self.f_name, self.f_id, self.f_title, self.f_faction, self.f_bio, self.f_role, self.btn_role,
                        self.f_extras, self.btn_random, self.f_map, self.f_bed, self.f_job, self.f_schedule,
                        self.f_dialogue, self.btn_dialogue, self.f_inventory, self.f_credits, self.f_access,
                        self.f_traits, self.btn_goals, self.btn_add_goal, self.btn_save, self.btn_revert,
                        self.btn_play]
        self.widgets += list(self.app_cyclers.values()) + list(self.attr_spin.values()) + list(self.skill_spin.values())

    def layout(self) -> None:
        sw, sh = self.screen.get_size()
        self.labels = []
        left = 210
        self.char_list.rect = pygame.Rect(8, 34, left - 16, sh - 34 - 76)
        y = sh - 68
        for i, b in enumerate((self.btn_new, self.btn_copy, self.btn_delete)):
            b.rect = pygame.Rect(8 + i * 66, y, 62, 28)

        # middle column -------------------------------------------------------
        mx, mw = left + 12, 500
        lw = 110

        def row(label: str, widget: gui.Widget, y: int, x: int = mx, w: int = mw, label_w: int = lw, h: int = 26) -> None:
            widget.rect = pygame.Rect(x + label_w, y, w - label_w, h)
            self.labels.append((label, widget))

        y = 34
        for label, w in (("Name", self.f_name), ("Id", self.f_id), ("Title", self.f_title),
                         ("Faction", self.f_faction), ("Bio", self.f_bio)):
            row(label, w, y)
            y += 29
        row("Role", self.f_role, y, w=mw - 190)
        self.btn_role.rect = pygame.Rect(mx + mw - 184, y, 184, 26)
        y += 42
        self.y_look = y - 2
        y += 22
        half = mw // 2 - 4
        for i, (key, label, _options) in enumerate(APPEARANCE_FIELDS):
            col = i % 2
            row(label, self.app_cyclers[key], y + (i // 2) * 29, x=mx + col * (half + 8), w=half, label_w=92)
        y += 4 * 29
        row("Extras", self.f_extras, y, w=half, label_w=92)
        self.btn_random.rect = pygame.Rect(mx + half + 8, y, half, 26)
        y += 42
        self.y_attrs = y - 2
        y += 22
        aw = mw // 5
        for i, a in enumerate(ATTRIBUTES):
            self.attr_spin[a].rect = pygame.Rect(mx + i * aw, y + 18, aw - 6, 26)
        y += 60
        self.y_skills = y - 2
        y += 22
        sw3 = mw // 3
        for i, s in enumerate(SKILLS):
            row(SKILL_LABELS[s], self.skill_spin[s], y + (i // 3) * 29, x=mx + (i % 3) * sw3, w=sw3 - 6, label_w=90)
        y += 3 * 29 + 12
        self.y_misc = y - 2
        y += 22
        for label, w in (("Inventory", self.f_inventory), ("Access", self.f_access), ("Traits", self.f_traits)):
            row(label, w, y)
            y += 29
        row("Credits", self.f_credits, y, w=220)
        self.btn_goals.rect = pygame.Rect(mx + 230, y, 90, 26)
        self.btn_add_goal.rect = pygame.Rect(mx + 326, y, mw - 326, 26)

        # right column --------------------------------------------------------
        rx = mx + mw + 16
        rw = sw - rx - 10
        self.preview = pygame.Rect(rx, 34, rw, 150)
        y = 196
        self.y_world = y - 2
        y += 22
        row("Map", self.f_map, y, x=rx, w=rw, label_w=84)
        y += 30
        self.minimap = pygame.Rect(rx, y, rw, max(120, sh - y - 250))
        y = self.minimap.bottom + 22
        for label, w in (("Bunk", self.f_bed), ("Job system", self.f_job), ("Schedule", self.f_schedule)):
            row(label, w, y, x=rx, w=rw, label_w=96)
            y += 29
        row("Dialogue", self.f_dialogue, y, x=rx, w=rw, label_w=96)
        y += 29
        self.btn_dialogue.rect = pygame.Rect(rx + 96, y, rw - 96, 26)
        y = sh - 66
        bw = (rw - 12) // 3
        for i, b in enumerate((self.btn_save, self.btn_revert, self.btn_play)):
            b.rect = pygame.Rect(rx + i * (bw + 6), y, bw, 30)
        self.mid_x, self.mid_w, self.right_x = mx, mw, rx

    # ------------------------------------------------------------------
    # data <-> widgets
    # ------------------------------------------------------------------
    def refresh_list(self) -> None:
        items = [("%s  (%s)" % (cd.get("name", cid), cid), cid) for cid, cd in sorted(self.content.characters.items())]
        if self.orig_id is None and self.data:
            items.append(("* %s  (new)" % self.data.get("name", "?"), "__new__"))
        self.char_list.set_items(items, keep=False)
        self.char_list.select(self.orig_id if self.orig_id else "__new__")

    def sync_widgets(self) -> None:
        d = self.data
        self.focus(None)
        self.f_name.set(d.get("name", ""))
        self.f_id.set(d.get("id", ""))
        self.f_title.set(d.get("title", ""))
        self.f_faction.set(d.get("faction", ""))
        self.f_bio.set(d.get("bio", ""))
        if d.get("role") in self.f_role.options:
            self.f_role.value = d["role"]
        look = chargen_art.normalize(d.get("appearance"))
        for key, cyc in self.app_cyclers.items():
            cyc.value = look[key]
        extras = sorted(look.get("extras") or [])
        self.f_extras.value = extras if extras in EXTRA_SETS else []
        for a, sp in self.attr_spin.items():
            sp.value = int((d.get("attrs") or {}).get(a, 4))
        for s, sp in self.skill_spin.items():
            sp.value = int((d.get("skills") or {}).get(s, 0))
        spawn = d.get("spawn") or {}
        if spawn.get("map") in self.f_map.options:
            self.f_map.value = spawn["map"]
        bed = (d.get("home") or {}).get("bed") or NONE
        self.f_bed.value = bed if bed in self.f_bed.options else NONE
        job = (d.get("job") or {}).get("system") or NONE
        self.f_job.value = job if job in self.f_job.options else NONE
        sched = d.get("schedule", "day")
        if isinstance(sched, str) and sched in self.f_schedule.options:
            self.f_schedule.value = sched
        self.f_dialogue.set_options([GENERIC] + sorted(self.content.dialogues))
        dlg = d.get("dialogue") or (d.get("id") if d.get("id") in self.content.dialogues else "")
        self.f_dialogue.value = dlg if dlg in self.f_dialogue.options else GENERIC
        self.f_inventory.set(format_item_list(d.get("inventory") or {}))
        self.f_credits.set(d.get("credits", 0))
        self.f_access.set(", ".join(d.get("access") or []))
        self.f_traits.set(", ".join(d.get("traits") or []))
        self.refresh_list()

    def touch(self) -> None:
        self.dirty = True

    def _set(self, key: str, value: Any) -> None:
        self.data[key] = value
        self.touch()

    def _set_text(self, key: str, value: str) -> None:
        d = self.data
        if key == "id":
            d["id"] = "".join(c for c in value.strip().lower().replace(" ", "_") if c.isalnum() or c == "_")
            d["_id_locked"] = True          # stop deriving the id from the name
            self.f_id.set(d["id"])
        elif key == "name":
            d["name"] = value.strip()
            if self.orig_id is None and not self._id_edited():
                d["id"] = chargen.unique_id(self.content, chargen.slugify(value))
                self.f_id.set(d["id"])
        elif key == "inventory":
            d["inventory"] = parse_item_list(value)
            unknown = [i for i in d["inventory"] if i not in self.content.items]
            if unknown:
                self.set_status("Unknown item(s): %s" % ", ".join(unknown), 6)
        elif key == "credits":
            try:
                d["credits"] = int(float(value or 0))
            except ValueError:
                d["credits"] = 0
        elif key in ("access", "traits"):
            d[key] = [v.strip() for v in value.split(",") if v.strip()]
        else:
            d[key] = value
        self.touch()
        self.refresh_list()

    def _id_edited(self) -> bool:
        return bool(self.data.get("_id_locked"))

    def _set_look(self, key: str, value: Any) -> None:
        look = dict(self.data.get("appearance") or {})
        look[key] = value
        self.data["appearance"] = chargen_art.normalize(look)
        self.touch()

    def _set_stat(self, group: str, key: str, value: int) -> None:
        self.data.setdefault(group, {})[key] = int(value)
        self.touch()

    def _set_map(self, map_id: str) -> None:
        gmap = self.maps.get(map_id)
        cell = chargen.free_spawn(self.content, gmap, near=(gmap.width // 2, gmap.height // 2) if gmap else None,
                                  ignore=self.orig_id or "")
        spawn = dict(self.data.get("spawn") or {})
        spawn.update(map=map_id, x=cell[0] if cell else 1, y=cell[1] if cell else 1)
        spawn.setdefault("facing", "down")
        self.data["spawn"] = spawn
        self.touch()

    def _set_home(self, bed: str) -> None:
        self.data["home"] = {} if bed == NONE else {"bed": bed}
        self.touch()

    def _set_job(self, system: str) -> None:
        job = dict(self.data.get("job") or {})
        if system == NONE:
            job.pop("system", None)
        else:
            job["system"] = system
        self.data["job"] = job
        self.touch()

    # ------------------------------------------------------------------
    # commands
    # ------------------------------------------------------------------
    def load(self, cid: str) -> None:
        self.data = copy.deepcopy(self.content.characters[cid])
        self.orig_id = cid
        self.dirty = False
        self.problems = chargen.problems_for(self.content, cid)
        self.sync_widgets()

    def new_character(self, role: str = "civilian", name: str = "New Crew Member") -> None:
        self.data = chargen.new_character(self.content, name, role, rng=self.rng, maps=self.maps)
        self.orig_id = None
        self.dirty = True
        self.problems = []
        self.sync_widgets()

    def _guard(self, then) -> None:
        if self.dirty:
            self.confirm("Unsaved changes", "Discard the changes to %s?" % self.data.get("name", "this character"), then)
        else:
            then()

    def _pick_char(self, cid: str) -> None:
        if cid == "__new__" or cid == self.orig_id:
            return
        self._guard(lambda: self.load(cid))
        self.refresh_list()

    def cmd_new(self) -> None:
        def chosen(role: Optional[str]) -> None:
            if role:
                self.new_character(role)
                self.set_status("New %s. Give them a name, then Save." % role)
        roles = [("%s  -  %s" % (r, a.get("title", "")), r) for r, a in sorted(self.content.archetypes.items())]
        self._guard(lambda: self.pick_list("New character: choose a role", roles, chosen))

    def cmd_copy(self) -> None:
        if not self.data:
            return
        data = copy.deepcopy(self.data)
        data["id"] = chargen.unique_id(self.content, data.get("id", "copy"))
        data["name"] = "%s (copy)" % data.get("name", "")
        cell = chargen.free_spawn(self.content, self.maps.get((data.get("spawn") or {}).get("map")),
                                  near=(data["spawn"].get("x", 1), data["spawn"].get("y", 1)))
        if cell:
            data["spawn"].update(x=cell[0], y=cell[1])
        data["home"] = {"bed": chargen.free_bed(self.content, self.maps)} if chargen.free_bed(self.content, self.maps) else {}
        self.data, self.orig_id, self.dirty = data, None, True
        self.sync_widgets()

    def cmd_delete(self) -> None:
        if self.orig_id is None:
            ids = sorted(self.content.characters)
            if ids:
                self.load(ids[0])
            return
        cid = self.orig_id

        def really() -> None:
            chargen.delete_character(self.content, cid)
            ids = sorted(self.content.characters)
            if ids:
                self.load(ids[0])
            else:
                self.new_character()
            self.set_status("Deleted %s." % cid)
        self.confirm("Delete character", "Delete %s and their file data/characters/%s.json?" % (
            self.data.get("name", cid), cid), really)

    def cmd_apply_role(self) -> None:
        role = self.f_role.value
        fresh = chargen.new_character(self.content, self.data.get("name", ""), role, rng=self.rng, maps=self.maps,
                                      char_id=self.data.get("id"))
        for key in ("title", "faction", "attrs", "skills", "traits", "access", "job", "schedule", "inventory",
                    "equipped", "credits"):
            self.data[key] = fresh[key]
        self.data["role"] = role
        self.touch()
        self.sync_widgets()
        self.set_status("Applied the %s preset (look and position kept)." % role)

    def cmd_random_look(self) -> None:
        archetype = self.content.archetypes.get(self.data.get("role", ""))
        self.data["appearance"] = chargen.random_looks(archetype, self.rng)
        self.touch()
        self.sync_widgets()

    def cmd_goals(self) -> None:
        def apply(values: List[str]) -> None:
            try:
                goals = json.loads(values[0] or "[]")
                if not isinstance(goals, list):
                    raise ValueError("goals must be a list")
            except ValueError as exc:
                self.message("Goals", "That is not valid JSON:\n%s" % exc)
                return
            self.data["goals"] = goals
            self.touch()
        self.prompt("Goals (JSON list - see docs/AUTHORING.md)",
                    [("Goals", json.dumps(self.data.get("goals") or []))], apply)

    def cmd_add_goal(self) -> None:
        templates = self.content.goals

        def chosen(name: Optional[str]) -> None:
            if not name:
                return
            placeholders = sorted(set(_placeholders(templates[name])))

            def filled(values: List[str]) -> None:
                args = {p: gui.parse_value(v) for p, v in zip(placeholders, values)}
                goal = {"use": name, "id": name, "args": args}
                self.data.setdefault("goals", []).append(goal)
                self.touch()
                self.set_status("Added goal '%s'. Use Goals... to fine-tune priority or conditions." % name)
            if placeholders:
                self.prompt("Goal: %s" % name, [(p, "") for p in placeholders], filled)
            else:
                filled([])
        items = [("%s  -  %s" % (k, str(v.get("desc", ""))[:36]), k) for k, v in sorted(templates.items())]
        if items:
            self.pick_list("Add a goal from data/goals.json", items, chosen)
        else:
            self.message("Goals", "data/goals.json has no templates.")

    def cmd_make_dialogue(self) -> None:
        cid = self.data.get("id", "")
        if not cid:
            return
        path = self.content.dialogue_path(cid)
        if os.path.exists(path):
            self.data["dialogue"] = cid
            self.touch()
            self.sync_widgets()
            self.set_status("%s already exists; using it." % os.path.relpath(path))
            return
        write_json(path, chargen.dialogue_stub(self.data))
        self.content.reload()
        self.data["dialogue"] = cid
        self.touch()
        self.sync_widgets()
        self.set_status("Wrote %s - open it in a text editor to write their lines." % os.path.relpath(path), 8)

    def cmd_save(self) -> bool:
        d = self.data
        cid = d.get("id", "")
        if not cid or cid == "player":
            self.message("Save", "The character needs an id (and it cannot be 'player').")
            return False
        if cid != self.orig_id and cid in self.content.characters:
            self.message("Save", "There is already a character with the id '%s'." % cid)
            return False
        d.pop("_id_locked", None)
        try:
            path = chargen.save_character(self.content, d, self.orig_id)
        except OSError as exc:
            self.message("Save failed", str(exc))
            return False
        self.orig_id = cid
        self.dirty = False
        self.maps = self.content.fresh_maps()
        self.problems = chargen.problems_for(self.content, cid)
        self.sync_widgets()
        self.set_status("Saved characters/%s.  %s" % (os.path.basename(path), "No problems found." if not self.problems
                                                      else "%d problem(s) found." % len(self.problems)), 5)
        return True

    def cmd_revert(self) -> None:
        if self.orig_id and self.orig_id in self.content.characters:
            self.load(self.orig_id)
            self.set_status("Reverted.")

    def cmd_play(self) -> None:
        if not self.cmd_save():
            return
        sp = self.data.get("spawn") or {}
        gmap = self.maps.get(sp.get("map"))
        cell = chargen.free_spawn(self.content, gmap, near=(int(sp.get("x", 1)), int(sp.get("y", 1)) + 1))
        args = [sys.executable, os.path.join(config.ROOT, "main.py"), "--map", str(sp.get("map"))]
        if cell:
            args += ["--pos", "%d,%d" % cell]
        if self.headless:
            self.set_status("Play: " + " ".join(args[1:]))
            return
        try:
            subprocess.Popen(args, cwd=config.ROOT)
        except OSError as exc:
            self.message("Play", "Could not start the game: %s" % exc)

    def can_quit(self) -> bool:
        if not self.dirty:
            return True
        self.confirm("Quit", "There are unsaved changes. Quit anyway?", lambda: setattr(self, "running", False))
        return False

    # ------------------------------------------------------------------
    # events / drawing
    # ------------------------------------------------------------------
    def on_event(self, event) -> bool:
        if event.type == pygame.MOUSEBUTTONDOWN and event.button == 1 and self.minimap.collidepoint(event.pos):
            cell = self._minimap_cell(event.pos)
            gmap = self.maps.get((self.data.get("spawn") or {}).get("map"))
            if cell and gmap is not None:
                if gmap.solid_at(*cell):
                    self.set_status("That tile is not walkable.")
                else:
                    self.data["spawn"].update(x=cell[0], y=cell[1])
                    self.touch()
                    self.set_status("%s will start at (%d, %d)." % (self.data.get("name"), cell[0], cell[1]))
            return True
        if event.type == pygame.KEYDOWN and event.key == pygame.K_s and (event.mod & pygame.KMOD_CTRL):
            self.cmd_save()
            return True
        return False

    def _thumb(self, map_id: str) -> Optional[pygame.Surface]:
        gmap = self.maps.get(map_id)
        if gmap is None:
            return None
        if map_id not in self._thumbs:
            self._thumbs[map_id] = self.renderer.render_whole_map(gmap, time=0.3)
        return self._thumbs[map_id]

    def _minimap_layout(self):
        map_id = (self.data.get("spawn") or {}).get("map")
        thumb = self._thumb(map_id)
        if thumb is None:
            return None
        k = min(self.minimap.w / thumb.get_width(), self.minimap.h / thumb.get_height())
        w, h = max(1, int(thumb.get_width() * k)), max(1, int(thumb.get_height() * k))
        rect = pygame.Rect(self.minimap.centerx - w // 2, self.minimap.centery - h // 2, w, h)
        return thumb, rect, k

    def _minimap_cell(self, pos) -> Optional[Tuple[int, int]]:
        layout = self._minimap_layout()
        if layout is None:
            return None
        _thumb, rect, k = layout
        if not rect.collidepoint(pos):
            return None
        return int((pos[0] - rect.x) / k) // config.TILE, int((pos[1] - rect.y) / k) // config.TILE

    def draw(self) -> None:
        scr = self.screen
        scr.fill(gui.BG)
        sw, sh = scr.get_size()
        f = gui.font()
        small = gui.font(1)
        pygame.draw.rect(scr, gui.PANEL, (0, 0, sw, 28))
        f.draw(scr, "CHARACTERS", (10, 5), gui.ACCENT)
        name = self.data.get("name", "")
        f.draw(scr, "%s%s" % (name, "  *unsaved*" if self.dirty else ""), (self.mid_x, 5), gui.SELECT)
        for label, widget in self.labels:
            small_label = widget in self.app_cyclers.values() or widget in self.skill_spin.values()
            (small if small_label else f).draw(scr, label, (widget.rect.x - 4, widget.rect.y + (9 if small_label else 4)),
                                               gui.DIM, align="right")
        mx = self.mid_x
        for y, title in ((self.y_look, "APPEARANCE"), (self.y_attrs, "ATTRIBUTES"), (self.y_skills, "SKILLS"),
                         (self.y_misc, "BELONGINGS AND ACCESS")):
            f.draw(scr, title, (mx, y), gui.ACCENT)
            pygame.draw.rect(scr, P.CYAN[0], (mx + f.width(title) + 8, y + 9, self.mid_w - f.width(title) - 8, 1))
        for a, sp in self.attr_spin.items():
            small.draw(scr, ATTR_LABELS[a], (sp.rect.centerx, sp.rect.y - 12), gui.DIM, align="center")
        f.draw(scr, "WORLD", (self.right_x, self.y_world), gui.ACCENT)

        # preview
        pr = self.preview
        pygame.draw.rect(scr, P.BLACK, pr)
        pygame.draw.rect(scr, P.STEEL[2], pr.inflate(-2, -2))
        look = self.data.get("appearance") or {}
        frame = int(self.time * 6) % 4
        x = pr.x + 14
        for direction in ("down", "left", "up", "right"):
            sprite = self.char_art.sprite(look, direction, frame)
            scr.blit(pygame.transform.scale(sprite, (64, 96)), (x, pr.y + 12))
            x += 70
        portrait = pygame.transform.scale(self.char_art.portrait(look), (128, 128))
        box = pygame.Rect(pr.right - 142, pr.y + 10, 132, 132)
        pygame.draw.rect(scr, P.NAVY[3], box)
        scr.blit(portrait, (box.x + 2, box.y + 2))
        small.draw(scr, "%s  -  %s" % (self.data.get("title", ""), self.data.get("role", "")),
                   (pr.x + 14, pr.bottom - 26), P.WHITE, shadow=P.BLACK)

        # mini map
        mm = self.minimap
        pygame.draw.rect(scr, P.BLACK, mm.inflate(4, 4))
        layout = self._minimap_layout()
        spawn = self.data.get("spawn") or {}
        if layout is not None:
            thumb, rect, k = layout
            scr.blit(pygame.transform.scale(thumb, rect.size), rect.topleft)
            t = config.TILE * k
            for cid, cd in self.content.characters.items():
                sp = cd.get("spawn") or {}
                if sp.get("map") == spawn.get("map") and cid != self.orig_id:
                    cx, cy = rect.x + (int(sp.get("x", 0)) + 0.5) * t, rect.y + (int(sp.get("y", 0)) + 0.5) * t
                    pygame.draw.circle(scr, P.BLACK, (cx, cy), 4)
                    pygame.draw.circle(scr, P.GREEN[3], (cx, cy), 3)
            cx = rect.x + (int(spawn.get("x", 0)) + 0.5) * t
            cy = rect.y + (int(spawn.get("y", 0)) + 0.5) * t
            pulse = 5 + int(self.time * 3) % 2
            pygame.draw.circle(scr, P.BLACK, (cx, cy), pulse + 1)
            pygame.draw.circle(scr, gui.SELECT, (cx, cy), pulse)
            pygame.draw.circle(scr, P.BLACK, (cx, cy), 2)
        small.draw(scr, "Click the map to choose where they start: (%s, %s)   green = other crew" % (
            spawn.get("x", "?"), spawn.get("y", "?")), (mm.x, mm.bottom + 5), gui.DIM)

        # status + validation (top bar, right-hand side)
        if self.status:
            text, color = self.status, gui.TEXT
        elif self.problems:
            text, color = "%d problem(s): %s" % (len(self.problems), self.problems[0].split(": ", 1)[-1]), P.AMBER[3]
        else:
            text, color = "Ctrl+S saves. New crew appear when the game next starts or loads.", gui.DIM
        text = text.replace("<", "<<")
        room = sw - self.right_x - 10
        while text and small.width(text) > room:
            text = text[:-1]
        small.draw(scr, text, (self.right_x, 10), color)


def _placeholders(value: Any) -> List[str]:
    out: List[str] = []
    if isinstance(value, str):
        i = 0
        while True:
            i = value.find("$", i)
            if i == -1:
                break
            j = i + 1
            while j < len(value) and (value[j].isalnum() or value[j] == "_"):
                j += 1
            if j > i + 1:
                out.append(value[i + 1:j])
            i = j
    elif isinstance(value, list):
        for v in value:
            out += _placeholders(v)
    elif isinstance(value, dict):
        for v in value.values():
            out += _placeholders(v)
    return out


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description="Create and edit characters for Tethys Deep.")
    parser.add_argument("character", nargs="?", help="id of a character to open in the editor")
    parser.add_argument("--new", metavar="NAME", help="create a character without opening the editor")
    parser.add_argument("--role", default="civilian", help="role preset from data/archetypes.json")
    parser.add_argument("--map", default=None, help="map the character starts on (default: their workplace)")
    parser.add_argument("--pos", default=None, help="start position x,y (default: a free tile near their job)")
    parser.add_argument("--id", default=None, help="character id (default: from the name)")
    parser.add_argument("--seed", type=int, default=None, help="random seed for looks and stats")
    parser.add_argument("--dialogue", action="store_true", help="also write a starter dialogue file")
    parser.add_argument("--list", action="store_true", help="list existing characters")
    parser.add_argument("--roles", action="store_true", help="list the role presets")
    parser.add_argument("--delete", metavar="ID", help="delete a character file")
    args = parser.parse_args(argv)

    content = Content.load()
    if args.roles:
        for role, a in sorted(content.archetypes.items()):
            skills = ", ".join("%s %d" % kv for kv in sorted((a.get("skills") or {}).items(), key=lambda kv: -kv[1]))
            print("%-14s %-20s %s" % (role, a.get("title", ""), skills))
        return 0
    if args.list:
        for cid, cd in sorted(content.characters.items()):
            sp = cd.get("spawn") or {}
            print("%-10s %-24s %-22s %s (%s, %s)" % (cid, cd.get("name", ""), cd.get("title", ""),
                                                     sp.get("map", "?"), sp.get("x", "?"), sp.get("y", "?")))
        return 0
    if args.delete:
        if chargen.delete_character(content, args.delete):
            print("deleted", args.delete)
            return 0
        print("no such character:", args.delete)
        return 1
    if args.new:
        if args.role not in content.archetypes:
            print("unknown role %r; try --roles" % args.role)
            return 1
        if args.map and args.map not in content.map_ids:
            print("unknown map %r; available: %s" % (args.map, ", ".join(content.map_ids)))
            return 1
        pos = None
        if args.pos:
            try:
                pos = tuple(int(v) for v in args.pos.split(","))
            except ValueError:
                print("--pos must look like 12,7")
                return 1
        rng = random.Random(args.seed)
        data = chargen.new_character(content, args.new, args.role, args.map, pos, rng, args.id)
        if args.dialogue:
            write_json(content.dialogue_path(data["id"]), chargen.dialogue_stub(data))
            data["dialogue"] = data["id"]
        path = chargen.save_character(content, data)
        print("wrote", os.path.relpath(path))
        print(chargen.summary(data))
        for line in chargen.problems_for(content, data["id"]):
            print("  !", line)
        print("They will be on the station the next time the game starts or a save is loaded.")
        return 0
    CharacterCreator(args.character, content=content).run()
    return 0


if __name__ == "__main__":
    sys.exit(main())
