"""Characters: the player and every NPC share this class.

A character definition lives in ``data/characters/<id>.json`` (see
``docs/AUTHORING.md`` for every field).  At runtime the same object carries
the simulation state: position, vitals, needs, inventory, relationships and
the AI's goals, all of which round-trip through ``to_dict`` / ``from_dict``
for saving.

Stats
-----
* Attributes (1-10): body, dexterity, intellect, presence, resolve.
* Skills (0-10+): each is governed by an attribute, which grants a small
  bonus: ``effective = skill + (attribute - 4) // 2 + modifiers``.
* Modifiers come from equipped items and active effects, using keys such as
  ``skill.engineering``, ``skill.all``, ``attr.body``, ``max_hp``, ``carry``,
  ``speed``, ``work``.
"""
from __future__ import annotations

from typing import Any, Dict, List, Optional, Tuple

from .. import config
from .items import Inventory, ItemDef

ATTRIBUTES = ["body", "dexterity", "intellect", "presence", "resolve"]
ATTR_LABELS = {
    "body": "Body", "dexterity": "Dexterity", "intellect": "Intellect",
    "presence": "Presence", "resolve": "Resolve",
}
ATTR_HELP = {
    "body": "Health, carrying capacity and brute force.",
    "dexterity": "Steady hands: engineering and stealth.",
    "intellect": "Tech, science and medicine.",
    "presence": "Persuasion, intimidation and trade.",
    "resolve": "Endurance; shrugging off hunger, fear and cold.",
}
# skill -> governing attribute
SKILLS: Dict[str, str] = {
    "engineering": "dexterity",
    "tech": "intellect",
    "science": "intellect",
    "medicine": "intellect",
    "persuasion": "presence",
    "intimidation": "presence",
    "stealth": "dexterity",
    "survival": "resolve",
    "athletics": "body",
}
SKILL_LABELS = {s: s.capitalize() for s in SKILLS}
SKILL_HELP = {
    "engineering": "Repair and maintain station machinery.",
    "tech": "Computers, locks, fabrication and drones.",
    "science": "Research, analysis and hydroponics.",
    "medicine": "Treat injuries and read people's health.",
    "persuasion": "Talk people round.",
    "intimidation": "Lean on people.",
    "stealth": "Go unnoticed; take what isn't yours.",
    "survival": "Cope with the ice, the dark and the cold.",
    "athletics": "Force, haul, climb.",
}

NEEDS = ["hunger", "fatigue"]

RELATION_TIERS = [(-50, "Hostile"), (-15, "Cold"), (15, "Neutral"), (40, "Friendly"), (70, "Trusted")]


def relation_label(value: float) -> str:
    for limit, label in RELATION_TIERS:
        if value < limit:
            return label
    return "Devoted"


def _key_to_json(key):
    if not key:
        return None
    return [key[0], [list(c) for c in key[1]]]


def _key_from_json(data):
    if not data:
        return None
    return (data[0], tuple((int(c[0]), int(c[1])) for c in data[1]))


def clamp(v: float, lo: float, hi: float) -> float:
    return lo if v < lo else hi if v > hi else v


class Character:
    def __init__(self, id: str, name: str = ""):
        self.id = id
        self.name = name or id
        self.title = ""
        self.role = "civilian"
        self.faction = ""
        self.bio = ""
        self.appearance: Dict[str, Any] = {}
        self.is_player = False
        self.background = ""
        self.blip = "blip"

        self.attrs: Dict[str, int] = {a: 4 for a in ATTRIBUTES}
        self.skills: Dict[str, int] = {s: 0 for s in SKILLS}
        self.level = 1
        self.xp = 0
        self.skill_points = 0
        self.hp = 0.0
        self.credits = 0
        self.needs: Dict[str, float] = {n: 0.0 for n in NEEDS}
        self.morale = 65.0
        self.effects: List[Dict[str, Any]] = []        # timed effects: {"id", "until"}
        self.auto_effects: List[str] = []              # condition effects (hungry, cold ...)
        self.inventory = Inventory()
        self.equipped: Dict[str, str] = {}

        # position / movement
        self.map_id = ""
        self.x = 0
        self.y = 0
        self.prev_x = 0
        self.prev_y = 0
        self.move_t = 1.0
        self.facing = "down"
        self.speed = config.NPC_SPEED
        self.anim_time = 0.0
        self.blocked_time = 0.0

        # social
        self.relations: Dict[str, float] = {}
        self.met = False
        self.dialogue = ""
        self.alive = True
        self.present = True            # False once they have left the station
        self.flags: Dict[str, Any] = {}
        self.traits: List[str] = []
        self.access: List[str] = ["crew"]

        # simulation / AI (see game/sim/ai.py)
        self.home: Dict[str, Any] = {}
        self.job: Dict[str, Any] = {}
        self.schedule: Any = "day"
        self.goals: List[Dict[str, Any]] = []
        self.ai: Dict[str, Any] = {}
        self.wants: Dict[str, int] = {}
        self.activity = ""
        self.asleep = False
        self.spawn: Dict[str, Any] = {}

        # not serialised
        self.content = None
        self.path: List[Tuple[int, int]] = []
        self._path_key = None
        self._mods: Optional[Dict[str, float]] = None
        self.bark = ""                # speech bubble text
        self.bark_time = 0.0

    # ------------------------------------------------------------------
    # construction
    # ------------------------------------------------------------------
    @classmethod
    def from_def(cls, char_id: str, data: dict, content=None) -> "Character":
        """Create a fresh character from a definition file."""
        ch = cls(char_id, str(data.get("name", char_id)))
        ch.content = content
        ch.title = str(data.get("title", ""))
        ch.role = str(data.get("role", "civilian"))
        ch.faction = str(data.get("faction", ""))
        ch.bio = str(data.get("bio", ""))
        ch.appearance = dict(data.get("appearance") or {})
        ch.blip = str(data.get("blip", "blip_robot" if ch.appearance.get("body") == "drone" else "blip"))
        for a in ATTRIBUTES:
            ch.attrs[a] = int((data.get("attrs") or {}).get(a, ch.attrs[a]))
        for s, v in (data.get("skills") or {}).items():
            if s in SKILLS:
                ch.skills[s] = int(v)
        ch.level = int(data.get("level", 1))
        ch.credits = int(data.get("credits", 0))
        ch.morale = float(data.get("morale", 65))
        ch.traits = list(data.get("traits") or [])
        ch.access = list(data.get("access") or ["crew"])
        ch.inventory = Inventory(data.get("inventory") or {})
        for slot, item_id in (data.get("equipped") or {}).items():
            ch.equipped[slot] = item_id
            if not ch.inventory.has(item_id):
                ch.inventory.add(item_id, 1)
        ch.relations = {k: float(v) for k, v in (data.get("relations") or {}).items()}
        ch.dialogue = str(data.get("dialogue", ""))
        ch.home = dict(data.get("home") or {})
        ch.job = dict(data.get("job") or {})
        ch.schedule = data.get("schedule", "day")
        ch.spawn = dict(data.get("spawn") or {})
        ch.flags = dict(data.get("flags") or {})
        needs = data.get("needs") or {}
        for n in NEEDS:
            ch.needs[n] = float(needs.get(n, 0.0))
        ch.map_id = str(ch.spawn.get("map", ""))
        ch.x = ch.prev_x = int(ch.spawn.get("x", 0))
        ch.y = ch.prev_y = int(ch.spawn.get("y", 0))
        ch.facing = str(ch.spawn.get("facing", "down"))
        ch.hp = float(ch.max_hp)
        return ch

    # ------------------------------------------------------------------
    # modifiers and stats
    # ------------------------------------------------------------------
    def invalidate(self) -> None:
        self._mods = None

    def mods(self) -> Dict[str, float]:
        """Sum of all modifiers from equipment and effects (cached)."""
        if self._mods is None:
            total: Dict[str, float] = {}
            content = self.content
            if content is not None:
                for item_id in self.equipped.values():
                    idef = content.items.get(item_id)
                    if idef is not None:
                        for k, v in idef.mods.items():
                            total[k] = total.get(k, 0.0) + v
                for eff_id in self.active_effects():
                    edef = content.effects.get(eff_id)
                    if edef:
                        for k, v in (edef.get("mods") or {}).items():
                            total[k] = total.get(k, 0.0) + v
            self._mods = total
        return self._mods

    def active_effects(self) -> List[str]:
        ids = list(self.auto_effects)
        for e in self.effects:
            if e["id"] not in ids:
                ids.append(e["id"])
        return ids

    def has_effect(self, effect_id: str) -> bool:
        return effect_id in self.auto_effects or any(e["id"] == effect_id for e in self.effects)

    def add_effect(self, effect_id: str, until: Optional[float] = None) -> None:
        for e in self.effects:
            if e["id"] == effect_id:
                if until is None or e.get("until") is None:
                    e["until"] = None
                else:
                    e["until"] = max(e["until"], until)
                break
        else:
            self.effects.append({"id": effect_id, "until": until})
        self.invalidate()

    def remove_effect(self, effect_id: str) -> None:
        before = len(self.effects)
        self.effects = [e for e in self.effects if e["id"] != effect_id]
        if len(self.effects) != before:
            self.invalidate()

    def attr(self, name: str) -> int:
        return int(self.attrs.get(name, 0) + self.mods().get("attr." + name, 0))

    def attr_bonus(self, name: str) -> int:
        return (self.attr(name) - 4) // 2

    def skill(self, name: str) -> int:
        """Effective skill including attribute bonus and modifiers."""
        base = self.skills.get(name, 0)
        gov = SKILLS.get(name)
        m = self.mods()
        value = base + (self.attr_bonus(gov) if gov else 0) + m.get("skill." + name, 0) + m.get("skill.all", 0)
        return max(0, int(value))

    @property
    def max_hp(self) -> int:
        return max(1, int(20 + self.attr("body") * 4 + (self.level - 1) * 2 + self.mods().get("max_hp", 0)))

    @property
    def carry_capacity(self) -> float:
        return 12 + self.attr("body") * 4 + self.mods().get("carry", 0)

    def carried_weight(self) -> float:
        if self.content is None:
            return 0.0
        return self.inventory.weight(self.content.items)

    @property
    def encumbered(self) -> bool:
        return self.carried_weight() > self.carry_capacity

    def condition_factor(self) -> float:
        """0.2 .. 1.0 multiplier describing how fit for work the character is."""
        f = 1.0
        hunger, fatigue = self.needs.get("hunger", 0), self.needs.get("fatigue", 0)
        if hunger > 90:
            f -= 0.35
        elif hunger > 70:
            f -= 0.15
        if fatigue > 90:
            f -= 0.4
        elif fatigue > 75:
            f -= 0.15
        if self.morale < 25:
            f -= 0.2
        elif self.morale > 80:
            f += 0.05
        if self.hp < self.max_hp * 0.4:
            f -= 0.2
        f += self.mods().get("work", 0.0)
        return clamp(f, 0.2, 1.25)

    def work_rate(self, skill: str) -> float:
        """Labour produced per hour at a job using ``skill``."""
        return (0.5 + 0.1 * self.skill(skill)) * self.condition_factor()

    # ------------------------------------------------------------------
    # vitals
    # ------------------------------------------------------------------
    def heal(self, amount: float) -> float:
        before = self.hp
        self.hp = clamp(self.hp + amount, 0.0, float(self.max_hp))
        return self.hp - before

    def hurt(self, amount: float) -> float:
        before = self.hp
        self.hp = clamp(self.hp - amount, 0.0, float(self.max_hp))
        return before - self.hp

    def add_need(self, need: str, delta: float) -> None:
        if need in self.needs:
            self.needs[need] = clamp(self.needs[need] + delta, 0.0, 100.0)

    def add_morale(self, delta: float) -> None:
        self.morale = clamp(self.morale + delta, 0.0, 100.0)

    # ------------------------------------------------------------------
    # progression
    # ------------------------------------------------------------------
    def xp_to_next(self) -> int:
        return 100 * self.level

    def add_xp(self, amount: int) -> int:
        """Add experience; returns the number of levels gained."""
        gained = 0
        self.xp += max(0, int(amount))
        while self.xp >= self.xp_to_next():
            self.xp -= self.xp_to_next()
            self.level += 1
            self.skill_points += 2
            gained += 1
        if gained:
            self.invalidate()
            self.hp = min(float(self.max_hp), self.hp + 2 * gained)
        return gained

    def raise_skill(self, skill: str) -> bool:
        if self.skill_points > 0 and skill in SKILLS and self.skills.get(skill, 0) < 10:
            self.skills[skill] = self.skills.get(skill, 0) + 1
            self.skill_points -= 1
            self.invalidate()
            return True
        return False

    # ------------------------------------------------------------------
    # equipment / access
    # ------------------------------------------------------------------
    def equip(self, item_id: str) -> bool:
        idef: Optional[ItemDef] = self.content.items.get(item_id) if self.content else None
        if idef is None or not idef.slot or not self.inventory.has(item_id):
            return False
        self.equipped[idef.slot] = item_id
        self.invalidate()
        self.hp = min(self.hp, float(self.max_hp))
        return True

    def unequip(self, slot: str) -> bool:
        if slot in self.equipped:
            del self.equipped[slot]
            self.invalidate()
            self.hp = min(self.hp, float(self.max_hp))
            return True
        return False

    def is_equipped(self, item_id: str) -> bool:
        return item_id in self.equipped.values()

    def check_equipment(self) -> None:
        """Unequip anything that is no longer in the inventory."""
        for slot, item_id in list(self.equipped.items()):
            if not self.inventory.has(item_id):
                del self.equipped[slot]
                self.invalidate()

    def access_groups(self) -> List[str]:
        """Door access groups from the character's role and carried keycards."""
        groups = list(self.access)
        if self.content is not None:
            for item_id in self.inventory.items:
                idef = self.content.items.get(item_id)
                if idef is not None:
                    for g in idef.access_groups():
                        if g not in groups:
                            groups.append(g)
        return groups

    # ------------------------------------------------------------------
    # relationships
    # ------------------------------------------------------------------
    def opinion_of(self, other_id: str) -> float:
        return self.relations.get(other_id, 0.0)

    def adjust_relation(self, other_id: str, delta: float) -> float:
        value = clamp(self.relations.get(other_id, 0.0) + delta, -100.0, 100.0)
        self.relations[other_id] = value
        return value

    # ------------------------------------------------------------------
    # movement helpers
    # ------------------------------------------------------------------
    @property
    def moving(self) -> bool:
        return self.move_t < 1.0

    @property
    def pos(self) -> Tuple[int, int]:
        return (self.x, self.y)

    def pixel_pos(self) -> Tuple[float, float]:
        """Top-left pixel of the tile the character is drawn on (interpolated)."""
        t = self.move_t if self.move_t < 1.0 else 1.0
        fx = self.prev_x + (self.x - self.prev_x) * t
        fy = self.prev_y + (self.y - self.prev_y) * t
        return fx * config.TILE, fy * config.TILE

    def place(self, map_id: str, x: int, y: int, facing: Optional[str] = None) -> None:
        self.map_id = map_id
        self.x = self.prev_x = int(x)
        self.y = self.prev_y = int(y)
        self.move_t = 1.0
        self.path = []
        if facing:
            self.facing = facing

    def face_towards(self, x: int, y: int) -> None:
        self.facing = config.dir_from_delta(x - self.x, y - self.y, self.facing)

    def say(self, text: str, seconds: float = 3.0) -> None:
        self.bark = text
        self.bark_time = seconds

    @property
    def display_name(self) -> str:
        return self.name

    @property
    def short_name(self) -> str:
        parts = [p for p in self.name.replace('"', "").split(" ") if p and not p.endswith(".")]
        return parts[0] if parts else self.name

    # ------------------------------------------------------------------
    # save / load
    # ------------------------------------------------------------------
    def to_dict(self) -> dict:
        return {
            "id": self.id, "name": self.name, "title": self.title, "role": self.role,
            "faction": self.faction, "bio": self.bio, "appearance": dict(self.appearance),
            "is_player": self.is_player, "background": self.background, "blip": self.blip,
            "attrs": dict(self.attrs), "skills": dict(self.skills),
            "level": self.level, "xp": self.xp, "skill_points": self.skill_points,
            "hp": self.hp, "credits": self.credits, "needs": dict(self.needs), "morale": self.morale,
            "effects": [dict(e) for e in self.effects], "auto_effects": list(self.auto_effects),
            "inventory": self.inventory.to_dict(), "equipped": dict(self.equipped),
            "map": self.map_id, "x": self.x, "y": self.y, "facing": self.facing, "speed": self.speed,
            # mid-step state, so a loaded game carries on exactly where it stopped
            "move": [self.prev_x, self.prev_y, self.move_t, self.blocked_time],
            "path": [list(c) for c in self.path],
            "path_key": _key_to_json(getattr(self, "_path_key", None)),
            "relations": dict(self.relations), "met": self.met, "dialogue": self.dialogue,
            "alive": self.alive, "present": self.present, "flags": dict(self.flags),
            "traits": list(self.traits), "access": list(self.access),
            "home": dict(self.home), "job": dict(self.job), "schedule": self.schedule,
            "goals": self.goals, "ai": self.ai, "wants": dict(self.wants),
            "activity": self.activity, "asleep": self.asleep, "spawn": dict(self.spawn),
        }

    @classmethod
    def from_dict(cls, data: dict, content=None) -> "Character":
        ch = cls(str(data["id"]), str(data.get("name", data["id"])))
        ch.content = content
        for key in ("title", "role", "faction", "bio", "background", "blip", "dialogue", "activity"):
            setattr(ch, key, str(data.get(key, getattr(ch, key))))
        ch.appearance = dict(data.get("appearance") or {})
        ch.is_player = bool(data.get("is_player", False))
        ch.attrs.update({k: int(v) for k, v in (data.get("attrs") or {}).items()})
        ch.skills.update({k: int(v) for k, v in (data.get("skills") or {}).items()})
        ch.level = int(data.get("level", 1))
        ch.xp = int(data.get("xp", 0))
        ch.skill_points = int(data.get("skill_points", 0))
        ch.credits = int(data.get("credits", 0))
        ch.needs.update({k: float(v) for k, v in (data.get("needs") or {}).items()})
        ch.morale = float(data.get("morale", 65))
        ch.effects = [dict(e) for e in data.get("effects") or []]
        ch.auto_effects = list(data.get("auto_effects") or [])
        ch.inventory = Inventory.from_dict(data.get("inventory"))
        ch.equipped = dict(data.get("equipped") or {})
        ch.place(str(data.get("map", "")), data.get("x", 0), data.get("y", 0), str(data.get("facing", "down")))
        ch.speed = float(data.get("speed", config.NPC_SPEED))
        move = data.get("move")
        if move and len(move) >= 4:
            ch.prev_x, ch.prev_y, ch.move_t, ch.blocked_time = int(move[0]), int(move[1]), float(move[2]), float(move[3])
        ch.path = [(int(c[0]), int(c[1])) for c in data.get("path") or []]
        ch._path_key = _key_from_json(data.get("path_key"))
        ch.relations = {k: float(v) for k, v in (data.get("relations") or {}).items()}
        ch.met = bool(data.get("met", False))
        ch.alive = bool(data.get("alive", True))
        ch.present = bool(data.get("present", True))
        ch.flags = dict(data.get("flags") or {})
        ch.traits = list(data.get("traits") or [])
        ch.access = list(data.get("access") or ["crew"])
        ch.home = dict(data.get("home") or {})
        ch.job = dict(data.get("job") or {})
        ch.schedule = data.get("schedule", "day")
        ch.goals = list(data.get("goals") or [])
        ch.ai = dict(data.get("ai") or {})
        ch.wants = {k: int(v) for k, v in (data.get("wants") or {}).items()}
        ch.asleep = bool(data.get("asleep", False))
        ch.spawn = dict(data.get("spawn") or {})
        ch.hp = float(data.get("hp", ch.max_hp))
        return ch

    def __repr__(self) -> str:
        return "<Character %s @%s %d,%d>" % (self.id, self.map_id, self.x, self.y)
