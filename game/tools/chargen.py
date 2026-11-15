"""Character scaffolding shared by the character creator's GUI and CLI.

``new_character`` turns a name and a role into a complete, valid character
definition: stats from the role's archetype (``data/archetypes.json``), a
random look that suits the role, a free bunk, a job, a daily schedule and a
safe place to stand.  Saving the result into ``data/characters`` is all it
takes to put somebody new on the station - the game picks the file up the
next time it starts (and adds the newcomer to existing saves too).
"""
from __future__ import annotations

import os
import random
import re
from typing import Any, Dict, List, Optional, Tuple

from game.art import chars as chargen_art
from game.content import Content, write_json
from game.engine import palette
from game.world.character import ATTRIBUTES, SKILLS


def slugify(name: str) -> str:
    """"Dr. Kira Vance" -> "kira" (first real word, lower case, ascii)."""
    words = [w for w in re.split(r"[^A-Za-z0-9]+", name) if w and w.lower() not in ("dr", "sgt", "mr", "ms", "mx")]
    base = (words[0] if words else "character").lower()
    return re.sub(r"[^a-z0-9_]", "", base) or "character"


def unique_id(content: Content, base: str) -> str:
    base = base or "character"
    if base not in content.characters and base != "player":
        return base
    n = 2
    while "%s%d" % (base, n) in content.characters:
        n += 1
    return "%s%d" % (base, n)


def random_looks(archetype: Optional[dict] = None, rng: Optional[random.Random] = None) -> dict:
    """A random appearance, constrained by the archetype's ``looks`` lists."""
    rng = rng or random.Random()
    looks = (archetype or {}).get("looks") or {}

    def pick(key: str, default: List[Any]) -> Any:
        options = looks.get(key) or default
        return rng.choice(list(options))

    body = pick("body", ["human"])
    appearance = {
        "body": body,
        "skin": pick("skin", list(palette.SKIN_TONES)[:7]),
        "hair": pick("hair", [h for h in chargen_art.HAIR_STYLES if h not in ("helmet", "cap")]),
        "hair_color": pick("hair_color", list(palette.HAIR_COLORS)),
        "outfit": pick("outfit", ["uniform", "jumpsuit"]),
        "primary": pick("primary", list(palette.CLOTH_COLORS)),
        "secondary": pick("secondary", ["amber", "white", "steel", "cyan", "tan", "red"]),
        "legs": pick("legs", ["slate", "black", "navy", "brown", "steel"]),
        "extras": list(pick("extras", [[], [], [], ["glasses"], ["beard"]])),
    }
    return chargen_art.normalize(appearance)


def spawn_cells_taken(content: Content, map_id: str, ignore: str = "") -> set:
    taken = set()
    for cid, cd in content.characters.items():
        sp = cd.get("spawn") or {}
        if cid != ignore and sp.get("map") == map_id:
            taken.add((int(sp.get("x", -1)), int(sp.get("y", -1))))
    return taken


def free_spawn(content: Content, gmap, near: Optional[Tuple[int, int]] = None, ignore: str = "",
               rng: Optional[random.Random] = None) -> Optional[Tuple[int, int]]:
    """A walkable cell that is not a doorway and not somebody else's start."""
    if gmap is None:
        return None
    taken = spawn_cells_taken(content, gmap.id, ignore)
    blocked = {(o.x, o.y) for o in gmap.objects if o.type in ("portal", "door", "trigger", "spawn")}

    def ok(x: int, y: int) -> bool:
        return not gmap.solid_at(x, y) and (x, y) not in taken and (x, y) not in blocked

    if near is not None:
        for radius in range(0, 12):
            ring = [(near[0] + dx, near[1] + dy) for dx in range(-radius, radius + 1)
                    for dy in range(-radius, radius + 1) if max(abs(dx), abs(dy)) == radius]
            ring.sort(key=lambda c: (abs(c[0] - near[0]) + abs(c[1] - near[1]), c[1] < near[1]))
            for x, y in ring:
                if ok(x, y):
                    return (x, y)
    cells = [(x, y) for y in range(gmap.height) for x in range(gmap.width) if ok(x, y)]
    if not cells:
        return None
    return (rng or random.Random(0)).choice(cells)


def free_bed(content: Content, maps: Dict[str, Any], ignore: str = "") -> str:
    """Id of a bed nobody has claimed ('' if the station is full)."""
    used = {(cd.get("home") or {}).get("bed") for cid, cd in content.characters.items() if cid != ignore}
    used.add(((content.game.get("player") or {}).get("home") or {}).get("bed"))
    medbay = content.game.get("medbay_bed")
    for gmap in maps.values():
        for obj in gmap.objects:
            if obj.type != "bed" or obj.id in used or obj.id == medbay:
                continue
            owner = obj.get("owner")
            if owner in ("", None) or (owner != "player" and owner not in content.characters):
                if "brig" in obj.id or "med" in obj.id:
                    continue
                return obj.id
    return ""


def job_location(content: Content, maps: Dict[str, Any], job: dict):
    """(map_id, (x, y)) of where a job is done, if it has a place."""
    system = (content.systems.get("systems") or {}).get(job.get("system") or "")
    if system and system.get("console"):
        for gmap in maps.values():
            obj = gmap.get_object(system["console"])
            if obj is not None:
                return gmap.id, (obj.x, obj.y + 1)
    if job.get("region"):
        for gmap in maps.values():
            region = gmap.find_region(job["region"])
            if region is not None:
                return gmap.id, region.center
    for marker in job.get("markers") or []:
        for gmap in maps.values():
            obj = gmap.get_object(marker)
            if obj is not None:
                return gmap.id, (obj.x, obj.y)
    return None


def new_character(content: Content, name: str, role: str = "civilian", map_id: Optional[str] = None,
                  pos: Optional[Tuple[int, int]] = None, rng: Optional[random.Random] = None,
                  char_id: Optional[str] = None, maps: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
    """Build a complete character definition (not yet saved)."""
    rng = rng or random.Random()
    maps = maps if maps is not None else content.fresh_maps()
    archetype = dict(content.archetypes.get(role) or content.archetypes.get("civilian") or {})
    cid = unique_id(content, char_id or slugify(name))
    attrs = {a: int((archetype.get("attrs") or {}).get(a, 5)) for a in ATTRIBUTES}
    for _ in range(2):                                 # a little individuality
        up, down = rng.sample(ATTRIBUTES, 2)
        if attrs[up] < 9 and attrs[down] > 2:
            attrs[up] += 1
            attrs[down] -= 1
    skills = {s: int(v) for s, v in (archetype.get("skills") or {}).items() if s in SKILLS}
    job = dict(archetype.get("job") or {})
    place = job_location(content, maps, job)
    if map_id is None:
        map_id = place[0] if place else (content.game.get("start") or {}).get("map") or (
            content.map_ids[0] if content.map_ids else "")
    gmap = maps.get(map_id)
    if pos is None:
        near = place[1] if place and place[0] == map_id else (
            (gmap.width // 2, gmap.height // 2) if gmap is not None else None)
        pos = free_spawn(content, gmap, near=near, rng=rng) or (1, 1)
    data: Dict[str, Any] = {
        "id": cid,
        "name": name.strip() or cid.title(),
        "title": archetype.get("title", role.title()),
        "role": role,
        "faction": archetype.get("faction", ""),
        "bio": "",
        "appearance": random_looks(archetype, rng),
        "attrs": attrs,
        "skills": skills,
        "traits": list(archetype.get("traits") or []),
        "access": list(archetype.get("access") or ["crew"]),
        "spawn": {"map": map_id, "x": int(pos[0]), "y": int(pos[1]), "facing": "down"},
        "home": {},
        "job": job,
        "schedule": archetype.get("schedule", "day"),
        "inventory": dict(archetype.get("inventory") or {}),
        "equipped": dict(archetype.get("equipped") or {}),
        "credits": int(archetype.get("credits", 30)),
        "relations": {"player": 0},
        "goals": [],
        "dialogue": "",
    }
    bed = free_bed(content, maps)
    if bed and "synthetic" not in data["traits"]:
        data["home"] = {"bed": bed}
    return data


def dialogue_stub(data: dict) -> dict:
    """A small starter conversation to edit by hand."""
    name = data.get("name", "Someone")
    title = data.get("title", "crew")
    return {
        "start": [
            {"if": "talked() == 0", "goto": "first"},
            {"goto": "hub"}
        ],
        "nodes": {
            "first": {
                "text": ["%s. %s. You must be the auditor." % (name, title),
                         "Replace this with a proper introduction."],
                "next": "hub"
            },
            "hub": {
                "variants": [
                    {"if": "rel() >= 30", "text": "Good to see you again."},
                    {"text": "What can I do for you?"}
                ],
                "choices": [
                    {"text": "What are you working on?", "goto": "work"},
                    {"text": "Do you need anything?", "if": "wants() != ''", "goto": "needs"},
                    {"text": "A question only an expert would ask.", "skill": ["science", 4], "once": True,
                     "do": ["add_rel(10)", "add_xp(20)"], "goto": "expert"},
                    {"text": "Goodbye.", "goto": "end"}
                ]
            },
            "work": {"text": "{activity()}. {goal_text()}", "next": "hub"},
            "needs": {
                "text": "I could use {wants_count()} x {item_name(wants())}.",
                "choices": [
                    {"text": "Here you go.", "if": "has_item(wants())",
                     "do": ["give_to(npc, wants(), 1)", "add_rel(8)"], "goto": "hub"},
                    {"text": "I will keep an eye out.", "goto": "hub"}
                ]
            },
            "expert": {"text": "Now that is a good question. (Write the answer here.)", "next": "hub"}
        }
    }


def save_character(content: Content, data: dict, old_id: Optional[str] = None) -> str:
    """Write a character file and refresh the content database."""
    cid = data["id"]
    path = content.character_path(cid)
    write_json(path, data)
    if old_id and old_id != cid:
        try:
            os.remove(content.character_path(old_id))
        except OSError:
            pass
    content.reload()
    return path


def delete_character(content: Content, cid: str) -> bool:
    try:
        os.remove(content.character_path(cid))
    except OSError:
        return False
    content.reload()
    return True


def problems_for(content: Content, cid: str) -> List[str]:
    """Validator messages about one character."""
    from game.tools.validate import validate
    report = validate(content)
    key = "characters/%s" % cid
    return [m for m in report.errors + report.warnings if m.startswith(key)]


def summary(data: dict) -> str:
    sp = data.get("spawn") or {}
    skills = ", ".join("%s %d" % (k, v) for k, v in sorted((data.get("skills") or {}).items(), key=lambda kv: -kv[1]))
    job = data.get("job") or {}
    work = job.get("system") or job.get("region") or ("patrol" if job.get("markers") else "none")
    return "%s (%s) - %s\n  starts on %s at (%s, %s); bunk: %s; job: %s; schedule: %s\n  skills: %s" % (
        data.get("name"), data.get("id"), data.get("title"), sp.get("map"), sp.get("x"), sp.get("y"),
        (data.get("home") or {}).get("bed") or "none", work, data.get("schedule"), skills or "none")
