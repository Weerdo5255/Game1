"""Functions available to content scripts.

Everything decorated with ``@script_fn`` can be called from dialogue files,
quest rules, item effects and NPC goals.  ``python -m game.tools.validate
--api`` prints this list with the docstrings below, so keep them short and
useful: they are the scripting reference.

Conventions
-----------
* ``who`` is a character id.  Stat functions default to whoever is using an
  item, otherwise the player; relationship functions default to the NPC
  being talked to.
* Getters ("cond") are safe to use in ``if`` conditions; "effect" functions
  change the world and belong in ``do`` lists.
"""
from __future__ import annotations

from typing import Any, Optional

from ..sim import ai
from .script import ScriptError, script_fn


class ScriptAPI:
    def __init__(self, world):
        self.world = world

    # -- helpers (not exposed) ------------------------------------------------------
    @property
    def _ctx(self) -> dict:
        return self.world.script.ctx

    def _char(self, who: Optional[str] = None, default: str = "player"):
        if who in (None, ""):
            who = self._ctx.get(default) or ("player" if default in ("user", "player") else "")
            if not who:
                who = "player"
        ch = self.world.characters.get(who)
        if ch is None:
            raise ScriptError("unknown character %r" % (who,))
        return ch

    def _user(self, who: Optional[str] = None):
        return self._char(who, "user")

    def _npc(self, who: Optional[str] = None):
        return self._char(who, "npc")

    def _system(self, sid: str):
        system = self.world.systems.get(sid)
        if system is None:
            raise ScriptError("unknown system %r" % (sid,))
        return system

    def _object(self, obj_id: Optional[str] = None):
        obj = self.world.find_object(obj_id or self._ctx.get("obj", ""))
        if obj is None:
            raise ScriptError("unknown object %r" % (obj_id,))
        return obj

    def _item(self, item_id: str) -> str:
        if item_id not in self.world.content.items:
            raise ScriptError("unknown item %r" % (item_id,))
        return item_id

    # ==============================================================================
    # Flags and variables
    # ==============================================================================
    @script_fn("cond")
    def flag(self, name, default=False):
        """Value of a story flag (False if never set)."""
        return self.world.flags.get(name, default)

    @script_fn("cond")
    def var(self, name, default=0):
        """Value of a numeric story variable."""
        return self.world.flags.get(name, default)

    @script_fn()
    def set_flag(self, name, value=True):
        """Set a story flag; raises the event ``flag:<name>``."""
        self.world.flags[name] = value
        self.world.events.emit("flag:" + str(name), flag=name, value=value)

    @script_fn()
    def clear_flag(self, name):
        """Remove a story flag."""
        self.world.flags.pop(name, None)

    @script_fn()
    def set_var(self, name, value):
        """Set a numeric story variable."""
        self.world.flags[name] = value

    @script_fn()
    def add_var(self, name, amount=1):
        """Add to a numeric story variable."""
        self.world.flags[name] = (self.world.flags.get(name, 0) or 0) + amount

    # ==============================================================================
    # Player inventory and money
    # ==============================================================================
    @script_fn("cond")
    def has_item(self, item, count=1, who=None):
        """True if the character (default player) carries ``count`` of ``item``."""
        return self._char(who).inventory.has(item, int(count))

    @script_fn("cond")
    def item_count(self, item, who=None):
        """How many of ``item`` the character carries."""
        return self._char(who).inventory.count(item)

    @script_fn("cond")
    def has_tag(self, tag, who=None):
        """True if the character carries any item with this tag."""
        return self._char(who).inventory.find_tag(self.world.content.items, tag) is not None

    @script_fn("cond")
    def equipped(self, item, who=None):
        """True if ``item`` is equipped."""
        return self._char(who).is_equipped(item)

    @script_fn("cond")
    def credits(self, who=None):
        """Credits held."""
        return self._char(who).credits

    @script_fn()
    def give_item(self, item, count=1, who=None):
        """Create ``count`` of ``item`` in the character's inventory."""
        self.world.give_item(self._char(who), self._item(item), int(count))

    @script_fn()
    def take_item(self, item, count=1, who=None):
        """Remove items from the character's inventory."""
        self.world.take_item(self._char(who), item, int(count))

    @script_fn()
    def give_to(self, who, item, count=1):
        """The player hands ``count`` of ``item`` to an NPC."""
        npc = self._npc(who)
        moved = self.world.player.inventory.transfer(npc.inventory, item, int(count))
        self.world.player.check_equipment()
        if moved:
            self.world.notify("- %s%s" % (self.world.item_name(item), " x%d" % moved if moved > 1 else ""))
            npc.wants.pop(item, None) if npc.inventory.count(item) >= npc.wants.get(item, 0) else None
            npc.ai["think"] = 0.0
            self.world.events.emit("gave:%s:%s" % (npc.id, item), npc=npc.id, item=item, count=moved)

    @script_fn()
    def take_from(self, who, item, count=1):
        """An NPC hands the player up to ``count`` of ``item`` (if they have it)."""
        npc = self._npc(who)
        moved = npc.inventory.remove(item, int(count))
        npc.check_equipment()
        if moved:
            self.world.give_item(self.world.player, item, moved)

    @script_fn()
    def give_credits(self, amount, who=None):
        """Add credits."""
        ch = self._char(who)
        ch.credits += int(amount)
        if ch.is_player:
            self.world.notify("+ %d credits" % int(amount), "good")

    @script_fn()
    def take_credits(self, amount, who=None):
        """Remove credits (never below zero)."""
        ch = self._char(who)
        ch.credits = max(0, ch.credits - int(amount))
        if ch.is_player:
            self.world.notify("- %d credits" % int(amount))

    # ==============================================================================
    # Character stats
    # ==============================================================================
    @script_fn("cond")
    def skill(self, name, who=None):
        """Effective skill (with attribute bonus and modifiers)."""
        return self._char(who).skill(name)

    @script_fn("cond")
    def attr(self, name, who=None):
        """Effective attribute."""
        return self._char(who).attr(name)

    @script_fn("cond")
    def level(self, who=None):
        """Character level."""
        return self._char(who).level

    @script_fn("cond")
    def hp(self, who=None):
        """Current health."""
        return self._user(who).hp

    @script_fn("cond")
    def hp_pct(self, who=None):
        """Health as a percentage of maximum."""
        ch = self._user(who)
        return 100.0 * ch.hp / max(1, ch.max_hp)

    @script_fn("cond")
    def need(self, name, who=None):
        """A need value 0-100 ("hunger" or "fatigue"); higher is worse."""
        return self._user(who).needs.get(name, 0.0)

    @script_fn("cond")
    def morale(self, who=None):
        """Morale 0-100 of a character (default: the NPC being talked to)."""
        return self._npc(who).morale

    @script_fn("cond")
    def crew_morale(self):
        """Average morale of everyone on the station."""
        crew = [c.morale for c in self.world.npcs() if "synthetic" not in c.traits]
        return sum(crew) / len(crew) if crew else 50.0

    @script_fn("cond")
    def has_effect(self, effect, who=None):
        """True if the character currently has a status effect."""
        return self._user(who).has_effect(effect)

    @script_fn("cond")
    def background(self):
        """The player's background id."""
        return self.world.player.background

    @script_fn("cond")
    def has_trait(self, trait, who=None):
        """True if the character has a trait."""
        return trait in self._npc(who).traits

    @script_fn()
    def add_xp(self, amount):
        """Give the player experience."""
        player = self.world.player
        levels = player.add_xp(int(amount))
        self.world.notify("+ %d XP" % int(amount), "good")
        if levels:
            self.world.notify("Level up! Level %d - skill points available" % player.level, "good")
            self.world.events.emit("level_up", level=player.level)

    @script_fn()
    def add_skill(self, name, amount=1, who=None):
        """Permanently raise a skill."""
        ch = self._char(who)
        ch.skills[name] = max(0, ch.skills.get(name, 0) + int(amount))
        ch.invalidate()
        if ch.is_player:
            self.world.notify("%s %+d" % (name.capitalize(), int(amount)), "good")

    @script_fn()
    def add_attr(self, name, amount=1, who=None):
        """Permanently raise an attribute."""
        ch = self._char(who)
        ch.attrs[name] = max(1, ch.attrs.get(name, 4) + int(amount))
        ch.invalidate()

    @script_fn()
    def heal(self, amount, who=None):
        """Restore health."""
        self._user(who).heal(float(amount))

    @script_fn()
    def hurt(self, amount, who=None):
        """Deal damage."""
        ch = self._user(who)
        ch.hurt(float(amount))
        if ch.hp <= 0:
            self.world.collapse(ch)

    @script_fn()
    def add_need(self, name, amount, who=None):
        """Change a need: ``add_need('hunger', -45)`` feeds the character."""
        self._user(who).add_need(name, float(amount))

    @script_fn()
    def add_morale(self, amount, who=None):
        """Change morale of one character, or everybody with ``who='all'``."""
        if who == "all":
            for c in self.world.npcs():
                c.add_morale(float(amount))
            return
        self._npc(who).add_morale(float(amount))

    @script_fn()
    def add_effect(self, effect, hours=None, who=None):
        """Apply a status effect, optionally for a number of game hours."""
        if effect not in self.world.content.effects:
            raise ScriptError("unknown effect %r" % (effect,))
        until = None if hours is None else self.world.clock.minutes + float(hours) * 60
        self._user(who).add_effect(effect, until)

    @script_fn()
    def remove_effect(self, effect, who=None):
        """Remove a status effect."""
        self._user(who).remove_effect(effect)

    @script_fn()
    def give_access(self, group, who=None):
        """Grant a door access group (e.g. 'engineering')."""
        ch = self._char(who)
        if group not in ch.access:
            ch.access.append(group)
        self.world.nav.invalidate()

    # ==============================================================================
    # Relationships and other characters
    # ==============================================================================
    @script_fn("cond")
    def rel(self, who=None):
        """How much an NPC likes the player, -100..100."""
        return self._npc(who).opinion_of("player")

    @script_fn("cond")
    def rel_between(self, a, b):
        """How much character ``a`` likes character ``b``."""
        return self._char(a).opinion_of(b)

    @script_fn("cond")
    def allies(self, threshold=20):
        """How many people still aboard like the player at least this much."""
        return sum(1 for c in self.world.npcs() if c.opinion_of("player") >= float(threshold))

    @script_fn("cond")
    def met(self, who=None):
        """True once the player has spoken to this NPC."""
        return self._npc(who).met

    @script_fn("cond")
    def talked(self, who=None):
        """How many conversations the player has had with this NPC."""
        return int(self.world.flags.get("talked:" + self._npc(who).id, 0))

    @script_fn("cond")
    def present(self, who=None):
        """True while the character is on the station."""
        ch = self.world.characters.get(who or self._ctx.get("npc", ""))
        return bool(ch and ch.present and ch.alive)

    @script_fn("cond")
    def asleep(self, who=None):
        """True if the character is asleep."""
        return self._npc(who).asleep

    @script_fn("cond")
    def npc_in(self, who, map_id):
        """True if the character is on the given map."""
        ch = self._char(who)
        return ch.present and ch.map_id == map_id

    @script_fn("cond")
    def npc_region(self, who=None):
        """Id of the region the character is standing in ('' if none)."""
        ch = self._npc(who)
        gmap = self.world.maps.get(ch.map_id)
        region = gmap.region_at(ch.x, ch.y) if gmap else None
        return region.id if region else ""

    @script_fn("cond")
    def near(self, who, distance=4):
        """True if the character is within ``distance`` tiles of the player."""
        ch, p = self._char(who), self.world.player
        return ch.present and ch.map_id == p.map_id and abs(ch.x - p.x) + abs(ch.y - p.y) <= int(distance)

    @script_fn("cond")
    def npc_has(self, who, item, count=1):
        """True if an NPC carries the item."""
        return self._char(who).inventory.has(item, int(count))

    @script_fn("text")
    def name(self, who=None):
        """Display name of a character."""
        return self._npc(who).name

    @script_fn("text")
    def first_name(self, who=None):
        """Short name of a character."""
        return self._npc(who).short_name

    @script_fn("text")
    def player_name(self):
        """The player's name."""
        return self.world.player.name

    @script_fn("text")
    def title(self, who=None):
        """Job title of a character."""
        return self._npc(who).title

    @script_fn("text")
    def item_name(self, item):
        """Display name of an item."""
        return self.world.item_name(item)

    @script_fn()
    def add_rel(self, amount, who=None):
        """Change how much an NPC likes the player."""
        npc = self._npc(who)
        if npc.is_player:
            return
        npc.adjust_relation("player", float(amount))
        if abs(float(amount)) >= 5:
            self.world.notify("%s %s" % (npc.short_name, "approves" if amount > 0 else "disapproves"),
                              "good" if amount > 0 else "bad")

    @script_fn()
    def add_rel_between(self, a, b, amount):
        """Change how much character ``a`` likes character ``b``."""
        self._char(a).adjust_relation(b, float(amount))

    @script_fn()
    def set_met(self, who=None):
        """Mark an NPC as known to the player."""
        self._npc(who).met = True

    @script_fn()
    def set_dialogue(self, who, dialogue):
        """Switch the dialogue file an NPC uses."""
        if dialogue not in self.world.content.dialogues:
            raise ScriptError("unknown dialogue %r" % (dialogue,))
        self._char(who).dialogue = dialogue

    @script_fn()
    def set_schedule(self, who, schedule):
        """Change an NPC's daily schedule (a name from schedules.json)."""
        if schedule not in self.world.content.schedules:
            raise ScriptError("unknown schedule %r" % (schedule,))
        npc = self._char(who)
        npc.schedule = schedule
        npc.ai["think"] = 0.0

    @script_fn()
    def set_job(self, who, system=""):
        """Reassign an NPC to operate a different system ('' = none)."""
        npc = self._char(who)
        npc.job = dict(npc.job, system=system)
        for g in [g for g in npc.goals if g.get("source") == "routine"]:
            npc.goals.remove(g)
        npc.ai["goal"] = ""
        npc.ai["think"] = 0.0

    @script_fn()
    def set_home(self, who, bed=""):
        """Give an NPC a different bed to sleep in ('' = none)."""
        npc = self._char(who)
        if bed:
            self._object(bed)
            npc.home = {"bed": bed}
        else:
            npc.home = {}

    @script_fn()
    def set_maintain(self, who, *systems):
        """Make an NPC responsible for repairing the listed systems."""
        npc = self._char(who)
        for sid in systems:
            self._system(sid)
        npc.job = dict(npc.job, duty="maintain", maintain=list(systems))
        npc.ai["think"] = 0.0

    @script_fn("cond")
    def can_trade(self, who=None):
        """True if the NPC's character file gives them a ``trade`` section."""
        npc = self._npc(who)
        return bool((self.world.content.characters.get(npc.id) or {}).get("trade"))

    @script_fn()
    def move_npc(self, who, map_id, x, y, facing=None):
        """Teleport a character."""
        if map_id not in self.world.maps:
            raise ScriptError("unknown map %r" % (map_id,))
        ch = self._char(who)
        ch.path = []
        self.world.transfer(ch, map_id, int(x), int(y), facing)

    @script_fn()
    def remove_npc(self, who, reason="left"):
        """The character leaves the station permanently."""
        self.world.remove_character(self._char(who), reason)

    @script_fn()
    def say(self, text, who=None, seconds=4):
        """Show a speech bubble over a character."""
        self._npc(who).say(self.world.text(str(text), self._ctx), float(seconds))

    # ==============================================================================
    # NPC goals
    # ==============================================================================
    @script_fn("cond")
    def npc_goal(self, who=None):
        """Id of the goal an NPC is pursuing right now."""
        return self._npc(who).ai.get("goal", "")

    @script_fn("cond")
    def goal_active(self, who, goal):
        """True if the NPC has this goal and it is not finished."""
        g = ai.find_goal(self._char(who), goal)
        return bool(g and g["status"] == "active")

    @script_fn("cond")
    def goal_done(self, who, goal):
        """True if the NPC completed this goal."""
        g = ai.find_goal(self._char(who), goal)
        return bool(g and g["status"] == "done")

    @script_fn("cond")
    def goal_failed(self, who, goal):
        """True if the NPC failed this goal."""
        g = ai.find_goal(self._char(who), goal)
        return bool(g and g["status"] == "failed")

    @script_fn("cond")
    def goal_progress(self, who, goal):
        """Progress 0..1 of an NPC goal."""
        g = ai.find_goal(self._char(who), goal)
        return float(g.get("progress", 0.0)) if g else 0.0

    @script_fn("text")
    def goal_text(self, who=None):
        """Description of what an NPC is currently trying to do."""
        return ai.goal_text(self._npc(who))

    @script_fn("text")
    def activity(self, who=None):
        """What an NPC is doing this moment ("Repairing Life Support")."""
        return self._npc(who).activity

    @script_fn("cond")
    def wants(self, who=None):
        """Id of an item the NPC is looking for ('' if nothing)."""
        npc = self._npc(who)
        for item in sorted(npc.wants):
            if item in self.world.content.items:
                return item
        return ""

    @script_fn("cond")
    def wants_count(self, who=None):
        """How many of the wanted item the NPC still needs."""
        npc = self._npc(who)
        item = self.wants(npc.id)
        return int(npc.wants.get(item, 0)) if item else 0

    @script_fn()
    def add_goal(self, who, goal, priority=None, **args):
        """Give an NPC a goal from goals.json (extra keyword args fill ``$name``
        placeholders in the template)."""
        npc = self._char(who)
        if goal not in self.world.content.goals:
            raise ScriptError("unknown goal template %r" % (goal,))
        spec = {"use": goal, "id": args.pop("id", goal), "args": args}
        if priority is not None:
            spec["priority"] = float(priority)
        inst = ai.instantiate(self.world, spec, "story")
        if inst is None:
            raise ScriptError("goal template %r has no steps" % (goal,))
        ai.add_goal(self.world, npc, inst)

    @script_fn()
    def cancel_goal(self, who, goal):
        """Remove a goal from an NPC."""
        ai.remove_goal(self._char(who), goal)

    @script_fn()
    def set_goal_priority(self, who, goal, priority):
        """Change how badly an NPC wants one of their goals."""
        g = ai.find_goal(self._char(who), goal)
        if g:
            g["priority"] = float(priority)

    # ==============================================================================
    # Station systems
    # ==============================================================================
    @script_fn("cond")
    def cond(self, system):
        """Integrity (0-100) of a station system."""
        return self._system(system).condition

    @script_fn("cond")
    def eff(self, system):
        """Current efficiency (0-1) of a station system."""
        return self._system(system).efficiency

    @script_fn("cond")
    def online(self, system):
        """True if the system is switched on and producing anything."""
        s = self._system(system)
        return s.enabled and s.efficiency > 0.05

    @script_fn("cond")
    def enabled(self, system):
        """True if the system is switched on."""
        return self._system(system).enabled

    @script_fn("cond")
    def priority(self, system):
        """A system's place in the power queue (1 = served first)."""
        return self._system(system).priority

    @script_fn("cond")
    def sys_status(self, system):
        """'ok', 'degraded', 'critical' or 'offline'."""
        return self._system(system).status

    @script_fn("cond")
    def res(self, resource):
        """Amount of a station resource (e.g. 'air', 'water')."""
        return self.world.systems.resource(resource)

    @script_fn("cond")
    def res_pct(self, resource):
        """A station resource as a percentage of its capacity."""
        return 100.0 * self.world.systems.resource_frac(resource)

    @script_fn("cond")
    def power_supply(self):
        """Power being generated (kW)."""
        return self.world.systems.power_supply

    @script_fn("cond")
    def power_demand(self):
        """Power being asked for (kW)."""
        return self.world.systems.power_demand

    @script_fn("cond")
    def brownout(self):
        """True while demand exceeds supply."""
        return self.world.systems.brownout

    @script_fn("cond")
    def stock(self, item):
        """Total count of an item in every container on the station."""
        return self.world.stock(item)

    @script_fn("cond")
    def container_count(self, obj, item):
        """Count of an item in one container."""
        inv = self.world.inventory_of(self._object(obj))
        return inv.count(item) if inv else 0

    @script_fn()
    def repair(self, system, amount):
        """Restore integrity to a system."""
        self._system(system).repair(float(amount))

    @script_fn()
    def damage(self, system, amount):
        """Damage a system."""
        self._system(system).damage(float(amount))

    @script_fn()
    def set_cap(self, system, value=100):
        """Raise or lower the integrity ceiling repairs can reach on a system."""
        self._system(system).cap = max(0.0, min(100.0, float(value)))

    @script_fn("cond")
    def cap(self, system):
        """The integrity ceiling of a system (100 unless something is missing)."""
        return self._system(system).cap

    @script_fn()
    def move_items(self, source, dest, item=None):
        """Move everything (or one kind of item) between two inventories;
        each is 'player', a character id or a container object id."""
        src = self.world.resolve_inventory(source)
        dst = self.world.resolve_inventory(dest)
        if src is None or dst is None:
            raise ScriptError("move_items: unknown inventory %r or %r" % (source, dest))
        for item_id, qty in list(src.entries()):
            if item is None or item_id == item:
                src.transfer(dst, item_id, qty)

    @script_fn()
    def set_condition(self, system, value):
        """Set a system's integrity outright."""
        s = self._system(system)
        s.condition = max(0.0, min(100.0, float(value)))

    @script_fn()
    def set_enabled(self, system, on=True):
        """Switch a system on or off."""
        self._system(system).enabled = bool(on)

    @script_fn()
    def set_priority(self, system, priority):
        """Change a system's place in the power queue (1 = first)."""
        self._system(system).priority = int(priority)

    @script_fn()
    def add_res(self, resource, amount):
        """Add to (or subtract from) a station resource."""
        self.world.systems.add_resource(resource, float(amount))

    # ==============================================================================
    # Quests
    # ==============================================================================
    @script_fn("cond")
    def quest_active(self, quest):
        """True while a quest is in progress."""
        return self.world.quests.status(quest) == "active"

    @script_fn("cond")
    def quest_done(self, quest):
        """True once a quest was completed."""
        return self.world.quests.status(quest) == "done"

    @script_fn("cond")
    def quest_failed(self, quest):
        """True once a quest was failed."""
        return self.world.quests.status(quest) == "failed"

    @script_fn("cond")
    def quest_started(self, quest):
        """True if a quest was ever started."""
        return self.world.quests.started(quest)

    @script_fn("cond")
    def stage(self, quest, stage=None):
        """Current stage id of a quest, or True/False if ``stage`` is given."""
        current = self.world.quests.stage(quest)
        if stage is None:
            return current
        return self.world.quests.status(quest) == "active" and current == stage

    @script_fn("cond")
    def stage_seen(self, quest, stage):
        """True if the quest ever reached ``stage``."""
        return self.world.quests.visited(quest, stage)

    def _quest(self, quest: str) -> str:
        if quest not in self.world.quests.defs:
            raise ScriptError("unknown quest %r" % (quest,))
        return quest

    @script_fn()
    def start_quest(self, quest, stage=None):
        """Add a quest to the journal."""
        w = self.world
        if w.quests.start(self._quest(quest), w.clock.minutes, stage):
            w.notify("New quest: %s" % w.quests.name(quest), "quest")
            w.add_log("Quest started: %s." % w.quests.name(quest), "quest")
            w.events.emit("quest:%s:%s" % (quest, w.quests.stage(quest)), quest=quest)
            w.events.emit("quest_start:" + quest, quest=quest)

    @script_fn()
    def set_stage(self, quest, stage):
        """Move a quest to another stage."""
        w = self.world
        fresh = not w.quests.started(self._quest(quest))
        if w.quests.set_stage(quest, stage, w.clock.minutes):
            w.notify(("New quest: %s" if fresh else "Quest updated: %s") % w.quests.name(quest), "quest")
            w.events.emit("quest:%s:%s" % (quest, stage), quest=quest)

    @script_fn()
    def complete_quest(self, quest):
        """Finish a quest successfully (awards its XP)."""
        w = self.world
        if w.quests.finish(self._quest(quest), "done", w.clock.minutes):
            w.notify("Quest complete: %s" % w.quests.name(quest), "quest")
            w.add_log("Quest complete: %s." % w.quests.name(quest), "quest")
            xp = int(w.quests.defs[quest].get("xp", 0))
            if xp:
                self.add_xp(xp)
            w.events.emit("quest_done:" + quest, quest=quest)

    @script_fn()
    def fail_quest(self, quest):
        """Fail a quest."""
        w = self.world
        if w.quests.finish(self._quest(quest), "failed", w.clock.minutes):
            w.notify("Quest failed: %s" % w.quests.name(quest), "bad")
            w.add_log("Quest failed: %s." % w.quests.name(quest), "quest")
            w.events.emit("quest_failed:" + quest, quest=quest)

    @script_fn()
    def journal(self, text):
        """Write a free-form note into the journal."""
        w = self.world
        w.quests.add_note(w.text(str(text), self._ctx), w.clock.minutes)
        w.notify("Journal updated", "quest")

    # ==============================================================================
    # Time and place
    # ==============================================================================
    @script_fn("cond")
    def day(self):
        """Current day number (starts at 1)."""
        return self.world.clock.day

    @script_fn("cond")
    def hour(self):
        """Hour of the day, 0-23."""
        return self.world.clock.hour

    @script_fn("cond")
    def minute(self):
        """Minute of the hour."""
        return self.world.clock.minute

    @script_fn("cond")
    def time(self):
        """Game minutes elapsed since 00:00 on day 1."""
        return self.world.clock.minutes

    @script_fn("cond")
    def time_between(self, start_hour, end_hour):
        """True if the time of day is within the range (may wrap midnight)."""
        return self.world.clock.is_between(float(start_hour), float(end_hour))

    @script_fn("cond")
    def hours_until(self, day, hour=0):
        """Game hours remaining until a given day and hour."""
        return (self.world.clock.to_minutes(int(day), float(hour)) - self.world.clock.minutes) / 60.0

    @script_fn("text")
    def clock(self):
        """The time as text, e.g. 'Day 2 14:05'."""
        return self.world.clock.stamp()

    @script_fn("cond")
    def in_map(self, map_id):
        """True if the player is on this map."""
        return self.world.player.map_id == map_id

    @script_fn("cond")
    def in_region(self, region):
        """True if the player is standing in this region."""
        p = self.world.player
        gmap = self.world.maps.get(p.map_id)
        return bool(gmap and any(r.id == region for r in gmap.regions_at(p.x, p.y)))

    @script_fn()
    def teleport(self, map_id, x, y, facing=None):
        """Move the player."""
        if map_id not in self.world.maps:
            raise ScriptError("unknown map %r" % (map_id,))
        self.world.transfer(self.world.player, map_id, int(x), int(y), facing)
        self.world.request("fade")

    @script_fn()
    def pass_time(self, minutes):
        """Let game time pass (the whole simulation keeps running)."""
        self.world.fast_forward(float(minutes))
        self.world.request("fade")

    # ==============================================================================
    # Chance
    # ==============================================================================
    @script_fn("cond")
    def chance(self, probability):
        """True with the given probability (0-1)."""
        return self.world.rng.random() < float(probability)

    @script_fn("cond")
    def rand(self, low, high):
        """Random whole number between low and high inclusive."""
        return self.world.rng.randint(int(low), int(high))

    @script_fn("cond")
    def roll(self, skill, difficulty, who=None):
        """Skill check: d20 + skill >= difficulty."""
        return self.world.skill_check(self._char(who), skill, int(difficulty))

    # ==============================================================================
    # Map objects
    # ==============================================================================
    @script_fn("cond")
    def obj(self, key, obj=None, default=False):
        """Read a property of a map object."""
        return self._object(obj).get(key, default)

    @script_fn("cond")
    def obj_exists(self, obj):
        """True if a map object with this id currently exists."""
        return self.world.find_object(str(obj)) is not None

    @script_fn("cond")
    def locked(self, obj=None):
        """True if a door or container is locked."""
        return bool(self._object(obj).get("locked"))

    @script_fn()
    def set_obj(self, obj, key, value):
        """Change a property of a map object."""
        self._object(obj).set(key, value)
        self.world.maps[self._object(obj).map_id].invalidate()

    @script_fn()
    def unlock(self, obj=None):
        """Unlock a door, portal or container."""
        self._object(obj).set("locked", False)
        self.world.nav.invalidate()

    @script_fn()
    def lock(self, obj=None):
        """Lock a door, portal or container."""
        self._object(obj).set("locked", True)
        self.world.nav.invalidate()

    @script_fn()
    def put_item(self, obj, item, count=1):
        """Add items to a container."""
        inv = self.world.inventory_of(self._object(obj))
        inv.add(self._item(item), int(count))

    @script_fn()
    def remove_from(self, obj, item, count=1):
        """Remove items from a container."""
        self.world.inventory_of(self._object(obj)).remove(item, int(count))

    @script_fn()
    def spawn_item(self, map_id, x, y, item, count=1):
        """Drop an item pickup onto a map."""
        if map_id not in self.world.maps:
            raise ScriptError("unknown map %r" % (map_id,))
        self.world.add_object(map_id, "item", int(x), int(y), item=self._item(item), qty=int(count))

    @script_fn()
    def set_tile(self, map_id, layer, x, y, tile):
        """Change one map tile (layer: ground, decor, objects or overhead)."""
        if map_id not in self.world.maps:
            raise ScriptError("unknown map %r" % (map_id,))
        self.world.set_tile(map_id, layer, int(x), int(y), str(tile))
        self.world.nav.invalidate()

    @script_fn()
    def fill_tiles(self, map_id, layer, x, y, w, h, tile):
        """Change a rectangle of map tiles ('' clears them)."""
        if map_id not in self.world.maps:
            raise ScriptError("unknown map %r" % (map_id,))
        for yy in range(int(y), int(y) + int(h)):
            for xx in range(int(x), int(x) + int(w)):
                self.world.set_tile(map_id, layer, xx, yy, str(tile))
        self.world.nav.invalidate()

    @script_fn()
    def remove_obj(self, obj=None):
        """Delete a map object."""
        self.world.remove_object(self._object(obj))

    # ==============================================================================
    # Presentation and flow
    # ==============================================================================
    @script_fn()
    def notify(self, text, kind="info"):
        """Show a small notification."""
        self.world.notify(self.world.text(str(text), self._ctx), kind)

    @script_fn()
    def announce(self, text):
        """Station-wide announcement (also written to the log)."""
        self.world.announce(self.world.text(str(text), self._ctx))

    @script_fn()
    def log(self, text):
        """Write a line into the activity log."""
        self.world.add_log(self.world.text(str(text), self._ctx), "info")

    @script_fn()
    def sound(self, name):
        """Play a sound effect."""
        self.world.request("sound", name=str(name))

    @script_fn()
    def emit(self, event):
        """Raise a custom event for director rules."""
        self.world.events.emit(str(event))

    @script_fn()
    def open_trade(self, who=None):
        """Open the trade screen with an NPC."""
        self.world.request("trade", npc=self._npc(who).id)

    @script_fn()
    def open_container(self, obj=None):
        """Open a container's contents."""
        self.world.request("container", obj=self._object(obj).id)

    @script_fn()
    def start_dialogue(self, dialogue, who=None, node=None):
        """Start another dialogue (optionally with an NPC, at a node)."""
        if dialogue not in self.world.content.dialogues:
            raise ScriptError("unknown dialogue %r" % (dialogue,))
        self.world.request("dialogue", dialogue=dialogue, npc=who or "", node=node)

    @script_fn()
    def end_game(self, ending):
        """End the game with the given ending id."""
        if ending not in self.world.content.endings:
            raise ScriptError("unknown ending %r" % (ending,))
        self.world.end_game(ending)
