"""Tile-based movement shared by the player and NPCs.

Characters move one tile at a time and slide smoothly between tiles.  A
character *occupies* its destination tile from the moment a step starts.
Stepping onto a portal moves the character to another map; this works the
same for NPCs nobody is watching.
"""
from __future__ import annotations

from typing import Optional, Tuple

from .. import config
from .pathfinding import can_open


def blocked(world, char, x: int, y: int, ignore_chars: bool = False) -> Optional[str]:
    """Why ``char`` cannot enter (x, y): "solid", "locked", "char" or None."""
    gmap = world.maps.get(char.map_id)
    if gmap is None or gmap.solid_at(x, y):
        return "solid"
    for obj in gmap.objects_at(x, y):
        if obj.type in ("door", "portal") and not can_open(char, obj):
            return "locked"
    if not ignore_chars:
        other = world.char_at(char.map_id, x, y, exclude=char)
        if other is not None:
            return "char"
    return None


def start_step(world, char, dx: int, dy: int) -> None:
    """Begin moving one tile (the caller has checked that it is free)."""
    char.prev_x, char.prev_y = char.x, char.y
    char.x += dx
    char.y += dy
    char.move_t = 0.0
    char.facing = config.dir_from_delta(dx, dy, char.facing)


def try_step(world, char, direction: str, ignore_chars: bool = False) -> Optional[str]:
    """Face ``direction`` and step if possible.  Returns the block reason."""
    dx, dy = config.DIRS[direction]
    char.facing = direction
    reason = blocked(world, char, char.x + dx, char.y + dy, ignore_chars)
    if reason is None:
        start_step(world, char, dx, dy)
    return reason


def effective_speed(char) -> float:
    speed = char.speed * (1.0 + char.mods().get("speed", 0.0))
    if char.is_player and char.encumbered:
        speed *= 0.6
    return max(0.8, speed)


def advance(world, char, dt: float) -> float:
    """Continue the current step.  Returns unused time once the character has
    arrived on its tile (0 if it is still in motion)."""
    if char.move_t >= 1.0:
        return dt
    speed = effective_speed(char)
    char.anim_time += dt
    char.move_t += speed * dt
    if char.move_t < 1.0:
        return 0.0
    leftover = (char.move_t - 1.0) / speed
    char.move_t = 1.0
    char.prev_x, char.prev_y = char.x, char.y
    on_arrive(world, char)
    return leftover


def on_arrive(world, char) -> None:
    gmap = world.maps.get(char.map_id)
    if gmap is None:
        return
    for obj in gmap.objects_at(char.x, char.y):
        if obj.type == "portal":
            if can_open(char, obj):
                if char.is_player:
                    # the view fades out first, then calls use_portal itself
                    world.request("portal", obj=obj.id)
                else:
                    use_portal(world, char, obj)
                return
        elif obj.type == "trigger" and char.is_player:
            fire_trigger(world, obj)
    if char.is_player:
        region = gmap.region_at(char.x, char.y)
        rid = region.id if region else ""
        if rid != world.flags.get("_region", ""):
            world.flags["_region"] = rid
            if rid:
                world.events.emit("enter_region:" + rid, region=rid, map=char.map_id)


def fire_trigger(world, obj) -> None:
    if obj.get("once", True) and obj.get("fired"):
        return
    ctx = {"obj": obj.id, "map": obj.map_id}
    if not world.script.eval(obj.get("cond"), ctx):
        return
    obj.set("fired", True)
    world.script.run(obj.get("on_enter"), ctx)


def use_portal(world, char, portal) -> None:
    target = str(portal.get("target_map", ""))
    if target not in world.maps:
        return
    tx, ty = int(portal.get("target_x", 0)), int(portal.get("target_y", 0))
    world.transfer(char, target, tx, ty, portal.get("facing") or None, via=portal.id)


def facing_cell(char) -> Tuple[int, int]:
    dx, dy = config.DIRS[char.facing]
    return char.x + dx, char.y + dy
