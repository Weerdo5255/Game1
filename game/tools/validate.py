"""Check every content file for mistakes.

    python -m game.tools.validate          # errors and warnings for ./data
    python -m game.tools.validate --api    # list every script function
    python -m game.tools.validate --data path/to/other/data

It catches the things that are tedious to find by playing: a ``goto`` to a
node that does not exist, a misspelt item id, a portal that drops the player
inside a wall, a script calling an unknown function, a flag that is read but
never set.  The test-suite runs this over the shipped campaign, and the
editors call :func:`validate` after saving.
"""
from __future__ import annotations

import ast
import sys
from collections import deque
from typing import Any, Dict, Iterable, List, Optional, Set, Tuple

from game import config
from game.art import chars as chargen
from game.art import icons as icon_art
from game.art import tiles as tiledefs
from game.engine import palette
from game.sim import ai
from game.world.character import ATTRIBUTES, NEEDS, SKILLS
from game.world.gamemap import OBJECT_TYPES
from game.world.items import KINDS, SLOTS

# script function -> [(argument index or keyword, kind)]
REFS: Dict[str, List[Tuple[Any, str]]] = {
    "flag": [(0, "flag_get")], "var": [(0, "flag_get")],
    "set_flag": [(0, "flag_set")], "clear_flag": [(0, "flag_set")],
    "set_var": [(0, "flag_set")], "add_var": [(0, "flag_set")],
    "has_item": [(0, "item"), (2, "char")], "item_count": [(0, "item"), (1, "char")],
    "equipped": [(0, "item")], "give_item": [(0, "item"), (2, "char")],
    "take_item": [(0, "item"), (2, "char")], "give_to": [(0, "char"), (1, "item")],
    "take_from": [(0, "char"), (1, "item")], "npc_has": [(0, "char"), (1, "item")],
    "item_name": [(0, "item")], "stock": [(0, "item")],
    "container_count": [(0, "object"), (1, "item")], "put_item": [(0, "object"), (1, "item")],
    "remove_from": [(0, "object"), (1, "item")], "spawn_item": [(0, "map"), (3, "item")],
    "move_items": [(0, "inventory"), (1, "inventory"), (2, "item")],
    "skill": [(0, "skill"), (1, "char")], "attr": [(0, "attr"), (1, "char")],
    "roll": [(0, "skill")], "add_skill": [(0, "skill")], "add_attr": [(0, "attr")],
    "need": [(0, "need"), (1, "char")], "add_need": [(0, "need"), (2, "char")],
    "add_effect": [(0, "effect"), (2, "char")], "has_effect": [(0, "effect"), (1, "char")],
    "remove_effect": [(0, "effect")],
    "rel": [(0, "char")], "add_rel": [(1, "char")], "met": [(0, "char")], "talked": [(0, "char")],
    "present": [(0, "char")], "asleep": [(0, "char")], "npc_in": [(0, "char"), (1, "map")],
    "npc_region": [(0, "char")], "near": [(0, "char")], "name": [(0, "char")],
    "first_name": [(0, "char")], "title": [(0, "char")], "morale": [(0, "char")],
    "add_morale": [(1, "char_or_all")], "say": [(1, "char")], "activity": [(0, "char")],
    "goal_text": [(0, "char")], "npc_goal": [(0, "char")], "wants": [(0, "char")],
    "wants_count": [(0, "char")], "goal_active": [(0, "char")], "goal_done": [(0, "char")],
    "goal_failed": [(0, "char")], "goal_progress": [(0, "char")],
    "add_goal": [(0, "char"), (1, "goal")], "cancel_goal": [(0, "char")],
    "set_goal_priority": [(0, "char")], "set_schedule": [(0, "char"), (1, "schedule")],
    "set_dialogue": [(0, "char"), (1, "dialogue")], "set_maintain": [(0, "char")],
    "move_npc": [(0, "char"), (1, "map")], "remove_npc": [(0, "char")], "set_met": [(0, "char")],
    "rel_between": [(0, "char"), (1, "char")], "add_rel_between": [(0, "char"), (1, "char")],
    "can_trade": [(0, "char")], "open_trade": [(0, "char")], "has_trait": [(1, "char")],
    "cond": [(0, "system")], "eff": [(0, "system")], "online": [(0, "system")],
    "enabled": [(0, "system")], "sys_status": [(0, "system")], "priority": [(0, "system")],
    "cap": [(0, "system")], "repair": [(0, "system")], "damage": [(0, "system")],
    "set_condition": [(0, "system")], "set_enabled": [(0, "system")],
    "set_priority": [(0, "system")], "set_cap": [(0, "system")],
    "res": [(0, "resource")], "res_pct": [(0, "resource")], "add_res": [(0, "resource")],
    "quest_active": [(0, "quest")], "quest_done": [(0, "quest")], "quest_failed": [(0, "quest")],
    "quest_started": [(0, "quest")], "complete_quest": [(0, "quest")], "fail_quest": [(0, "quest")],
    "start_quest": [(0, "quest"), (1, "stage")], "stage": [(0, "quest"), (1, "stage")],
    "stage_seen": [(0, "quest"), (1, "stage")], "set_stage": [(0, "quest"), (1, "stage")],
    "in_map": [(0, "map")], "in_region": [(0, "region")], "teleport": [(0, "map")],
    "set_tile": [(0, "map"), (4, "tile")], "fill_tiles": [(0, "map"), (6, "tile")],
    "obj": [(1, "object")], "locked": [(0, "object")], "set_obj": [(0, "object")],
    "unlock": [(0, "object")], "lock": [(0, "object")], "remove_obj": [(0, "object")],
    "open_container": [(0, "object")],
    "start_dialogue": [(0, "dialogue"), (1, "char_or_empty"), (2, "node")],
    "end_game": [(0, "ending")], "sound": [(0, "sound")],
}

# flags the engine itself writes
ENGINE_FLAGS = {"thefts_seen", "morale_mod", "audit_authority"}


class Report:
    def __init__(self):
        self.errors: List[str] = []
        self.warnings: List[str] = []

    def error(self, where: str, message: str) -> None:
        self.errors.append("%s: %s" % (where, message))

    def warn(self, where: str, message: str) -> None:
        self.warnings.append("%s: %s" % (where, message))


class Validator:
    def __init__(self, content):
        from game.story.api import ScriptAPI
        from game.story.script import ScriptEngine
        self.c = content
        self.r = Report()
        self.engine = ScriptEngine()
        self.engine.register_object(ScriptAPI(None))
        self.maps = content.fresh_maps()
        self.objects: Dict[str, Any] = {}
        self.regions: Set[str] = set()
        self.flags_set: Set[str] = set()
        self.flags_get: Dict[str, str] = {}
        for map_id, gmap in self.maps.items():
            for obj in gmap.objects:
                if obj.id in self.objects:
                    self.r.error("maps/%s" % map_id, "object id %r is also used on map %r" % (
                        obj.id, self.objects[obj.id].map_id))
                obj.map_id = map_id
                self.objects[obj.id] = obj
            for region in gmap.regions:
                self.regions.add(region.id)
        from game.engine.audio import SOUNDS
        self.sounds = set(SOUNDS)

    # -- scripts -------------------------------------------------------------------
    def _exists(self, kind: str, value: str, extra: Optional[str] = None) -> bool:
        c = self.c
        if kind == "item":
            return value in c.items
        if kind in ("char", "char_or_all", "char_or_empty"):
            if kind == "char_or_all" and value == "all":
                return True
            if kind == "char_or_empty" and value == "":
                return True
            return value == "player" or value in c.characters
        if kind == "system":
            return value in (c.systems.get("systems") or {})
        if kind == "resource":
            return value in (c.systems.get("resources") or {})
        if kind == "quest":
            return value in c.quests
        if kind == "stage":
            return extra in c.quests and value in (c.quests[extra].get("stages") or {})
        if kind == "map":
            return value in self.maps
        if kind == "region":
            return value in self.regions
        if kind == "object":
            return value in self.objects
        if kind == "inventory":
            return value == "player" or value in c.characters or value in self.objects
        if kind == "dialogue":
            return value in c.dialogues
        if kind == "node":
            return extra in c.dialogues and value in (c.dialogues[extra].get("nodes") or {})
        if kind == "ending":
            return value in c.endings
        if kind == "goal":
            return value in c.goals
        if kind == "schedule":
            return value in c.schedules
        if kind == "effect":
            return value in c.effects
        if kind == "skill":
            return value in SKILLS
        if kind == "attr":
            return value in ATTRIBUTES
        if kind == "need":
            return value in NEEDS
        if kind == "tile":
            return value == "" or value in tiledefs.TILES
        if kind == "sound":
            return value in self.sounds
        return True

    def _check_refs(self, where: str, text: str, statement: bool) -> None:
        try:
            tree = ast.parse(str(text).strip(), mode="exec" if statement else "eval")
        except SyntaxError:
            return
        for node in ast.walk(tree):
            if not isinstance(node, ast.Call) or not isinstance(node.func, ast.Name):
                continue
            specs = REFS.get(node.func.id)
            if not specs:
                continue
            args: Dict[Any, ast.AST] = dict(enumerate(node.args))
            first = node.args[0].value if node.args and isinstance(node.args[0], ast.Constant) else None
            for index, kind in specs:
                arg = args.get(index)
                if not isinstance(arg, ast.Constant) or not isinstance(arg.value, str):
                    continue
                value = arg.value
                if kind == "flag_set":
                    self.flags_set.add(value)
                elif kind == "flag_get":
                    self.flags_get.setdefault(value, where)
                elif not self._exists(kind, value, first if kind in ("stage", "node") else None):
                    self.r.error(where, "%s(): unknown %s %r" % (node.func.id, kind.split("_")[0], value))
            if node.func.id == "add_goal":
                # template arguments may carry flag names (e.g. arrest(flag='dex_arrested'))
                for kw in node.keywords:
                    if isinstance(kw.value, ast.Constant) and isinstance(kw.value.value, str):
                        self.flags_set.add(kw.value.value)
            if node.func.id == "set_maintain":
                for arg in node.args[1:]:
                    if isinstance(arg, ast.Constant) and not self._exists("system", arg.value):
                        self.r.error(where, "set_maintain(): unknown system %r" % (arg.value,))

    def cond(self, where: str, text: Any) -> None:
        if text in (None, "") or isinstance(text, (bool, int, float)):
            return
        for problem in self.engine.check(text):
            self.r.error(where, "%s  [%s]" % (problem, text))
        self._check_refs(where, str(text), False)

    def effects(self, where: str, statements: Any) -> None:
        if not statements:
            return
        if isinstance(statements, str):
            statements = [statements]
        if not isinstance(statements, list):
            self.r.error(where, "script must be a string or a list of strings")
            return
        for text in statements:
            for problem in self.engine.check(text, statement=True):
                self.r.error(where, "%s  [%s]" % (problem, text))
            self._check_refs(where, str(text), True)

    def text(self, where: str, text: Any) -> None:
        if isinstance(text, list):
            for t in text:
                self.text(where, t)
            return
        if not isinstance(text, str):
            return
        for problem in self.engine.check_format(text):
            self.r.error(where, "%s  [%s]" % (problem, text[:60]))
        i = 0
        while True:
            i = text.find("{", i)
            if i == -1 or text.startswith("{{", i):
                break
            end = text.find("}", i)
            if end == -1:
                break
            self._check_refs(where, text[i + 1:end], False)
            i = end + 1

    # -- sections ------------------------------------------------------------------
    def check_items(self) -> None:
        for item_id, idef in self.c.items.items():
            where = "items.json/%s" % item_id
            if idef.icon not in icon_art.ICONS:
                self.r.error(where, "unknown icon %r" % idef.icon)
            if idef.color not in palette.CLOTH_COLORS:
                self.r.error(where, "unknown colour %r" % idef.color)
            if idef.kind not in KINDS:
                self.r.warn(where, "unusual kind %r" % idef.kind)
            if idef.slot and idef.slot not in SLOTS:
                self.r.error(where, "unknown equipment slot %r" % idef.slot)
            self.effects(where + "/use", idef.use)
        for key in ("part_item", "food_item"):
            value = self.c.game.get(key)
            if value and value not in self.c.items:
                self.r.error("game.json", "%s %r is not an item" % (key, value))

    def check_systems(self) -> None:
        resources = self.c.systems.get("resources") or {}
        for sid, sd in (self.c.systems.get("systems") or {}).items():
            where = "systems.json/%s" % sid
            if sd.get("map") and sd["map"] not in self.maps:
                self.r.error(where, "unknown map %r" % sd["map"])
            console = sd.get("console")
            if console:
                obj = self.objects.get(console)
                if obj is None:
                    self.r.error(where, "console object %r does not exist" % console)
                elif sd.get("map") and obj.map_id != sd["map"]:
                    self.r.warn(where, "console %r is on map %r, not %r" % (console, obj.map_id, sd["map"]))
            for name in list(sd.get("consumes") or {}) + list(sd.get("produces") or {}):
                if name not in resources:
                    self.r.error(where, "unknown resource %r" % name)
            fuel = sd.get("fuel")
            if fuel:
                if fuel.get("item") not in self.c.items:
                    self.r.error(where, "fuel item %r does not exist" % fuel.get("item"))
                if fuel.get("hopper") not in self.objects:
                    self.r.error(where, "fuel hopper %r does not exist" % fuel.get("hopper"))
            for recipe in sd.get("makes") or []:
                if recipe.get("item") not in self.c.items:
                    self.r.error(where, "makes unknown item %r" % recipe.get("item"))
                if recipe.get("into") not in self.objects:
                    self.r.error(where, "output container %r does not exist" % recipe.get("into"))
                for need in recipe.get("needs") or {}:
                    if need not in self.c.items:
                        self.r.error(where, "needs unknown item %r" % need)
                if recipe.get("needs") and recipe.get("from") not in self.objects:
                    self.r.error(where, "input container %r does not exist" % recipe.get("from"))

    def check_maps(self) -> None:
        c = self.c
        if not self.maps:
            self.r.error("maps", "no maps found")
            return
        for map_id, gmap in self.maps.items():
            where = "maps/%s" % map_id
            unknown = set()
            for layer in config.LAYERS:
                for row in gmap.layers[layer]:
                    for name in row:
                        if name and name not in tiledefs.TILES:
                            unknown.add(name)
            for name in sorted(unknown):
                self.r.error(where, "unknown tile %r" % name)
            for obj in gmap.objects:
                ow = "%s/%s" % (where, obj.id)
                if obj.type not in OBJECT_TYPES:
                    self.r.error(ow, "unknown object type %r" % obj.type)
                if not gmap.in_bounds(obj.x, obj.y):
                    self.r.error(ow, "is outside the map")
                if obj.type == "portal":
                    target = obj.get("target_map")
                    other = self.maps.get(target)
                    if other is None:
                        self.r.error(ow, "portal leads to unknown map %r" % target)
                    else:
                        tx, ty = int(obj.get("target_x", 0)), int(obj.get("target_y", 0))
                        if other.solid_at(tx, ty):
                            self.r.error(ow, "portal target %s (%d,%d) is not walkable" % (target, tx, ty))
                        elif other.object_at(tx, ty, "portal") is not None:
                            self.r.error(ow, "portal target lands on another portal")
                    if gmap.solid_at(obj.x, obj.y):
                        self.r.error(ow, "portal sits on a solid tile")
                if obj.type == "door" and gmap.solid_at(obj.x, obj.y):
                    self.r.error(ow, "door sits on a solid tile")
                if obj.type == "container":
                    for item_id in (obj.get("items") or {}):
                        if item_id not in c.items:
                            self.r.error(ow, "contains unknown item %r" % item_id)
                if obj.type == "item" and obj.get("item") not in c.items:
                    self.r.error(ow, "unknown item %r" % obj.get("item"))
                if obj.get("key") and obj.get("key") not in c.items:
                    self.r.error(ow, "unknown key item %r" % obj.get("key"))
                if obj.type == "console" and obj.get("system") not in (c.systems.get("systems") or {}):
                    self.r.error(ow, "console for unknown system %r" % obj.get("system"))
                if obj.get("dialogue") and obj.get("dialogue") not in c.dialogues:
                    self.r.error(ow, "unknown dialogue %r" % obj.get("dialogue"))
                owner = obj.get("owner")
                if obj.type == "bed" and owner and owner != "player" and owner not in c.characters:
                    self.r.warn(ow, "bed owner %r is not a character" % owner)
                sprite = obj.get("sprite")
                if sprite and sprite not in tiledefs.TILES:
                    self.r.error(ow, "unknown sprite tile %r" % sprite)
                self.cond(ow + "/cond", obj.get("cond"))
                self.effects(ow + "/on_interact", obj.get("on_interact"))
                self.effects(ow + "/on_enter", obj.get("on_enter"))
                self.text(ow + "/text", obj.get("text"))
        start = c.game.get("start") or {}
        start_map = start.get("map")
        if start_map not in self.maps:
            self.r.error("game.json", "start map %r does not exist" % start_map)
            return
        seen = {start_map}
        queue = deque([start_map])
        while queue:
            cur = queue.popleft()
            for obj in self.maps[cur].objects_of("portal"):
                nxt = obj.get("target_map")
                if nxt in self.maps and nxt not in seen:
                    seen.add(nxt)
                    queue.append(nxt)
        for map_id in self.maps:
            if map_id not in seen:
                self.r.warn("maps/%s" % map_id, "cannot be reached from the start map")
        for key in ("medbay_bed",):
            if c.game.get(key) and c.game[key] not in self.objects:
                self.r.error("game.json", "%s %r is not a map object" % (key, c.game[key]))
        if c.game.get("mess_region") and c.game["mess_region"] not in self.regions:
            self.r.warn("game.json", "mess_region %r is not a region on any map" % c.game["mess_region"])

    def check_goal(self, where: str, spec: dict) -> None:
        c = self.c
        if "use" in spec:
            if spec["use"] not in c.goals:
                self.r.error(where, "unknown goal template %r" % spec["use"])
                return
            template = c.goals[spec["use"]]
            steps = ai._substitute(template.get("steps") or [], spec.get("args") or {})
        else:
            steps = spec.get("steps") or []
            if not steps:
                self.r.error(where, "goal has no steps")
        self.cond(where + "/when", spec.get("when"))
        self.effects(where + "/on_complete", spec.get("on_complete"))
        self.effects(where + "/on_fail", spec.get("on_fail"))
        systems = c.systems.get("systems") or {}
        for i, step in enumerate(steps):
            sw = "%s/step %d" % (where, i)
            verb = step.get("do")
            if verb not in ai.HANDLERS:
                self.r.error(sw, "unknown step verb %r" % verb)
            for key in ("item",):
                v = step.get(key)
                if isinstance(v, str) and not v.startswith("$") and v not in c.items:
                    self.r.error(sw, "unknown item %r" % v)
            v = step.get("system")
            if isinstance(v, str) and not v.startswith("$") and v not in systems:
                self.r.error(sw, "unknown system %r" % v)
            for key in ("object", "to"):
                v = step.get(key)
                if isinstance(v, str) and not v.startswith("$") and v not in self.objects:
                    self.r.error(sw, "unknown object %r" % v)
            for v in list(step.get("from") or []) + list(step.get("exclude") or []):
                if isinstance(v, str) and not v.startswith("$") and v not in self.objects:
                    self.r.error(sw, "unknown container %r" % v)
            v = step.get("char")
            if isinstance(v, str) and not v.startswith("$") and v != "player" and v not in c.characters:
                self.r.error(sw, "unknown character %r" % v)
            v = step.get("region")
            if isinstance(v, str) and v and not v.startswith("$") and v not in self.regions:
                self.r.error(sw, "unknown region %r" % v)
            if isinstance(step.get("until"), str) and not step["until"].startswith("$"):
                self.cond(sw + "/until", step.get("until"))
            self.cond(sw + "/if", step.get("if"))
            if isinstance(step.get("var"), str):
                self.flags_set.add(step["var"])
            script = step.get("script") or step.get("effects")
            if script and not any("$" in str(s) for s in (script if isinstance(script, list) else [script])):
                self.effects(sw + "/script", script)

    def check_schedules(self) -> None:
        for name, blocks in self.c.schedules.items():
            where = "schedules.json/%s" % name
            if not isinstance(blocks, list):
                self.r.error(where, "must be a list of blocks")
                continue
            covered = [False] * 24
            for block in blocks:
                if block.get("do") not in ("work", "meal", "leisure", "sleep", "patrol", "idle"):
                    self.r.error(where, "unknown activity %r" % block.get("do"))
                region = block.get("region")
                if region and region not in self.regions:
                    self.r.error(where, "unknown region %r" % region)
                frm, to = int(block.get("from", 0)), int(block.get("to", 24))
                h = frm
                for _ in range(24):
                    if h % 24 == to % 24 and h != frm:
                        break
                    covered[h % 24] = True
                    h += 1
                    if frm < to and h >= to:
                        break
            gaps = [h for h in range(24) if not covered[h]]
            if gaps:
                self.r.warn(where, "no activity for hours %s" % gaps)

    def check_characters(self) -> None:
        c = self.c
        systems = c.systems.get("systems") or {}
        for cid, cd in c.characters.items():
            where = "characters/%s" % cid
            if cid == "player":
                self.r.error(where, "'player' is reserved for the player character")
            spawn = cd.get("spawn") or {}
            gmap = self.maps.get(spawn.get("map"))
            if gmap is None:
                self.r.error(where, "spawn map %r does not exist (the character will not appear)" % spawn.get("map"))
            elif gmap.solid_at(int(spawn.get("x", 0)), int(spawn.get("y", 0))):
                self.r.error(where, "spawn position (%s,%s) on %s is not walkable" % (
                    spawn.get("x"), spawn.get("y"), gmap.id))
            app = cd.get("appearance") or {}
            norm = chargen.normalize(app)
            for key, value in app.items():
                if key == "extras":
                    for extra in value or []:
                        if extra not in chargen.EXTRAS:
                            self.r.warn(where, "unknown appearance extra %r" % extra)
                elif key in norm and norm[key] != value:
                    self.r.warn(where, "appearance %s=%r is not valid; %r will be used" % (key, value, norm[key]))
            for a in cd.get("attrs") or {}:
                if a not in ATTRIBUTES:
                    self.r.error(where, "unknown attribute %r" % a)
            for s in cd.get("skills") or {}:
                if s not in SKILLS:
                    self.r.error(where, "unknown skill %r" % s)
            for item_id in cd.get("inventory") or {}:
                if item_id not in c.items:
                    self.r.error(where, "inventory has unknown item %r" % item_id)
            for slot, item_id in (cd.get("equipped") or {}).items():
                idef = c.items.get(item_id)
                if idef is None:
                    self.r.error(where, "equips unknown item %r" % item_id)
                elif idef.slot != slot:
                    self.r.error(where, "item %r does not go in slot %r" % (item_id, slot))
            bed = (cd.get("home") or {}).get("bed")
            if bed and bed not in self.objects:
                self.r.error(where, "home bed %r does not exist" % bed)
            job = cd.get("job") or {}
            for sid in [job.get("system")] + list(job.get("maintain") or []) + list(job.get("restock") or []):
                if sid and sid not in systems:
                    self.r.error(where, "job refers to unknown system %r" % sid)
            if job.get("region") and job["region"] not in self.regions:
                self.r.error(where, "job region %r does not exist" % job["region"])
            for marker in job.get("markers") or []:
                if marker not in self.objects:
                    self.r.error(where, "job marker %r does not exist" % marker)
            sched = cd.get("schedule", "day")
            if isinstance(sched, str) and sched not in c.schedules:
                self.r.error(where, "unknown schedule %r" % sched)
            dlg = cd.get("dialogue")
            if dlg and dlg not in c.dialogues:
                self.r.error(where, "unknown dialogue %r" % dlg)
            elif not dlg and cid not in c.dialogues and "generic" not in c.dialogues:
                self.r.warn(where, "has no dialogue and there is no generic.json fallback")
            for other in cd.get("relations") or {}:
                if other != "player" and other not in c.characters:
                    self.r.warn(where, "relation to unknown character %r" % other)
            for stock in (cd.get("trade") or {}).get("stock") or []:
                if stock not in self.objects:
                    self.r.error(where, "trade stock container %r does not exist" % stock)
            for i, spec in enumerate(cd.get("goals") or []):
                self.check_goal("%s/goal %s" % (where, spec.get("id", i)), spec)
        pdef = c.game.get("player") or {}
        for item_id in pdef.get("inventory") or {}:
            if item_id not in c.items:
                self.r.error("game.json/player", "unknown item %r" % item_id)
        for bg in c.backgrounds():
            bw = "game.json/backgrounds/%s" % bg.get("id")
            for a in bg.get("attrs") or {}:
                if a not in ATTRIBUTES:
                    self.r.error(bw, "unknown attribute %r" % a)
            for s in bg.get("skills") or {}:
                if s not in SKILLS:
                    self.r.error(bw, "unknown skill %r" % s)
            for item_id in bg.get("items") or {}:
                if item_id not in c.items:
                    self.r.error(bw, "unknown item %r" % item_id)

    def check_dialogues(self) -> None:
        c = self.c
        for did, data in c.dialogues.items():
            where = "dialogue/%s" % did
            nodes = data.get("nodes") or {}
            if not nodes:
                self.r.error(where, "has no nodes")
                continue

            def target(node_where: str, value: Any) -> None:
                if value in (None, "end"):
                    return
                if value not in nodes:
                    self.r.error(node_where, "goes to missing node %r" % (value,))
                else:
                    reached.add(value)

            reached: Set[str] = set()
            start = data.get("start", "start")
            if isinstance(start, str):
                target(where + "/start", start)
            else:
                for option in start:
                    self.cond(where + "/start", option.get("if"))
                    target(where + "/start", option.get("goto"))
                if start and start[-1].get("if"):
                    self.r.warn(where, "the last 'start' entry has a condition; add an unconditional fallback")
            for nid, node in nodes.items():
                nw = "%s/%s" % (where, nid)
                speaker = node.get("speaker", "npc")
                if speaker not in ("npc", "player", "narrator") and speaker not in c.characters \
                        and not node.get("speaker_name"):
                    self.r.error(nw, "unknown speaker %r (add speaker_name for a voice with no character)" % speaker)
                self.text(nw, node.get("text"))
                for variant in node.get("variants") or []:
                    self.cond(nw + "/variants", variant.get("if"))
                    self.text(nw + "/variants", variant.get("text"))
                self.effects(nw + "/do", node.get("do"))
                for option in node.get("branch") or []:
                    self.cond(nw + "/branch", option.get("if"))
                    target(nw + "/branch", option.get("goto"))
                target(nw + "/next", node.get("next"))
                for i, choice in enumerate(node.get("choices") or []):
                    cw = "%s/choice %d" % (nw, i)
                    if not choice.get("text"):
                        self.r.error(cw, "has no text")
                    self.text(cw, choice.get("text"))
                    self.cond(cw + "/if", choice.get("if"))
                    for key in ("do", "pass_do", "fail_do"):
                        self.effects("%s/%s" % (cw, key), choice.get(key))
                    for key in ("goto", "pass", "fail"):
                        target(cw, choice.get(key))
                    if "skill" in choice and choice["skill"][0] not in SKILLS:
                        self.r.error(cw, "unknown skill %r" % choice["skill"][0])
                    if "attr" in choice and choice["attr"][0] not in ATTRIBUTES:
                        self.r.error(cw, "unknown attribute %r" % choice["attr"][0])
                    if "check" in choice:
                        if choice["check"][0] not in SKILLS:
                            self.r.error(cw, "unknown skill %r" % choice["check"][0])
                        if "pass" not in choice and "fail" not in choice:
                            self.r.warn(cw, "skill check without 'pass' or 'fail' targets")
                    if "item" in choice and choice["item"][0] not in c.items:
                        self.r.error(cw, "unknown item %r" % choice["item"][0])
                if not node.get("text") and not node.get("variants") and not node.get("branch") \
                        and not node.get("do") and not node.get("next") and not node.get("choices"):
                    self.r.warn(nw, "node is empty")
            # nodes only reachable through scripts (start_dialogue / talk steps) are fine,
            # so unreached nodes are a warning rather than an error
            for nid in nodes:
                if nid not in reached:
                    self.flags_get.setdefault("__node__%s/%s" % (did, nid), where)

    def check_quests(self) -> None:
        for qid, qd in self.c.quests.items():
            if not qd.get("stages"):
                self.r.warn("quests.json/%s" % qid, "has no stages")

    def check_rules(self) -> None:
        seen = set()
        for rule in self.c.rules:
            where = "story rule %s" % rule.get("id")
            if rule.get("id") in seen:
                self.r.error(where, "duplicate rule id")
            seen.add(rule.get("id"))
            self.cond(where + "/if", rule.get("if"))
            self.effects(where + "/do", rule.get("do"))
            if not rule.get("do"):
                self.r.warn(where, "has no effects")
        for eid, ending in self.c.endings.items():
            for i, slide in enumerate(ending.get("slides") or []):
                where = "endings.json/%s/slide %d" % (eid, i)
                self.cond(where, slide.get("if"))
                self.text(where, slide.get("text"))
        intro = self.c.game.get("intro")
        if intro and intro not in self.c.dialogues:
            self.r.error("game.json", "intro dialogue %r does not exist" % intro)

    def check_goal_templates(self) -> None:
        for gid, template in self.c.goals.items():
            where = "goals.json/%s" % gid
            for i, step in enumerate(template.get("steps") or []):
                if step.get("do") not in ai.HANDLERS:
                    self.r.error(where, "step %d: unknown verb %r" % (i, step.get("do")))
            if not template.get("steps"):
                self.r.error(where, "template has no steps")

    def check_flags(self) -> None:
        script_nodes = set()
        # nodes named in talk steps / add_goal(node=...) cannot be seen statically, so
        # just collect every string that appears in goal templates and rules
        blob = repr(self.c.rules) + repr(self.c.goals) + repr(self.c.characters) + repr(
            [d for d in self.c.dialogues.values()])
        for name, where in sorted(self.flags_get.items()):
            if name.startswith("__node__"):
                did, nid = name[len("__node__"):].split("/", 1)
                if ("'%s'" % nid) not in blob and ('"%s"' % nid) not in blob:
                    self.r.warn("dialogue/%s/%s" % (did, nid), "node is never reached")
                else:
                    script_nodes.add(nid)
                continue
            if name in self.flags_set or name in ENGINE_FLAGS or name.startswith(("_", "talked:")):
                continue
            self.r.warn(where, "flag %r is read but nothing ever sets it" % name)

    def run(self) -> Report:
        for problem in self.c.problems:
            self.r.error("load", problem)
        self.check_items()
        self.check_systems()
        self.check_maps()
        self.check_schedules()
        self.check_goal_templates()
        self.check_characters()
        self.check_dialogues()
        self.check_quests()
        self.check_rules()
        self.check_flags()
        return self.r


def validate(content) -> Report:
    """Validate a loaded :class:`~game.content.Content`; returns a Report."""
    return Validator(content).run()


def print_api() -> None:
    from game.story.api import ScriptAPI
    from game.story.script import ScriptEngine
    import inspect
    engine = ScriptEngine()
    api = ScriptAPI(None)
    engine.register_object(api)
    groups: Dict[str, List[str]] = {"cond": [], "text": [], "effect": []}
    for name in sorted(engine.funcs):
        fn = engine.funcs[name]
        kind = engine.kinds.get(name, "effect")
        if not hasattr(fn, "__script_kind__"):
            continue
        sig = str(inspect.signature(fn))
        doc = " ".join((fn.__doc__ or "").split())
        groups.setdefault(kind, []).append("  %s%s\n      %s" % (name, sig, doc))
    titles = {"cond": "CONDITIONS / GETTERS (use in 'if')", "text": "TEXT (use inside {braces})",
              "effect": "EFFECTS (use in 'do')"}
    for kind in ("cond", "text", "effect"):
        print(titles[kind])
        print("\n".join(groups[kind]))
        print()
    print("Also available: min, max, abs, round, int, float, str, len, bool, and/or/not, "
          "comparisons, arithmetic, 'x if cond else y'.")
    print("Context names: npc, self, obj, item, map, player.")


def main(argv=None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if "--api" in argv:
        print_api()
        return 0
    data_dir = None
    if "--data" in argv:
        data_dir = argv[argv.index("--data") + 1]
    from game.content import Content
    content = Content.load(data_dir)
    report = validate(content)
    for line in report.errors:
        print("ERROR  ", line)
    for line in report.warnings:
        print("warning", line)
    print()
    print("%d map(s), %d character(s), %d dialogue(s), %d item(s), %d quest(s), %d rule(s)" % (
        len(content.map_ids), len(content.characters), len(content.dialogues), len(content.items),
        len(content.quests), len(content.rules)))
    print("%d error(s), %d warning(s)" % (len(report.errors), len(report.warnings)))
    return 1 if report.errors else 0


if __name__ == "__main__":
    sys.exit(main())
