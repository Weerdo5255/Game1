"""Save slots.

A save file is the JSON produced by ``World.to_dict`` wrapped with a small
header (player name, place, clock, play time) that the load menu can show
without reading the whole world.  Files live in the ``saves`` directory:
``autosave.json``, ``quicksave.json`` and ``slot1.json`` ... ``slot6.json``.
"""
from __future__ import annotations

import json
import os
import time
from typing import Any, Dict, List, Optional

from . import config

SLOTS = ["slot1", "slot2", "slot3", "slot4", "slot5", "slot6"]
SPECIAL = ["autosave", "quicksave"]


def slot_path(slot: str) -> str:
    return os.path.join(config.save_dir(), slot + ".json")


def slot_label(slot: str) -> str:
    if slot == "autosave":
        return "Autosave"
    if slot == "quicksave":
        return "Quick save"
    return "Slot %s" % slot.replace("slot", "")


def header_for(world) -> Dict[str, Any]:
    player = world.player
    gmap = world.maps.get(player.map_id) if player else None
    return {
        "name": player.name if player else "",
        "level": player.level if player else 1,
        "place": gmap.place_name(player.x, player.y) if gmap and player else "",
        "map": gmap.name if gmap else "",
        "stamp": world.clock.stamp(),
        "playtime": world.playtime,
        "saved_at": time.time(),
        "title": world.content.title,
    }


def save_game(world, slot: str) -> str:
    """Write the world to a slot; returns the file path."""
    os.makedirs(config.save_dir(), exist_ok=True)
    data = {"header": header_for(world), "world": world.to_dict()}
    path = slot_path(slot)
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf8") as fh:
        json.dump(data, fh, separators=(",", ":"))
    os.replace(tmp, path)
    return path


def load_game(content, slot: str):
    """Load a slot and return a ready-to-run World (raises on failure)."""
    from .world.world import World
    with open(slot_path(slot), "r", encoding="utf8") as fh:
        data = json.load(fh)
    version = int((data.get("world") or {}).get("version", 0))
    if version > config.SAVE_VERSION:
        raise ValueError("save file is from a newer version of the game")
    return World.from_dict(content, data["world"])


def read_header(slot: str) -> Optional[Dict[str, Any]]:
    path = slot_path(slot)
    if not os.path.exists(path):
        return None
    try:
        with open(path, "r", encoding="utf8") as fh:
            # the header is first in the file; read just enough to find it
            head = fh.read(4096)
        start = head.find('"header":')
        if start == -1:
            return None
        decoder = json.JSONDecoder()
        obj, _ = decoder.raw_decode(head[start + len('"header":'):].lstrip())
        return obj if isinstance(obj, dict) else None
    except (OSError, ValueError):
        return None


def list_saves(include_special: bool = True) -> List[Dict[str, Any]]:
    """Every slot with its header (``None`` when empty), newest info included."""
    out = []
    for slot in (SPECIAL if include_special else []) + SLOTS:
        out.append({"slot": slot, "label": slot_label(slot), "header": read_header(slot)})
    return out


def latest_slot() -> Optional[str]:
    best, best_t = None, -1.0
    for entry in list_saves():
        header = entry["header"]
        if header and float(header.get("saved_at", 0)) > best_t:
            best, best_t = entry["slot"], float(header.get("saved_at", 0))
    return best


def delete_save(slot: str) -> None:
    try:
        os.remove(slot_path(slot))
    except OSError:
        pass


def format_playtime(seconds: float) -> str:
    seconds = int(seconds)
    return "%d:%02d" % (seconds // 3600, (seconds % 3600) // 60)
