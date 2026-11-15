"""Content database: everything loaded from the ``data`` directory.

``Content.load()`` reads every JSON file once.  The rest of the engine only
ever talks to this object, so a different story is simply a different data
directory (``GAME_DATA_DIR`` or ``Content.load("path")``).

Layout::

    data/game.json            title, start position/time, player backgrounds
    data/items.json           item definitions
    data/effects.json         status effects
    data/systems.json         station systems and shared resources
    data/quests.json          quest definitions
    data/goals.json           reusable NPC goal templates
    data/schedules.json       named daily schedules
    data/archetypes.json      role presets used by the character creator
    data/characters/*.json    one file per character
    data/dialogue/*.json      one file per dialogue
    data/maps/*.json          one file per map
    data/story/*.json         director rules (lists) and endings (endings.json)
"""
from __future__ import annotations

import glob
import json
import os
from typing import Any, Dict, List, Optional

from . import config
from .sim.needs import DEFAULT_EFFECTS
from .world.gamemap import GameMap
from .world.items import ItemDef


class Content:
    def __init__(self, data_dir: Optional[str] = None):
        self.dir = data_dir or config.data_dir()
        self.game: Dict[str, Any] = {}
        self.items: Dict[str, ItemDef] = {}
        self.effects: Dict[str, dict] = {}
        self.systems: Dict[str, Any] = {}
        self.quests: Dict[str, dict] = {}
        self.goals: Dict[str, dict] = {}
        self.schedules: Dict[str, list] = {}
        self.archetypes: Dict[str, dict] = {}
        self.characters: Dict[str, dict] = {}
        self.dialogues: Dict[str, dict] = {}
        self.rules: List[dict] = []
        self.endings: Dict[str, dict] = {}
        self.map_ids: List[str] = []
        self.problems: List[str] = []        # files that failed to load
        self._map_cache: Dict[str, dict] = {}

    # -- loading -------------------------------------------------------------------
    @classmethod
    def load(cls, data_dir: Optional[str] = None) -> "Content":
        content = cls(data_dir)
        content.reload()
        return content

    def path(self, *parts: str) -> str:
        return os.path.join(self.dir, *parts)

    def _read(self, path: str, default: Any) -> Any:
        if not os.path.exists(path):
            return default
        try:
            with open(path, "r", encoding="utf8") as fh:
                return json.load(fh)
        except (OSError, ValueError) as exc:
            self.problems.append("%s: %s" % (os.path.relpath(path, self.dir), exc))
            return default

    def reload(self) -> None:
        self.problems = []
        self.game = self._read(self.path("game.json"), {}) or {}
        raw_items = self._read(self.path("items.json"), {}) or {}
        self.items = {k: ItemDef.from_dict(k, v) for k, v in raw_items.items() if not k.startswith("_")}
        self.effects = {k: dict(v) for k, v in DEFAULT_EFFECTS.items()}
        for k, v in (self._read(self.path("effects.json"), {}) or {}).items():
            if not k.startswith("_"):
                merged = dict(self.effects.get(k, {}))
                merged.update(v)
                self.effects[k] = merged
        self.systems = self._read(self.path("systems.json"), {}) or {}
        self.quests = _strip(self._read(self.path("quests.json"), {}) or {})
        self.goals = _strip(self._read(self.path("goals.json"), {}) or {})
        self.schedules = _strip(self._read(self.path("schedules.json"), {}) or {})
        self.archetypes = _strip(self._read(self.path("archetypes.json"), {}) or {})

        self.characters = {}
        for path in sorted(glob.glob(self.path("characters", "*.json"))):
            data = self._read(path, None)
            if isinstance(data, dict):
                cid = str(data.get("id") or os.path.splitext(os.path.basename(path))[0])
                data["id"] = cid
                self.characters[cid] = data

        self.dialogues = {}
        for path in sorted(glob.glob(self.path("dialogue", "*.json"))):
            data = self._read(path, None)
            if isinstance(data, dict):
                did = str(data.get("id") or os.path.splitext(os.path.basename(path))[0])
                self.dialogues[did] = data

        self.rules = []
        self.endings = {}
        for path in sorted(glob.glob(self.path("story", "*.json"))):
            data = self._read(path, None)
            base = os.path.splitext(os.path.basename(path))[0]
            if isinstance(data, list):
                for i, rule in enumerate(data):
                    if isinstance(rule, dict):
                        rule.setdefault("id", "%s_%d" % (base, i))
                        self.rules.append(rule)
            elif isinstance(data, dict) and base == "endings":
                self.endings = _strip(data)

        self._map_cache = {}
        self.map_ids = sorted(os.path.splitext(os.path.basename(p))[0]
                              for p in glob.glob(self.path("maps", "*.json")))

    # -- maps ------------------------------------------------------------------------
    def map_path(self, map_id: str) -> str:
        return self.path("maps", map_id + ".json")

    def character_path(self, char_id: str) -> str:
        return self.path("characters", char_id + ".json")

    def dialogue_path(self, dialogue_id: str) -> str:
        return self.path("dialogue", dialogue_id + ".json")

    def load_map(self, map_id: str) -> Optional[GameMap]:
        """A fresh, independent instance of a map."""
        data = self._map_cache.get(map_id)
        if data is None:
            data = self._read(self.map_path(map_id), None)
            if not isinstance(data, dict):
                return None
            data.setdefault("id", map_id)
            self._map_cache[map_id] = data
        try:
            return GameMap.from_dict(data)
        except (KeyError, TypeError, ValueError) as exc:
            self.problems.append("maps/%s.json: %s" % (map_id, exc))
            return None

    def fresh_maps(self) -> Dict[str, GameMap]:
        maps: Dict[str, GameMap] = {}
        for map_id in self.map_ids:
            gmap = self.load_map(map_id)
            if gmap is not None:
                maps[gmap.id] = gmap
        return maps

    # -- convenience -------------------------------------------------------------------
    @property
    def title(self) -> str:
        return str(self.game.get("title", "Untitled"))

    def backgrounds(self) -> List[dict]:
        return list(self.game.get("backgrounds") or [])


def _strip(data: dict) -> dict:
    """Drop "_comment"-style keys."""
    return {k: v for k, v in data.items() if not str(k).startswith("_")}


def write_json(path: str, data: Any) -> None:
    """Write JSON atomically with stable formatting."""
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf8") as fh:
        json.dump(data, fh, indent=2, ensure_ascii=False)
        fh.write("\n")
    os.replace(tmp, path)
