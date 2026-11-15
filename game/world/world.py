"""The World: one object holding the entire simulated game state.

Maps, characters, station systems, the clock, story flags, quests and the
event bus all live here.  ``World.update(dt)`` advances *everything* - every
map, every NPC, every system - regardless of where the player is looking,
and ``World.to_dict`` / ``World.from_dict`` capture all of it for saving.

The class deliberately knows nothing about rendering or input.  Scenes read
it to draw the screen, and collect what the simulation wants shown through
two small queues: ``toasts`` (notifications) and ``requests`` (open a
dialogue, a trade window, the ending ...).
"""
from __future__ import annotations

import random
from typing import Any, Dict, Iterable, List, Optional, Tuple

from .. import config
from ..sim import ai, needs
from ..sim.events import EventBus
from ..sim.systems import SystemNetwork
from ..story.api import ScriptAPI
from ..story.director import Director
from ..story.quests import QuestLog
from ..story.script import ScriptEngine
from .character import Character
from .clock import GameClock
from .gamemap import GameMap, MapObject
from .items import Inventory
from .pathfinding import Navigator

SLOW_TICK = 0.5          # real seconds between systems / needs updates
LOG_LIMIT = 250


class World:
    def __init__(self, content, seed: Optional[int] = None):
        self.content = content
        self.rng = random.Random(seed)
        game = content.game
        start = game.get("start") or {}
        self.clock = GameClock(
            GameClock.to_minutes(int(start.get("day", 1)), float(start.get("hour", 8)), float(start.get("minute", 0))),
            float(game.get("time_scale", config.TIME_SCALE)))
        self.maps: Dict[str, GameMap] = content.fresh_maps()
        self.characters: Dict[str, Character] = {}
        self.player: Optional[Character] = None
        self.systems = SystemNetwork(content.systems)
        self.flags: Dict[str, Any] = {}
        self.quests = QuestLog(content.quests)
        self.events = EventBus()
        self.script = ScriptEngine(self)
        self.api = ScriptAPI(self)
        self.script.register_object(self.api)
        self.director = Director(self, content.rules)
        self.nav = Navigator(self)
        self.log: List[Dict[str, Any]] = []
        self.toasts: List[Tuple[str, str]] = []
        self.requests: List[Dict[str, Any]] = []
        self.ended: Optional[str] = None
        self.busy = False               # set by the UI while a dialogue/menu is open
        self.playtime = 0.0
        self.added_objects: Dict[str, List[dict]] = {}
        self.removed_objects: Dict[str, List[str]] = {}
        self.tile_changes: Dict[str, List[list]] = {}      # tiles altered by scripts
        self._slow = 0.0
        self._marks = {"tick": -1, "hour": -1, "day": -1}
        self._obj_index: Optional[Dict[str, MapObject]] = None
        self._containers: Optional[List[MapObject]] = None
        self.events.on("system_status", self._on_system_status)
        self.events.on("resource_level", self._on_resource_level)
        self._sync_marks()

    # ------------------------------------------------------------------
    # creation
    # ------------------------------------------------------------------
    @classmethod
    def new_game(cls, content, setup: Optional[dict] = None, seed: Optional[int] = None) -> "World":
        """Start a fresh game.  ``setup`` may carry the player's "name",
        "background" id and "appearance"."""
        setup = setup or {}
        world = cls(content, seed)
        game = content.game
        pdef = dict(game.get("player") or {})
        player = Character.from_def("player", pdef, content)
        player.is_player = True
        player.name = str(setup.get("name") or pdef.get("name") or "Auditor")
        player.speed = config.WALK_SPEED
        if setup.get("appearance"):
            player.appearance = dict(setup["appearance"])
        background = None
        for bg in content.backgrounds():
            if bg.get("id") == setup.get("background"):
                background = bg
        if background is None and content.backgrounds():
            background = content.backgrounds()[0]
        if background:
            player.background = str(background.get("id", ""))
            for k, v in (background.get("attrs") or {}).items():
                player.attrs[k] = player.attrs.get(k, 4) + int(v)
            for k, v in (background.get("skills") or {}).items():
                player.skills[k] = player.skills.get(k, 0) + int(v)
            for k, v in (background.get("items") or {}).items():
                player.inventory.add(k, int(v))
            player.credits += int(background.get("credits", 0))
            for slot, item_id in (background.get("equipped") or {}).items():
                if player.inventory.has(item_id):
                    player.equipped[slot] = item_id
        player.invalidate()
        player.hp = float(player.max_hp)
        player.met = True
        start = game.get("start") or {}
        map_id = str(start.get("map") or (content.map_ids[0] if content.map_ids else ""))
        x, y, facing = int(start.get("x", 1)), int(start.get("y", 1)), str(start.get("facing", "down"))
        gmap = world.maps.get(map_id)
        if gmap is not None:
            for obj in gmap.objects_of("spawn"):
                if obj.get("name", "player_start") == start.get("spawn", "player_start"):
                    x, y, facing = obj.x, obj.y, str(obj.get("facing", facing))
                    break
        player.place(map_id, x, y, facing)
        world.add_character(player)
        for cid, cdef in content.characters.items():
            world.spawn_from_def(cid, cdef)
        world.systems.tick(world, 1e-6)       # prime efficiency / power figures
        world.flags["_region"] = ""
        world.events.emit("start")
        world.events.pump()
        return world

    def spawn_from_def(self, cid: str, cdef: dict) -> Optional[Character]:
        if cid in self.characters or cid == "player":
            return None
        ch = Character.from_def(cid, cdef, self.content)
        if not ch.dialogue:
            ch.dialogue = cid if cid in self.content.dialogues else "generic"
        if ch.map_id not in self.maps:
            return None
        for spec in cdef.get("goals") or []:
            goal = ai.instantiate(self, spec, "authored")
            if goal is not None:
                ch.goals.append(goal)
        ch.ai["think"] = self.rng.uniform(0.0, ai.THINK_INTERVAL)
        self.add_character(ch)
        return ch

    def add_character(self, ch: Character) -> None:
        ch.content = self.content
        self.characters[ch.id] = ch
        if ch.is_player:
            self.player = ch

    def remove_character(self, ch: Character, reason: str = "left") -> None:
        """The character leaves the station for good."""
        if not ch.present:
            return
        ch.present = False
        ch.path = []
        ch.asleep = False
        self.events.emit("left:" + ch.id, npc=ch.id, reason=reason)
        self.add_log("%s has left the station." % ch.name, "crew")

    # ------------------------------------------------------------------
    # time
    # ------------------------------------------------------------------
    def minutes(self, dt: float) -> float:
        """Game minutes in ``dt`` real seconds."""
        return dt * self.clock.time_scale

    def hours(self, dt: float) -> float:
        return dt * self.clock.time_scale / 60.0

    def update(self, dt: float) -> None:
        """Advance the simulation by ``dt`` real seconds."""
        self.playtime += dt
        while dt > 1e-9 and not self.ended:
            step = min(dt, config.MAX_SIM_STEP)
            self.tick(step)
            dt -= step

    def tick(self, dt: float) -> None:
        if self.ended:
            return
        self.clock.advance(self.minutes(dt))
        self._slow += dt
        if self._slow >= SLOW_TICK:
            hours = self.hours(self._slow)
            self._slow = 0.0
            self.systems.tick(self, hours)
            for ch in list(self.characters.values()):
                needs.update_character(self, ch, hours)
        for ch in list(self.characters.values()):
            if not ch.is_player:
                ai.update_npc(self, ch, dt)
        self._periodic()
        self.director.update()
        self.events.pump()

    def fast_forward(self, game_minutes: float, step: float = config.MAX_SIM_STEP) -> None:
        """Run the full simulation for a stretch of game time (sleeping,
        waiting, time passing in a script) without rendering."""
        seconds = max(0.0, game_minutes) / max(0.001, self.clock.time_scale)
        while seconds > 1e-9 and not self.ended:
            dt = min(step, seconds)
            self.tick(dt)
            seconds -= dt

    def _sync_marks(self) -> None:
        m = self.clock.minutes
        self._marks = {"tick": int(m // 10), "hour": int(m // 60), "day": self.clock.day}

    def _periodic(self) -> None:
        m = self.clock.minutes
        marks = self._marks
        t10 = int(m // 10)
        if t10 != marks["tick"]:
            marks["tick"] = t10
            self.events.emit("tick")
        hour = int(m // 60)
        if hour != marks["hour"]:
            marks["hour"] = hour
            self.events.emit("hour", hour=self.clock.hour, day=self.clock.day)
        if self.clock.day != marks["day"]:
            marks["day"] = self.clock.day
            self.events.emit("day", day=self.clock.day)

    # ------------------------------------------------------------------
    # characters
    # ------------------------------------------------------------------
    def npcs(self) -> List[Character]:
        return [c for c in self.characters.values() if not c.is_player and c.present and c.alive]

    def chars_in(self, map_id: str) -> List[Character]:
        return [c for c in self.characters.values() if c.present and c.map_id == map_id]

    def char_at(self, map_id: str, x: int, y: int, exclude: Optional[Character] = None,
                include_asleep: bool = False) -> Optional[Character]:
        for c in self.characters.values():
            if c.x == x and c.y == y and c.map_id == map_id and c.present and c is not exclude:
                if c.asleep and not include_asleep:
                    continue
                return c
        return None

    def crew_count(self) -> int:
        return sum(1 for c in self.characters.values()
                   if c.present and c.alive and not needs.is_synthetic(c))

    def transfer(self, char: Character, map_id: str, x: int, y: int, facing: Optional[str] = None,
                 via: Optional[str] = None) -> None:
        """Move a character to another place instantly (portals, teleports)."""
        if map_id not in self.maps:
            return
        old = char.map_id
        char.place(map_id, x, y, facing)
        char._path_key = None
        if old != map_id:
            self.events.emit("map_changed", char=char.id, from_map=old, to_map=map_id, via=via or "")
            if char.is_player:
                self.flags["_region"] = ""
                self.events.emit("enter_map:" + map_id, map=map_id)
                region = self.maps[map_id].region_at(x, y)
                if region is not None:
                    self.flags["_region"] = region.id
                    self.events.emit("enter_region:" + region.id, region=region.id, map=map_id)

    def collapse(self, ch: Character) -> None:
        """Somebody ran out of health."""
        ch.hp = max(1.0, ch.max_hp * 0.25)
        if ch.is_player:
            self.request("collapse")
            return
        bed = self.find_object(str(self.content.game.get("medbay_bed", "")))
        if bed is not None:
            cells = self.nav.approach_cells(ch, bed.map_id, bed.x, bed.y)
            if cells:
                self.transfer(ch, bed.map_id, cells[0][0], cells[0][1])
            ch.ai["bed"] = bed.id
        ch.asleep = True
        ch.path = []
        ch.ai["goal"] = ""
        ch.ai["down_until"] = self.clock.minutes + 360
        ch.needs["hunger"] = min(ch.needs.get("hunger", 0), 60.0)
        ch.needs["fatigue"] = min(ch.needs.get("fatigue", 0), 50.0)
        self.events.emit("collapse:" + ch.id, npc=ch.id)
        self.announce("Medical alert: %s has collapsed." % ch.name)

    # ------------------------------------------------------------------
    # objects, regions, containers
    # ------------------------------------------------------------------
    def reindex(self) -> None:
        self._obj_index = None
        self._containers = None
        self.nav.invalidate()

    def _index(self) -> Dict[str, MapObject]:
        if self._obj_index is None:
            index: Dict[str, MapObject] = {}
            for gmap in self.maps.values():
                for obj in gmap.objects:
                    obj.map_id = gmap.id
                    index.setdefault(obj.id, obj)
            self._obj_index = index
        return self._obj_index

    def find_object(self, obj_id: str) -> Optional[MapObject]:
        if not obj_id:
            return None
        return self._index().get(obj_id)

    def find_marker(self, name: str) -> Optional[MapObject]:
        obj = self.find_object(name)
        if obj is not None:
            return obj
        for gmap in self.maps.values():
            for o in gmap.objects:
                if o.type == "marker" and o.get("name") == name:
                    return o
        return None

    def find_region(self, region_id: str, map_id: Optional[str] = None):
        """``(map_id, Region)`` for a region id, searching every map."""
        if not region_id:
            return None
        if map_id and map_id in self.maps:
            region = self.maps[map_id].find_region(region_id)
            if region is not None:
                return map_id, region
        for mid, gmap in self.maps.items():
            region = gmap.find_region(region_id)
            if region is not None:
                return mid, region
        return None

    def containers(self) -> List[MapObject]:
        if self._containers is None:
            self._containers = [o for g in self.maps.values() for o in g.objects if o.type == "container"]
        return self._containers

    def inventory_of(self, obj: Optional[MapObject]) -> Optional[Inventory]:
        if obj is None:
            return None
        if obj.inventory is None:
            obj.inventory = Inventory(obj.get("items") or {})
        return obj.inventory

    def resolve_inventory(self, ref: str) -> Optional[Inventory]:
        """Inventory of "player", a character id or a container object id."""
        if ref in ("player", ""):
            return self.player.inventory if self.player else None
        ch = self.characters.get(ref)
        if ch is not None:
            return ch.inventory
        return self.inventory_of(self.find_object(ref))

    def stock(self, item_id: str, include_characters: bool = False) -> int:
        total = sum(self.inventory_of(o).count(item_id) for o in self.containers())
        if include_characters:
            total += sum(c.inventory.count(item_id) for c in self.characters.values() if c.present)
        return total

    def random_free_cell(self, char: Character, map_id: str, region=None,
                         near: Optional[Tuple[int, int]] = None, radius: int = 4) -> Optional[Tuple[int, int]]:
        gmap = self.maps.get(map_id)
        if gmap is None:
            return None
        passable = self.nav.passable_fn(char, map_id)
        rng = self.rng
        for _ in range(16):
            if near is not None:
                x = near[0] + rng.randint(-radius, radius)
                y = near[1] + rng.randint(-radius, radius)
                if region is not None and not region.contains(x, y):
                    continue
            elif region is not None:
                x = rng.randrange(region.x, region.x + region.w)
                y = rng.randrange(region.y, region.y + region.h)
            else:
                x, y = rng.randrange(gmap.width), rng.randrange(gmap.height)
            if passable(x, y) and self.char_at(map_id, x, y, exclude=char) is None:
                return (x, y)
        if region is not None and near is None:
            return self.nav.nearest_free(char, map_id, region.center[0], region.center[1], radius=8)
        return None

    def add_object(self, map_id: str, type: str, x: int, y: int, **props) -> Optional[MapObject]:
        gmap = self.maps.get(map_id)
        if gmap is None:
            return None
        obj = gmap.add_object(type, x, y, **props)
        self.added_objects.setdefault(map_id, []).append(obj.to_dict())
        self.reindex()
        return obj

    def remove_object(self, obj: MapObject) -> None:
        gmap = self.maps.get(obj.map_id)
        if gmap is None:
            return
        gmap.remove_object(obj)
        added = self.added_objects.get(obj.map_id, [])
        for entry in added:
            if entry.get("id") == obj.id:
                added.remove(entry)
                break
        else:
            self.removed_objects.setdefault(obj.map_id, []).append(obj.id)
        self.reindex()

    def set_tile(self, map_id: str, layer: str, x: int, y: int, tile: str) -> None:
        """Change a map tile during play (remembered in save files)."""
        gmap = self.maps.get(map_id)
        if gmap is None or layer not in gmap.layers or not gmap.in_bounds(x, y):
            return
        gmap.set_tile(layer, x, y, tile)
        self.tile_changes.setdefault(map_id, []).append([layer, int(x), int(y), tile])

    # ------------------------------------------------------------------
    # messages
    # ------------------------------------------------------------------
    def notify(self, text: str, kind: str = "info") -> None:
        self.toasts.append((text, kind))
        if len(self.toasts) > 30:
            del self.toasts[:10]

    def add_log(self, text: str, kind: str = "info") -> None:
        self.log.append({"t": self.clock.minutes, "text": text, "kind": kind})
        if len(self.log) > LOG_LIMIT:
            del self.log[:LOG_LIMIT // 5]

    def announce(self, text: str) -> None:
        """Station-wide announcement: logged and shown to the player."""
        self.add_log(text, "announce")
        self.notify(text, "warn")
        self.events.emit("announce", text=text)

    def request(self, type: str, **data: Any) -> None:
        self.requests.append(dict(data, type=type))

    def text(self, text: str, ctx: Optional[dict] = None) -> str:
        """Expand ``$PLAYER`` / ``$NPC`` / ``$TIME`` and ``{script}`` in text."""
        if "$" in text:
            npc = self.characters.get((ctx or {}).get("npc", "")) if ctx else None
            text = (text.replace("$PLAYER", self.player.name if self.player else "")
                        .replace("$NPC", npc.name if npc else "")
                        .replace("$TIME", self.clock.time_str())
                        .replace("$DAY", str(self.clock.day)))
        return self.script.format(text, ctx)

    def item_name(self, item_id: str) -> str:
        idef = self.content.items.get(item_id)
        return idef.name if idef else str(item_id)

    def object_name(self, obj_id: str) -> str:
        obj = self.find_object(obj_id)
        return obj.name if obj else ""

    def _on_system_status(self, name: str, data: Dict[str, Any]) -> None:
        system = self.systems.get(data.get("system", ""))
        if system is None:
            return
        status, old = data.get("status"), data.get("old")
        worse = ["ok", "degraded", "critical", "offline"]
        if worse.index(status) > worse.index(old):
            label = {"degraded": "is degrading", "critical": "is CRITICAL", "offline": "has gone OFFLINE"}[status]
            self.announce("Station alert: %s %s." % (system.name, label))
        elif status == "ok":
            self.announce("%s is back to nominal." % system.name)

    def _on_resource_level(self, name: str, data: Dict[str, Any]) -> None:
        rdef = self.systems.resource_defs.get(data.get("resource", ""), {})
        label = rdef.get("name", data.get("resource"))
        level = data.get("level")
        if level == "low" and data.get("old") == "ok":
            self.announce("Station alert: %s is running low." % label)
        elif level == "critical":
            self.announce("Station alert: %s is CRITICAL." % label)

    # ------------------------------------------------------------------
    # items and checks
    # ------------------------------------------------------------------
    def give_item(self, char: Character, item_id: str, qty: int = 1, quiet: bool = False) -> None:
        if qty <= 0 or item_id not in self.content.items:
            return
        char.inventory.add(item_id, qty)
        if char.is_player:
            if not quiet:
                self.notify("+ %s%s" % (self.item_name(item_id), " x%d" % qty if qty > 1 else ""), "good")
            self.events.emit("item:" + item_id, item=item_id, count=qty)

    def take_item(self, char: Character, item_id: str, qty: int = 1, quiet: bool = False) -> int:
        taken = char.inventory.remove(item_id, qty)
        char.check_equipment()
        if taken and char.is_player and not quiet:
            self.notify("- %s%s" % (self.item_name(item_id), " x%d" % taken if taken > 1 else ""), "info")
        return taken

    def use_item(self, char: Character, item_id: str) -> bool:
        """Apply an item's ``use`` script to ``char`` and consume it."""
        idef = self.content.items.get(item_id)
        if idef is None or not idef.use or not char.inventory.has(item_id):
            return False
        self.script.run(idef.use, {"user": char.id, "item": item_id})
        if idef.kind == "consumable":
            char.inventory.remove(item_id, 1)
            char.check_equipment()
        if char.is_player and idef.use_text:
            self.notify(idef.use_text, "info")
        self.events.emit("used:" + item_id, char=char.id, item=item_id)
        return True

    def check_chance(self, char: Character, skill: str, dc: int) -> int:
        """Percent chance that ``d20 + skill >= dc`` (1 always fails, 20 always succeeds)."""
        return int(max(5, min(95, (21 + char.skill(skill) - dc) * 5)))

    def skill_check(self, char: Character, skill: str, dc: int) -> bool:
        roll = self.rng.randint(1, 20)
        success = roll == 20 or (roll != 1 and roll + char.skill(skill) >= dc)
        self.events.emit("check", char=char.id, skill=skill, dc=dc, roll=roll, success=success)
        if char.is_player:
            self.add_log("%s check (difficulty %d): rolled %d + %d - %s." % (
                skill.capitalize(), dc, roll, char.skill(skill), "success" if success else "failure"), "check")
        return success

    def player_repair(self, system_id: str) -> Tuple[bool, str]:
        """The player fits one spare part to a system (takes game time)."""
        system = self.systems.get(system_id)
        player = self.player
        if system is None or player is None:
            return False, "Nothing to repair."
        if system.condition >= 99.5:
            return False, "%s needs no repairs." % system.name
        if system.condition >= system.cap - 0.5:
            return False, "Ordinary parts will not help any further: something specific is missing."
        part = player.inventory.find_tag(self.content.items, "part")
        if not part:
            return False, "You need spare parts."
        player.inventory.remove(part, 1)
        gained = system.repair(ai.repair_amount(player))
        self.events.emit("repaired:" + system.id, npc=player.id, system=system.id, amount=gained)
        self.events.emit("player_repaired", system=system.id, amount=gained)
        self.add_log("You repaired %s (+%d%%)." % (system.name, round(gained)), "player")
        return True, "%s repaired: +%d%% integrity." % (system.name, round(gained))

    def report_theft(self, container: MapObject) -> Optional[Character]:
        """The player took from a restricted container: did anybody see?"""
        player = self.player
        if player is None:
            return None
        stealth = player.skill("stealth")
        sight = max(2, 7 - stealth // 2)
        for npc in self.chars_in(player.map_id):
            if npc.is_player or npc.asleep or not npc.alive:
                continue
            if abs(npc.x - player.x) + abs(npc.y - player.y) <= sight:
                npc.adjust_relation("player", -12)
                owner = self.characters.get(str(container.get("owner") or ""))
                if owner is not None and owner is not npc:
                    owner.adjust_relation("player", -8)
                self.flags["thefts_seen"] = int(self.flags.get("thefts_seen", 0)) + 1
                self.notify("%s saw you take that." % npc.short_name, "bad")
                self.add_log("%s saw you taking things from the %s." % (npc.name, container.name.lower()), "player")
                self.events.emit("theft", npc=npc.id, container=container.id)
                return npc
        return None

    def end_game(self, ending_id: str) -> None:
        if not self.ended:
            self.ended = str(ending_id)
            self.request("ending", id=str(ending_id))

    # ------------------------------------------------------------------
    # save / load
    # ------------------------------------------------------------------
    def to_dict(self) -> dict:
        maps: Dict[str, dict] = {}
        for map_id, gmap in self.maps.items():
            objs: Dict[str, dict] = {}
            for obj in gmap.objects:
                entry: Dict[str, Any] = {}
                if obj.state:
                    entry["state"] = dict(obj.state)
                if obj.inventory is not None:
                    entry["items"] = obj.inventory.to_dict()
                if entry:
                    objs[obj.id] = entry
            maps[map_id] = {
                "objects": objs,
                "added": list(self.added_objects.get(map_id, [])),
                "removed": list(self.removed_objects.get(map_id, [])),
                "tiles": list(self.tile_changes.get(map_id, [])),
            }
        state = self.rng.getstate()
        return {
            "version": config.SAVE_VERSION,
            "clock": self.clock.to_dict(),
            "rng": [state[0], list(state[1]), state[2]],
            "flags": dict(self.flags),
            "quests": self.quests.to_dict(),
            "director": self.director.to_dict(),
            "systems": self.systems.to_dict(),
            "characters": {cid: c.to_dict() for cid, c in self.characters.items()},
            "maps": maps,
            "log": self.log[-150:],
            "playtime": self.playtime,
            "ended": self.ended,
            "slow": self._slow,
        }

    @classmethod
    def from_dict(cls, content, data: dict) -> "World":
        world = cls(content)
        world.clock = GameClock.from_dict(data.get("clock") or {})
        rng = data.get("rng")
        if rng:
            try:
                world.rng.setstate((rng[0], tuple(rng[1]), rng[2]))
            except (TypeError, ValueError):
                pass
        world.flags = dict(data.get("flags") or {})
        world.quests.load_dict(data.get("quests"))
        world.director.load_dict(data.get("director"))
        world.systems.load_dict(data.get("systems"))
        for map_id, md in (data.get("maps") or {}).items():
            gmap = world.maps.get(map_id)
            if gmap is None:
                continue
            for obj_id in md.get("removed") or []:
                obj = gmap.get_object(obj_id)
                if obj is not None:
                    gmap.remove_object(obj)
            world.removed_objects[map_id] = list(md.get("removed") or [])
            for od in md.get("added") or []:
                if gmap.get_object(str(od.get("id", ""))) is None:
                    obj = MapObject.from_dict(od)
                    obj.map_id = map_id
                    gmap.objects.append(obj)
            for layer, x, y, tile in md.get("tiles") or []:
                if layer in gmap.layers and gmap.in_bounds(x, y):
                    gmap.layers[layer][y][x] = tile
            world.tile_changes[map_id] = [list(t) for t in md.get("tiles") or []]
            gmap.invalidate()
            world.added_objects[map_id] = [dict(o) for o in md.get("added") or []]
            for obj_id, entry in (md.get("objects") or {}).items():
                obj = gmap.get_object(obj_id)
                if obj is None:
                    continue
                obj.state = dict(entry.get("state") or {})
                if "items" in entry:
                    obj.inventory = Inventory.from_dict(entry["items"])
        world.reindex()
        for cid, cd in (data.get("characters") or {}).items():
            ch = Character.from_dict(cd, content)
            if ch.map_id not in world.maps and world.maps:
                ch.map_id = next(iter(world.maps))
            world.add_character(ch)
        # characters added to the data folder after this save was made
        for cid, cdef in content.characters.items():
            if cid not in world.characters:
                world.spawn_from_def(cid, cdef)
        world.log = list(data.get("log") or [])
        world.playtime = float(data.get("playtime", 0.0))
        world.ended = data.get("ended")
        world._slow = float(data.get("slow", 0.0))
        world._sync_marks()
        return world
