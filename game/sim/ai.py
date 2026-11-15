"""NPC goal AI.

Every NPC owns a list of *goals*.  A goal is plain data - a priority and an
ordered list of *steps* - so it can be written in JSON, generated on the fly
and saved in the middle of execution::

    {"id": "decode_signal", "desc": "Decode the signal", "priority": 62,
     "when": "not flag('lab_sealed')",
     "steps": [
        {"do": "acquire", "item": "signal_amplifier", "count": 1},
        {"do": "work", "system": "lab", "var": "signal_progress", "target": 30},
        {"do": "run", "script": ["set_flag('signal_decoded')"]}],
     "on_complete": ["announce('...')"]}

Where goals come from
---------------------
* **routine**  the character's schedule (work shift, meals, leisure, sleep)
* **drives**   goals NPCs set for themselves from their situation: hungry ->
               find food; exhausted -> sleep; hurt -> go to the medbay; a
               system they are responsible for is breaking -> fetch parts
               and repair it; a machine is out of fuel -> restock it
* **authored** personal ambitions listed in the character file
* **story**    goals handed out by scripts (``add_goal``)

Each NPC regularly picks the eligible goal with the highest priority and
executes its current step.  This runs for every NPC on every map each tick,
on screen or not, at the same fidelity: off-screen crew really do walk to
the store room, take the last spare part and repair something with it.

Step verbs: goto, work, repair, acquire, deliver, consume, sleep, recover,
idle, patrol, wait, talk, say, run, leave.  Add new ones with ``@step``.
"""
from __future__ import annotations

import math
from typing import Any, Callable, Dict, List, Optional, Tuple

from ..world import movement
from ..world.pathfinding import can_open
from . import needs as needs_mod

RUNNING, DONE, BLOCKED, FAILED = "running", "done", "blocked", "failed"

THINK_INTERVAL = 2.5       # real seconds between decisions
HYSTERESIS = 8.0           # bonus for the goal already being pursued
SLEEP_HOLD = 25.0          # extra stickiness while asleep
REPAIR_THRESHOLD = 65.0
JURY_CAP = 45.0            # without parts a system can only be patched up to this
DESPERATE_HUNGER = 85.0    # starving people stop asking whose shelf the food is on
PART_WORK_HOURS = 0.5      # time to fit one spare part

HANDLERS: Dict[str, Callable] = {}


def step(name: str):
    def deco(fn):
        HANDLERS[name] = fn
        return fn
    return deco


# ---------------------------------------------------------------------------
# Goal bookkeeping
# ---------------------------------------------------------------------------
def make_goal(gid: str, desc: str, priority: float, steps: List[dict], source: str = "story",
              **extra: Any) -> Dict[str, Any]:
    goal: Dict[str, Any] = {
        "id": gid, "desc": desc, "priority": float(priority), "steps": steps,
        "step": 0, "state": {}, "status": "active", "source": source,
        "retry_at": 0.0, "progress": 0.0,
    }
    goal.update(extra)
    return goal


def _substitute(value: Any, args: Dict[str, Any]) -> Any:
    if isinstance(value, str):
        if value.startswith("$") and value[1:] in args:
            return args[value[1:]]
        for k, v in args.items():
            value = value.replace("$" + k, str(v))
        return value
    if isinstance(value, list):
        return [_substitute(v, args) for v in value]
    if isinstance(value, dict):
        return {k: _substitute(v, args) for k, v in value.items()}
    return value


def instantiate(world, spec: dict, source: str = "authored") -> Optional[Dict[str, Any]]:
    """Turn a goal spec from a content file into a runtime goal.

    ``spec`` is either a full goal (it has "steps") or a reference to a
    template in ``data/goals.json``: ``{"use": "stockpile", "args": {...}}``.
    Any other keys in the spec override the template.
    """
    spec = dict(spec)
    if "use" in spec:
        template = world.content.goals.get(spec.pop("use"))
        if template is None:
            return None
        args = spec.pop("args", {}) or {}
        merged = _substitute(dict(template), args)
        merged.update(_substitute(spec, args))
        spec = merged
    if not spec.get("steps"):
        return None
    gid = str(spec.pop("id", "") or "goal_%d" % (abs(hash(str(spec.get("desc", "")))) % 100000))
    desc = str(spec.pop("desc", gid))
    priority = float(spec.pop("priority", 50))
    steps = [dict(s) for s in spec.pop("steps")]
    spec.pop("args", None)
    return make_goal(gid, desc, priority, steps, source=spec.pop("source", source), **spec)


def find_goal(npc, gid: str) -> Optional[Dict[str, Any]]:
    for g in npc.goals:
        if g["id"] == gid:
            return g
    return None


def add_goal(world, npc, goal: Dict[str, Any]) -> bool:
    """Give ``npc`` a goal.  An active goal with the same id is kept as is."""
    existing = find_goal(npc, goal["id"])
    if existing is not None:
        if existing["status"] == "active":
            return False
        npc.goals.remove(existing)
    npc.goals.append(goal)
    npc.ai["think"] = 0.0
    return True


def remove_goal(npc, gid: str) -> bool:
    goal = find_goal(npc, gid)
    if goal is None:
        return False
    npc.goals.remove(goal)
    if npc.ai.get("goal") == gid:
        npc.ai["goal"] = ""
        npc.ai["think"] = 0.0
        npc.path = []
        if npc.asleep:
            wake(npc)
    return True


def current_goal(npc) -> Optional[Dict[str, Any]]:
    gid = npc.ai.get("goal")
    if not gid:
        return None
    goal = find_goal(npc, gid)
    if goal is None or goal["status"] != "active":
        return None
    return goal


def goal_text(npc) -> str:
    """What the NPC is trying to do, for the crew screen and dialogue."""
    goal = current_goal(npc)
    return goal["desc"] if goal else ""


def wake(npc) -> None:
    npc.asleep = False
    npc.ai.pop("bed", None)
    npc.ai["resting"] = False


# ---------------------------------------------------------------------------
# Per-tick update
# ---------------------------------------------------------------------------
def update_npc(world, npc, dt: float) -> None:
    if not npc.alive or not npc.present:
        return
    ai = npc.ai
    if npc.bark_time > 0:
        npc.bark_time -= dt
        if npc.bark_time <= 0:
            npc.bark = ""
    if ai.get("down_until", 0) > world.clock.minutes:      # collapsed, being treated
        npc.activity = "Recovering in the medbay"
        return
    goal = current_goal(npc)
    ai["think"] = ai.get("think", 0.0) - dt
    if goal is None or ai["think"] <= 0:
        ai["think"] = THINK_INTERVAL
        ensure_routine(world, npc)
        run_drives(world, npc)
        new = choose_goal(world, npc, goal)
        if new is not goal:
            _switch(world, npc, goal, new)
            goal = new
    if goal is not None:
        _run_goal(world, npc, goal, dt)
    else:
        npc.activity = "Idle"
    follow_path(world, npc, dt)


def choose_goal(world, npc, current: Optional[dict]) -> Optional[dict]:
    now = world.clock.minutes
    best, best_p = None, -1e9
    ctx = {"self": npc.id, "npc": npc.id}
    for goal in npc.goals:
        if goal["status"] != "active" or goal.get("retry_at", 0) > now:
            continue
        when = goal.get("when")
        if when and not world.script.eval(when, ctx):
            continue
        p = float(goal["priority"])
        if goal is current:
            p += HYSTERESIS
            if npc.asleep:
                p += SLEEP_HOLD
        if p > best_p:
            best, best_p = goal, p
    return best


def _switch(world, npc, old: Optional[dict], new: Optional[dict]) -> None:
    npc.path = []
    npc._path_key = None
    if npc.asleep:
        first = new["steps"][new["step"]] if new and new["step"] < len(new["steps"]) else None
        if not first or first.get("do") not in ("sleep", "recover"):
            wake(npc)
    npc.ai["resting"] = False
    npc.ai["goal"] = new["id"] if new else ""
    if new is not None and not new.get("announced"):
        new["announced"] = True
        world.events.emit("goal_start:%s:%s" % (npc.id, new["id"]), npc=npc.id, goal=new["id"])
        if new.get("log", new.get("source") in ("authored", "story", "duty")):
            world.add_log("%s: %s." % (npc.name, new["desc"]), "crew")


def _run_goal(world, npc, goal: dict, dt: float) -> None:
    steps = goal["steps"]
    script = world.script
    for _ in range(8):                    # several instant steps may chain in one tick
        i = goal["step"]
        if i >= len(steps):
            _complete(world, npc, goal)
            return
        spec = steps[i]
        handler = HANDLERS.get(spec.get("do", ""))
        state = goal["state"]
        try:
            if handler is None:
                status = DONE
            elif "if" in spec and not state.get("_ok"):
                if script.eval(spec["if"], {"self": npc.id, "npc": npc.id}):
                    state["_ok"] = True
                    status = handler(world, npc, goal, spec, state, dt)
                else:
                    status = DONE
            else:
                status = handler(world, npc, goal, spec, state, dt)
        except Exception as exc:  # noqa: BLE001 - a broken goal in a content file must not stop the world
            script.errors.append("%s: goal %r step %d (%s) failed: %s: %s" % (
                npc.id, goal.get("id"), i, spec.get("do"), type(exc).__name__, exc))
            status = FAILED
        if status == RUNNING:
            return
        if status == DONE:
            goal["step"] = i + 1
            goal["state"] = {}
            continue
        if status == BLOCKED:
            now = world.clock.minutes
            goal["retry_at"] = now + float(spec.get("retry", goal.get("retry", 25)))
            goal["blocks"] = goal.get("blocks", 0) + 1
            goal["state"] = {}
            npc.ai["goal"] = ""
            npc.ai["think"] = 0.0
            npc.path = []
            world.events.emit("goal_blocked:%s:%s" % (npc.id, goal["id"]), npc=npc.id, goal=goal["id"])
            if goal["blocks"] == 1 and goal.get("log", goal.get("source") in ("authored", "story", "duty")):
                why = goal.pop("why", "")
                world.add_log("%s is stuck: %s%s." % (npc.name, goal["desc"], " (%s)" % why if why else ""), "crew")
            if goal.get("max_blocks") and goal["blocks"] >= int(goal["max_blocks"]):
                _fail(world, npc, goal)
            return
        _fail(world, npc, goal)
        return


def _complete(world, npc, goal: dict) -> None:
    now = world.clock.minutes
    ctx = {"self": npc.id, "npc": npc.id}
    goal["status"] = "done"
    goal["done_at"] = now
    goal["progress"] = 1.0
    npc.ai["goal"] = ""
    npc.ai["think"] = 0.0
    world.script.run(goal.get("on_complete"), ctx)
    world.events.emit("goal_done:%s:%s" % (npc.id, goal["id"]), npc=npc.id, goal=goal["id"])
    if goal.get("log", goal.get("source") in ("authored", "story", "duty")):
        world.add_log("%s finished: %s." % (npc.name, goal["desc"]), "crew")
    if goal.get("repeat"):
        goal.update(status="active", step=0, state={}, announced=False,
                    retry_at=now + float(goal.get("cooldown", 60)), blocks=0)
    elif goal.get("source") in ("routine", "drive", "duty") and goal in npc.goals:
        npc.goals.remove(goal)
        if goal.get("source") != "routine":
            npc.ai.setdefault("cd", {})[goal["id"]] = now + float(goal.get("cooldown", 20))


def _fail(world, npc, goal: dict) -> None:
    ctx = {"self": npc.id, "npc": npc.id}
    goal["status"] = "failed"
    npc.ai["goal"] = ""
    npc.ai["think"] = 0.0
    world.script.run(goal.get("on_fail"), ctx)
    world.events.emit("goal_failed:%s:%s" % (npc.id, goal["id"]), npc=npc.id, goal=goal["id"])
    if goal.get("source") in ("routine", "drive", "duty") and goal in npc.goals:
        npc.goals.remove(goal)
        if goal.get("source") != "routine":
            npc.ai.setdefault("cd", {})[goal["id"]] = world.clock.minutes + float(goal.get("cooldown", 45))


# ---------------------------------------------------------------------------
# Movement
# ---------------------------------------------------------------------------
def follow_path(world, npc, dt: float) -> None:
    remaining = dt
    player = world.player
    for _ in range(8):
        if remaining <= 0:
            return
        if npc.moving:
            map_before = npc.map_id
            remaining = movement.advance(world, npc, remaining)
            if npc.moving:
                return
            if npc.map_id != map_before:
                npc.path = []
                npc._path_key = None
                return
        if not npc.path:
            return
        nx, ny = npc.path[0]
        if abs(nx - npc.x) + abs(ny - npc.y) != 1:
            npc.path = []                      # stale path
            npc._path_key = None
            return
        same_map = player is not None and npc.map_id == player.map_id
        patience = 1.1 if (same_map and (nx, ny) == (player.x, player.y)) else 0.45
        reason = movement.blocked(world, npc, nx, ny,
                                  ignore_chars=not same_map or npc.blocked_time > patience)
        if reason is None:
            movement.start_step(world, npc, nx - npc.x, ny - npc.y)
            npc.path.pop(0)
            npc.blocked_time = 0.0
        elif reason == "char":
            npc.blocked_time += remaining
            npc.face_towards(nx, ny)
            return
        else:
            npc.path = []                      # the way is shut: plan again
            npc._path_key = None
            return


def travel(world, npc, map_id: str, cells: List[Tuple[int, int]]) -> str:
    """Head for any of ``cells`` on ``map_id``: "arrived", "moving" or
    "unreachable".  Call it every tick; it plans (and re-plans) lazily."""
    if not cells or map_id not in world.maps:
        return "unreachable"
    nav = world.nav
    if npc.map_id == map_id:
        if npc.pos in cells:
            if npc.moving:
                return "moving"
            npc.path = []
            return "arrived"
        key = (map_id, tuple(cells))
    else:
        portal = nav.next_portal(npc, map_id)
        if portal is None:
            return "unreachable"
        cells = [(portal.x, portal.y)]
        key = (npc.map_id, tuple(cells))
    if not npc.path or getattr(npc, "_path_key", None) != key:
        path = nav.path(npc, npc.map_id, npc.pos, cells)
        if path is None:
            npc.path = []
            npc._path_key = None
            return "unreachable"
        npc.path = path
        npc._path_key = key
    return "moving"


def resolve_target(world, npc, spec: dict, state: dict):
    """Work out where a step wants the NPC to stand.

    Returns ``(map_id, cells, focus)`` - ``focus`` being a cell to face once
    there - or ``None`` if the target does not exist.
    """
    nav = world.nav
    obj = None
    if spec.get("object"):
        obj = world.find_object(str(spec["object"]))
    elif spec.get("system"):
        system = world.systems.get(str(spec["system"]))
        obj = world.find_object(system.console) if system is not None and system.console else None
    elif spec.get("marker"):
        obj = world.find_marker(str(spec["marker"]))
    elif spec.get("bed"):
        obj = world.find_object(str(npc.home.get("bed", "")))
    if obj is not None:
        cells = nav.approach_cells(npc, obj.map_id, obj.x, obj.y)
        return obj.map_id, cells, ((obj.x, obj.y) if cells != [(obj.x, obj.y)] else None)
    if spec.get("char"):
        other = world.characters.get(str(spec["char"]))
        if other is None or not other.present:
            return None
        passable = nav.passable_fn(npc, other.map_id)
        cells = [(other.x + dx, other.y + dy) for dx, dy in ((0, 1), (-1, 0), (1, 0), (0, -1))
                 if passable(other.x + dx, other.y + dy)]
        return other.map_id, cells, (other.x, other.y)
    if spec.get("region"):
        cell = state.get("cell")
        if not cell:
            found = world.find_region(str(spec["region"]), spec.get("map"))
            if found is None:
                return None
            map_id, region = found
            picked = world.random_free_cell(npc, map_id, region)
            if picked is None:
                return None
            cell = state["cell"] = [map_id, picked[0], picked[1]]
        return cell[0], [(cell[1], cell[2])], None
    if "x" in spec and "y" in spec:
        map_id = str(spec.get("map") or npc.map_id)
        if map_id not in world.maps:
            return None
        x, y = int(spec["x"]), int(spec["y"])
        cells = nav.approach_cells(npc, map_id, x, y)
        return map_id, cells, ((x, y) if cells != [(x, y)] else None)
    if spec.get("map") and spec["map"] in world.maps:
        gmap = world.maps[spec["map"]]
        cell = state.get("cell")
        if not cell:
            picked = nav.nearest_free(npc, gmap.id, gmap.width // 2, gmap.height // 2, radius=30)
            if picked is None:
                return None
            cell = state["cell"] = [gmap.id, picked[0], picked[1]]
        return cell[0], [(cell[1], cell[2])], None
    return None


def _go(world, npc, spec: dict, state: dict):
    """Resolve + travel.  Returns (status, focus)."""
    target = resolve_target(world, npc, spec, state)
    if target is None:
        return "unreachable", None
    map_id, cells, focus = target
    return travel(world, npc, map_id, cells), focus


def _in_block(world, spec: dict) -> bool:
    if "until_hour" not in spec:
        return True
    end = float(spec["until_hour"])
    start = float(spec.get("from_hour", (end - 1) % 24))
    return world.clock.is_between(start, end)


def _unreachable(world, npc, goal, spec, state) -> str:
    """Common handling when a destination cannot be reached right now."""
    state["unreach"] = state.get("unreach", 0) + 1
    if goal.get("source") == "routine":
        npc.activity = "Waiting"
        return RUNNING if _in_block(world, spec) else DONE
    goal["why"] = "can't get there"
    return BLOCKED


# ---------------------------------------------------------------------------
# Step handlers
# ---------------------------------------------------------------------------
@step("goto")
def _step_goto(world, npc, goal, spec, state, dt):
    status, focus = _go(world, npc, spec, state)
    if status == "arrived":
        if focus:
            npc.face_towards(*focus)
        return DONE
    if status == "unreachable":
        return _unreachable(world, npc, goal, spec, state)
    npc.activity = spec.get("label") or "Walking"
    return RUNNING


@step("work")
def _step_work(world, npc, goal, spec, state, dt):
    if not _in_block(world, spec):
        return DONE
    system = world.systems.get(str(spec.get("system", "")))
    if system is None:
        npc.activity = "On duty"
        return RUNNING if "until_hour" in spec else DONE
    status, focus = _go(world, npc, spec, state)
    if status == "moving":
        npc.activity = "Heading to %s" % system.name
        return RUNNING
    if status == "unreachable":
        return _unreachable(world, npc, goal, spec, state)
    if focus:
        npc.face_towards(*focus)
    dt_h = world.hours(dt)
    rate = npc.work_rate(str(spec.get("skill") or system.skill))
    system.add_labour(rate * dt_h, npc.id)
    state["hours"] = state.get("hours", 0.0) + dt_h
    npc.activity = "%s %s" % (spec.get("label") or "Working at", system.name)
    var = spec.get("var")
    if var:
        value = float(world.flags.get(var, 0.0) or 0.0)
        value += float(spec.get("rate", 1.0)) * rate * system.efficiency * dt_h
        world.flags[var] = value
        target = float(spec.get("target", 0) or 0)
        if target > 0:
            goal["progress"] = max(0.0, min(1.0, value / target))
            if value >= target:
                return DONE
    if "hours" in spec:
        goal["progress"] = max(goal.get("progress", 0.0), min(1.0, state["hours"] / float(spec["hours"])))
        if state["hours"] >= float(spec["hours"]):
            return DONE
    if spec.get("until") and world.script.eval(spec["until"], {"self": npc.id, "npc": npc.id}):
        return DONE
    return RUNNING


def repair_amount(char) -> float:
    """Condition restored by fitting one spare part."""
    return 8.0 + 1.5 * char.skill("engineering")


@step("repair")
def _step_repair(world, npc, goal, spec, state, dt):
    system = world.systems.get(str(spec.get("system", "")))
    if system is None:
        return DONE
    target = min(float(spec.get("target", 88)), system.cap)
    if system.condition >= target - 0.01:
        _log_repair(world, npc, system, state)
        return DONE
    status, focus = _go(world, npc, {"system": system.id}, state)
    if status == "moving":
        npc.activity = "Heading to %s" % system.name
        return RUNNING
    if status == "unreachable":
        return _unreachable(world, npc, goal, spec, state)
    if focus:
        npc.face_towards(*focus)
    dt_h = world.hours(dt)
    npc.activity = "Repairing %s" % system.name
    part = npc.inventory.find_tag(world.content.items, "part")
    if part:
        state["t"] = state.get("t", 0.0) + dt_h * npc.condition_factor()
        if state["t"] >= PART_WORK_HOURS:
            state["t"] = 0.0
            npc.inventory.remove(part, 1)
            gained = system.repair(repair_amount(npc))
            state["gained"] = state.get("gained", 0.0) + gained
            world.events.emit("repaired:" + system.id, npc=npc.id, system=system.id, amount=gained)
        return RUNNING
    cap = min(float(spec.get("jury_cap", JURY_CAP)), system.cap)
    if system.condition < cap:
        npc.activity = "Jury-rigging %s" % system.name
        state["gained"] = state.get("gained", 0.0) + system.repair(
            (1.5 + 0.35 * npc.skill("engineering")) * npc.condition_factor() * dt_h)
        return RUNNING
    _log_repair(world, npc, system, state)
    return DONE


def _log_repair(world, npc, system, state) -> None:
    if state.get("gained", 0.0) >= 2.0:
        world.add_log("%s repaired %s (integrity now %d%%)." % (npc.name, system.name, round(system.condition)),
                      "crew")


def _container_ok(world, npc, obj, spec: dict) -> bool:
    if obj.get("machine") and obj.id not in (spec.get("from") or ()):
        return False
    if obj.id in (spec.get("exclude") or ()):
        return False
    if spec.get("from") and obj.id not in spec["from"]:
        return False
    if not can_open(npc, obj):
        return False
    if obj.get("restricted") and not has_claim(npc, obj):
        # not theirs: only thieves and the desperate help themselves
        if not (spec.get("steal") or "thief" in npc.traits or _desperate(npc, spec)):
            return False
    return True


def has_claim(npc, obj) -> bool:
    """True if a restricted container is this character's to use: they (or
    their department) own it, or they hold one of its access groups."""
    owner = str(obj.get("owner") or "")
    if not owner or owner in (npc.id, npc.faction, npc.role):
        return True
    mine = npc.access_groups()
    return any(g in mine for g in obj.access_groups())


def _desperate(npc, spec: dict) -> bool:
    """Hunger beats paperwork: the starving take food wherever it is not
    actually locked away."""
    return spec.get("tag") == "food" and npc.needs.get("hunger", 0.0) >= DESPERATE_HUNGER


def find_source(world, npc, spec: dict):
    """Nearest container the NPC may take the wanted item from."""
    items = world.content.items
    item, tag = spec.get("item"), spec.get("tag")
    best, best_cost = None, 1 << 30
    for obj in world.containers():
        inv = world.inventory_of(obj)
        if item:
            if not inv.has(item):
                continue
        elif not inv.find_tag(items, tag):
            continue
        if not _container_ok(world, npc, obj, spec):
            continue
        cost = world.nav.travel_cost(npc, obj.map_id, obj.x, obj.y)
        if cost < best_cost:
            best, best_cost = obj, cost
    return best


@step("acquire")
def _step_acquire(world, npc, goal, spec, state, dt):
    items = world.content.items
    item, tag = spec.get("item"), spec.get("tag")
    count = int(spec.get("count", 1))
    have = npc.inventory.count(item) if item else npc.inventory.count_tag(items, tag)
    want_key = item or str(spec.get("want_item") or "")
    if have >= count:
        if want_key:
            npc.wants.pop(want_key, None)
        return DONE
    need = count - have
    src = world.find_object(state["src"]) if state.get("src") else None
    if src is not None:
        inv = world.inventory_of(src)
        if not (inv.has(item) if item else inv.find_tag(items, tag)):
            src = None
    if src is None:
        src = find_source(world, npc, spec)
        if src is None:
            if spec.get("optional") or (have > 0 and not spec.get("strict")):
                return DONE                     # make do with what we could get
            if want_key:
                npc.wants[want_key] = need
            goal["why"] = "no %s to be found" % (world.item_name(item) if item else tag)
            return BLOCKED
        state["src"] = src.id
    cells = world.nav.approach_cells(npc, src.map_id, src.x, src.y)
    status = travel(world, npc, src.map_id, cells)
    name = world.item_name(item) if item else str(tag)
    if status == "moving":
        npc.activity = "Fetching %s" % name
        return RUNNING
    if status == "unreachable":
        state["src"] = None
        return _unreachable(world, npc, goal, spec, state)
    npc.face_towards(src.x, src.y)
    inv = world.inventory_of(src)
    taken_id = item or inv.find_tag(items, tag)
    if taken_id:
        moved = inv.transfer(npc.inventory, taken_id, need)
        if moved:
            world.events.emit("npc_took:" + npc.id, npc=npc.id, item=taken_id, count=moved, container=src.id)
            if src.get("restricted") and not has_claim(npc, src) and not spec.get("steal") \
                    and "thief" not in npc.traits:
                world.add_log("%s took %s from the %s without asking." % (
                    npc.name, world.item_name(taken_id).lower(), src.name.lower()), "crew")
                world.events.emit("npc_raid:" + src.id, npc=npc.id, item=taken_id, container=src.id)
    state["src"] = None
    return RUNNING


@step("deliver")
def _step_deliver(world, npc, goal, spec, state, dt):
    dest = world.find_object(str(spec.get("to", "")))
    if dest is None:
        return FAILED
    items = world.content.items
    item = spec.get("item") or npc.inventory.find_tag(items, str(spec.get("tag", "")))
    if not item or not npc.inventory.has(item):
        return DONE
    cells = world.nav.approach_cells(npc, dest.map_id, dest.x, dest.y)
    status = travel(world, npc, dest.map_id, cells)
    if status == "moving":
        npc.activity = "Carrying %s" % world.item_name(item)
        return RUNNING
    if status == "unreachable":
        return _unreachable(world, npc, goal, spec, state)
    npc.face_towards(dest.x, dest.y)
    count = spec.get("count", "all")
    qty = npc.inventory.count(item) if count == "all" else min(int(count), npc.inventory.count(item))
    moved = npc.inventory.transfer(world.inventory_of(dest), item, qty)
    world.events.emit("npc_delivered:" + npc.id, npc=npc.id, item=item, count=moved, container=dest.id)
    return DONE


@step("consume")
def _step_consume(world, npc, goal, spec, state, dt):
    items = world.content.items
    item = spec.get("item") or npc.inventory.find_tag(items, str(spec.get("tag", "food")))
    if not item or not npc.inventory.has(item):
        return DONE
    state["t"] = state.get("t", 0.0) + world.minutes(dt)
    npc.activity = "Eating" if "food" in items[item].tags else "Using %s" % items[item].name
    if state["t"] < float(spec.get("minutes", 6)):
        return RUNNING
    world.use_item(npc, item)
    return DONE


@step("sleep")
def _step_sleep(world, npc, goal, spec, state, dt):
    if not npc.asleep:
        bed = world.find_object(str(npc.home.get("bed", ""))) if npc.home.get("bed") else None
        if bed is not None and not state.get("nobed"):
            cells = world.nav.approach_cells(npc, bed.map_id, bed.x, bed.y)
            status = travel(world, npc, bed.map_id, cells)
            if status == "moving":
                npc.activity = "Heading to bed"
                return RUNNING
            if status == "unreachable":
                state["nobed"] = True
                bed = None
            else:
                npc.face_towards(bed.x, bed.y)
        npc.asleep = True
        npc.path = []
        npc.ai["bed"] = bed.id if bed is not None else ""
    npc.activity = "Sleeping"
    fatigue = npc.needs.get("fatigue", 0.0)
    if "until_hour" in spec:
        if not _in_block(world, spec) and fatigue <= 30:
            wake(npc)
            return DONE
        return RUNNING
    if fatigue <= float(spec.get("until_fatigue", 5)):
        wake(npc)
        return DONE
    return RUNNING


@step("recover")
def _step_recover(world, npc, goal, spec, state, dt):
    """Lie down (ideally in the medbay) until health is back."""
    target = float(spec.get("until_hp", 80)) / 100.0 * npc.max_hp
    if npc.hp >= target:
        wake(npc)
        return DONE
    bed = world.find_object(str(spec.get("object", ""))) if spec.get("object") else None
    if bed is not None and not npc.asleep:
        cells = world.nav.approach_cells(npc, bed.map_id, bed.x, bed.y)
        status = travel(world, npc, bed.map_id, cells)
        if status == "moving":
            npc.activity = "Heading to the medbay"
            return RUNNING
        if status == "arrived":
            npc.ai["bed"] = bed.id
            npc.ai["resting"] = True
    npc.asleep = True
    npc.path = []
    npc.activity = "Recovering"
    return RUNNING


@step("idle")
def _step_idle(world, npc, goal, spec, state, dt):
    if not _in_block(world, spec):
        return DONE
    if "minutes" in spec:
        state["t"] = state.get("t", 0.0) + world.minutes(dt)
        if state["t"] >= float(spec["minutes"]):
            return DONE
    label = spec.get("label")
    found = world.find_region(str(spec["region"]), spec.get("map")) if spec.get("region") else None
    if found is not None:
        map_id, region = found
        npc.activity = label or "Relaxing in %s" % region.name
        inside = npc.map_id == map_id and region.contains(npc.x, npc.y)
    else:
        map_id, region = npc.map_id, None
        npc.activity = label or "Taking it easy"
        inside = True
    if npc.moving or npc.path:
        if not inside and state.get("cell"):
            cell = state["cell"]
            if travel(world, npc, cell[0], [(cell[1], cell[2])]) == "unreachable":
                state.pop("cell", None)
        return RUNNING
    if not inside:
        cell = state.get("cell")
        if not cell:
            picked = world.random_free_cell(npc, map_id, region)
            if picked is None:
                return RUNNING if "until_hour" in spec or "minutes" in spec else DONE
            cell = state["cell"] = [map_id, picked[0], picked[1]]
        if travel(world, npc, cell[0], [(cell[1], cell[2])]) == "unreachable":
            state.pop("cell", None)
            state["pause"] = 5.0
        return RUNNING
    state.pop("cell", None)
    state["pause"] = state.get("pause", world.rng.uniform(1.0, 6.0)) - dt
    if state["pause"] > 0:
        return RUNNING
    state["pause"] = world.rng.uniform(5.0, 16.0)
    if world.rng.random() < 0.3:
        npc.facing = world.rng.choice(("up", "down", "left", "right"))
        return RUNNING
    picked = world.random_free_cell(npc, map_id, region, near=(npc.x, npc.y), radius=4)
    if picked is not None:
        travel(world, npc, map_id, [picked])
    return RUNNING


@step("patrol")
def _step_patrol(world, npc, goal, spec, state, dt):
    if not _in_block(world, spec):
        return DONE
    markers = list(spec.get("markers") or [])
    if not markers:
        return DONE if "until_hour" not in spec else RUNNING
    npc.activity = spec.get("label") or "On patrol"
    if state.get("pause", 0) > 0:
        state["pause"] -= dt
        return RUNNING
    i = int(state.get("i", 0)) % len(markers)
    marker = world.find_marker(markers[i])
    if marker is None:
        state["i"] = i + 1
        return RUNNING
    cells = world.nav.approach_cells(npc, marker.map_id, marker.x, marker.y)
    status = travel(world, npc, marker.map_id, cells)
    if status == "arrived" or status == "unreachable":
        state["i"] = i + 1
        state["pause"] = world.rng.uniform(4.0, 9.0) if status == "arrived" else 2.0
        if "laps" in spec and state["i"] >= len(markers) * int(spec["laps"]):
            return DONE
    return RUNNING


@step("wait")
def _step_wait(world, npc, goal, spec, state, dt):
    # a wait may name a place ("object", "region", "marker" ...): the NPC goes back
    # there whenever something more urgent (a meal, sleep) has pulled them away
    if any(k in spec for k in ("object", "region", "marker", "system", "char", "x")):
        status, focus = _go(world, npc, spec, state)
        if status == "moving":
            npc.activity = "Walking"
            return RUNNING
        if status == "arrived" and focus:
            npc.face_towards(*focus)
    if spec.get("until"):
        if world.script.eval(spec["until"], {"self": npc.id, "npc": npc.id}):
            return DONE
        npc.activity = spec.get("label") or "Waiting"
        return RUNNING
    state["t"] = state.get("t", 0.0) + world.minutes(dt)
    npc.activity = spec.get("label") or "Waiting"
    return DONE if state["t"] >= float(spec.get("minutes", 5)) else RUNNING


@step("talk")
def _step_talk(world, npc, goal, spec, state, dt):
    other = world.characters.get(str(spec.get("char", "")))
    if other is None or not other.present or not other.alive:
        return DONE if spec.get("optional", True) else FAILED
    status, focus = _go(world, npc, {"char": other.id}, state)
    if status == "moving":
        npc.activity = "Looking for %s" % other.short_name
        return RUNNING
    if status == "unreachable":
        return _unreachable(world, npc, goal, spec, state)
    npc.face_towards(other.x, other.y)
    if other.is_player:
        # walk up to the player and open a conversation
        if world.busy:
            return RUNNING
        world.request("dialogue", npc=npc.id, dialogue=spec.get("dialogue") or npc.dialogue,
                      node=spec.get("node"))
        return DONE
    if not state.get("begun"):
        state["begun"] = True
        if not other.asleep and not other.moving:
            other.face_towards(npc.x, npc.y)
        npc.say(spec.get("line") or "...", 4.0)
    state["t"] = state.get("t", 0.0) + world.minutes(dt)
    npc.activity = "Talking with %s" % other.short_name
    if state["t"] < float(spec.get("minutes", 5)):
        return RUNNING
    world.script.run(spec.get("effects"), {"self": npc.id, "npc": other.id})
    return DONE


@step("say")
def _step_say(world, npc, goal, spec, state, dt):
    npc.say(world.text(str(spec.get("text", "")), {"self": npc.id, "npc": npc.id}), float(spec.get("seconds", 4)))
    return DONE


@step("run")
def _step_run(world, npc, goal, spec, state, dt):
    world.script.run(spec.get("script"), {"self": npc.id, "npc": npc.id})
    return DONE


@step("leave")
def _step_leave(world, npc, goal, spec, state, dt):
    world.remove_character(npc, spec.get("reason", "left"))
    return DONE


# ---------------------------------------------------------------------------
# Routine (schedule)
# ---------------------------------------------------------------------------
DEFAULT_SCHEDULE = [
    {"from": 6, "to": 7, "do": "meal"},
    {"from": 7, "to": 12, "do": "work"},
    {"from": 12, "to": 13, "do": "meal"},
    {"from": 13, "to": 18, "do": "work"},
    {"from": 18, "to": 22, "do": "leisure"},
    {"from": 22, "to": 6, "do": "sleep"},
]


def schedule_blocks(world, npc) -> List[dict]:
    sched = npc.schedule
    if isinstance(sched, str):
        sched = world.content.schedules.get(sched)
    return sched if isinstance(sched, list) and sched else DEFAULT_SCHEDULE


def current_block(world, npc) -> Optional[dict]:
    clock = world.clock
    for block in schedule_blocks(world, npc):
        if clock.is_between(float(block.get("from", 0)), float(block.get("to", 24))):
            return block
    return None


def ensure_routine(world, npc) -> None:
    block = current_block(world, npc)
    gid = "routine:%s:%s" % (block.get("do"), block.get("from")) if block else "routine:none"
    existing = [g for g in npc.goals if g.get("source") == "routine"]
    if existing and existing[0]["id"] == gid and existing[0]["status"] == "active":
        return
    for g in existing:
        npc.goals.remove(g)
        if npc.ai.get("goal") == g["id"]:
            npc.ai["goal"] = ""
    if block is not None:
        npc.goals.append(_routine_goal(world, npc, block, gid))


def _routine_goal(world, npc, block: dict, gid: str) -> dict:
    kind = str(block.get("do", "idle"))
    frm, to = float(block.get("from", 0)), float(block.get("to", 24))
    window = {"from_hour": frm, "until_hour": to}
    job = npc.job or {}
    mess = block.get("region") or world.content.game.get("mess_region", "mess_hall")
    if kind == "work":
        system = block.get("system") or job.get("system")
        if system and world.systems.get(system) is not None:
            steps = [dict(window, do="work", system=system)]
        elif block.get("markers") or job.get("markers"):
            steps = [dict(window, do="patrol", markers=block.get("markers") or job.get("markers"),
                          label=job.get("label") or "On patrol")]
        else:
            steps = [dict(window, do="idle", region=block.get("region") or job.get("region") or "",
                          label=job.get("label") or "On duty")]
        return make_goal(gid, "On shift", float(block.get("priority", 40)), steps, source="routine")
    if kind == "meal":
        steps = []
        if npc.needs.get("hunger", 0) >= 25 and not needs_mod.is_synthetic(npc):
            steps.append({"do": "acquire", "tag": "food", "count": 1, "optional": True})
            steps.append({"do": "consume", "tag": "food"})
        steps.append(dict(window, do="idle", region=mess, label="Having a meal break"))
        return make_goal(gid, "Meal break", float(block.get("priority", 44)), steps, source="routine")
    if kind == "sleep":
        if needs_mod.is_synthetic(npc):
            steps = [dict(window, do="idle", region=block.get("region") or "", label="Recharging")]
        else:
            steps = [dict(window, do="sleep")]
        return make_goal(gid, "Sleeping", float(block.get("priority", 60)), steps, source="routine")
    if kind == "patrol":
        steps = [dict(window, do="patrol", markers=block.get("markers") or job.get("markers") or [])]
        return make_goal(gid, "On patrol", float(block.get("priority", 40)), steps, source="routine")
    region = block.get("region") or (mess if kind == "leisure" else "")
    label = block.get("label") or ("" if kind == "leisure" else "Off duty")
    steps = [dict(window, do="idle", region=region, label=label)]
    return make_goal(gid, "Off duty", float(block.get("priority", 20)), steps, source="routine")


# ---------------------------------------------------------------------------
# Drives: goals NPCs give themselves
# ---------------------------------------------------------------------------
def run_drives(world, npc) -> None:
    synthetic = needs_mod.is_synthetic(npc)
    if not synthetic:
        _drive_hunger(world, npc)
        _drive_rest(world, npc)
        _drive_health(world, npc)
    _drive_duty(world, npc)


def _set_drive(world, npc, gid: str, desc: str, priority: float, steps: List[dict],
               source: str = "drive", **extra) -> None:
    goal = find_goal(npc, gid)
    if goal is not None and goal["status"] == "active":
        goal["priority"] = float(priority)        # urgency changes over time
        return
    if npc.ai.get("cd", {}).get(gid, 0) > world.clock.minutes:
        return                                    # just did this; don't thrash
    add_goal(world, npc, make_goal(gid, desc, priority, steps, source=source, **extra))


def _drive_hunger(world, npc) -> None:
    hunger = npc.needs.get("hunger", 0.0)
    if hunger < 55:
        remove = find_goal(npc, "eat")
        if remove is not None and npc.ai.get("goal") != "eat":
            npc.goals.remove(remove)
        return
    priority = min(95.0, 50.0 + (hunger - 55.0) * 1.1)
    _set_drive(world, npc, "eat", "Find something to eat", priority, [
        {"do": "acquire", "tag": "food", "count": 1, "want_item": world.content.game.get("food_item", "")},
        {"do": "consume", "tag": "food"},
    ], retry=40)


def _drive_rest(world, npc) -> None:
    fatigue = npc.needs.get("fatigue", 0.0)
    if fatigue >= 93:
        _set_drive(world, npc, "rest", "Get some sleep", 86.0, [{"do": "sleep", "until_fatigue": 25}])


def _drive_health(world, npc) -> None:
    if npc.hp < npc.max_hp * 0.45:
        bed = world.content.game.get("medbay_bed", "")
        _set_drive(world, npc, "heal", "Get treated in the medbay", 82.0,
                   [{"do": "recover", "object": bed, "until_hp": 80}])


def _drive_duty(world, npc) -> None:
    """Look after the systems this character is responsible for."""
    job = npc.job or {}
    maintain = list(job.get("maintain") or [])
    if job.get("duty") == "maintain" and job.get("system") and job["system"] not in maintain:
        maintain.append(job["system"])
    systems = world.systems
    if maintain and not any(g["id"].startswith("repair:") and g["status"] == "active" for g in npc.goals):
        worst, worst_score = None, 0.0
        threshold = float(job.get("repair_below", REPAIR_THRESHOLD))
        part_item = world.content.game.get("part_item", "")
        have_parts = npc.inventory.count_tag(world.content.items, "part") > 0 \
            or find_source(world, npc, {"tag": "part"}) is not None
        needy = 0
        for sid in maintain:
            s = systems.get(sid)
            if s is None or s.condition >= min(threshold, s.cap - 12):
                continue                      # healthy, or as good as it can get for now
            needy += 1
            if not have_parts and s.condition >= JURY_CAP - 2:
                continue                      # nothing useful can be done without parts
            score = (threshold - s.condition) * (1.6 if s.kind == "generator" or s.priority <= 2 else 1.0)
            if score > worst_score:
                worst, worst_score = s, score
        if needy and not have_parts and part_item:
            # remember what is missing so the player (or a script) can help
            if part_item not in npc.wants:
                world.add_log("%s needs spare parts and cannot find any." % npc.name, "crew")
            npc.wants[part_item] = max(1, needy)
        elif part_item:
            npc.wants.pop(part_item, None)
        if worst is not None:
            parts = int(max(1, min(3, math.ceil((88.0 - worst.condition) / max(1.0, repair_amount(npc))))))
            priority = min(90.0, 58.0 + worst_score * 0.6)
            _set_drive(world, npc, "repair:" + worst.id, "Repair %s" % worst.name, priority, [
                {"do": "acquire", "tag": "part", "count": parts, "optional": True},
                {"do": "repair", "system": worst.id, "target": 88},
            ], source="duty", system=worst.id, log=False)
    elif maintain:
        # stop fussing once the system is healthy again
        for g in list(npc.goals):
            if g["id"].startswith("repair:") and g["status"] == "active" and npc.ai.get("goal") != g["id"]:
                s = systems.get(g.get("system", ""))
                if s is not None and s.condition >= 88:
                    npc.goals.remove(g)

    for sid in job.get("restock") or []:
        s = systems.get(sid)
        if s is None:
            continue
        gid = "restock:" + sid
        if find_goal(npc, gid) is not None:
            continue
        if s.fuel:
            hopper = world.find_object(str(s.fuel.get("hopper", "")))
            item = str(s.fuel.get("item", ""))
            if hopper is not None and world.inventory_of(hopper).count(item) < 1 and s.fuel_left < 8:
                _set_drive(world, npc, gid, "Refuel %s" % s.name, 78.0, [
                    {"do": "acquire", "item": item, "count": 2, "exclude": [hopper.id]},
                    {"do": "deliver", "item": item, "to": hopper.id},
                ], source="duty", retry=60)
        for recipe in s.makes:
            needs = recipe.get("needs") or {}
            src = world.find_object(str(recipe.get("from", "")))
            if not needs or src is None:
                continue
            inv = world.inventory_of(src)
            for item, qty in needs.items():
                if inv.count(item) < int(qty):
                    _set_drive(world, npc, gid, "Restock %s" % s.name, 52.0, [
                        {"do": "acquire", "item": item, "count": int(qty) * 2, "exclude": [src.id]},
                        {"do": "deliver", "item": item, "to": src.id},
                    ], source="duty", retry=90)
                    break
