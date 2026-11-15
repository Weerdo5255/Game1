"""Needs, vitals and condition effects.

Every character (player included) gets hungry and tired.  Those needs, the
station's air and the place the character is standing in are turned into
*effects* ("Hungry", "Hypoxia", "Cold" ...) whose stat modifiers come from
``data/effects.json``.  This is one of the main couplings in the simulation:
when hydroponics or life support falters, everybody's skills drop, so repairs
slow down, so things get worse.
"""
from __future__ import annotations

from typing import Dict, List

HUNGER_PER_HOUR = 4.2
HUNGER_PER_HOUR_ASLEEP = 2.0
FATIGUE_PER_HOUR = 5.6
SLEEP_RECOVERY_PER_HOUR = 13.0
NATURAL_REGEN = 0.6
MORALE_DRIFT_PER_HOUR = 3.0
BASE_MORALE = 68.0

# Effects the engine itself applies.  ``data/effects.json`` may override the
# numbers or add more (for items and scripts).
DEFAULT_EFFECTS: Dict[str, dict] = {
    "hungry": {"name": "Hungry", "desc": "Your stomach is distracting you.",
               "mods": {"skill.all": -1}, "morale": -10, "bad": True},
    "starving": {"name": "Starving", "desc": "You are weak with hunger.",
                 "mods": {"skill.all": -2, "speed": -0.15}, "hp_per_hour": -1.0, "morale": -25, "bad": True},
    "tired": {"name": "Tired", "desc": "You need sleep.",
              "mods": {"skill.all": -1}, "morale": -6, "bad": True},
    "exhausted": {"name": "Exhausted", "desc": "You can barely stand.",
                  "mods": {"skill.all": -2, "speed": -0.2}, "hp_per_hour": -0.3, "morale": -15, "bad": True},
    "hypoxia": {"name": "Thin Air", "desc": "The air is stale. Thinking is hard.",
                "mods": {"skill.all": -1}, "hp_per_hour": -0.4, "morale": -15, "bad": True},
    "suffocating": {"name": "Suffocating", "desc": "There is not enough oxygen.",
                    "mods": {"skill.all": -3, "speed": -0.25}, "hp_per_hour": -6.0, "morale": -35, "bad": True},
    "parched": {"name": "Parched", "desc": "The tanks are dry. Your tongue feels like felt.",
                "mods": {"skill.all": -2}, "hp_per_hour": -1.2, "morale": -25, "bad": True},
    "cold": {"name": "Freezing", "desc": "The cold is seeping into your bones.",
             "mods": {"skill.all": -1, "speed": -0.1}, "hp_per_hour": -2.5, "morale": -8, "bad": True},
    "demoralised": {"name": "Demoralised", "desc": "What is the point?",
                    "mods": {"work": -0.1}, "bad": True},
    "inspired": {"name": "Inspired", "desc": "Things are looking up.",
                 "mods": {"skill.all": 1}},
    "resting": {"name": "Resting", "desc": "Recovering in a med bed.",
                "mods": {}, "hp_per_hour": 5.0},
}


def is_synthetic(ch) -> bool:
    return "synthetic" in ch.traits or ch.appearance.get("body") == "drone"


def environment_is_cold(world, ch) -> bool:
    gmap = world.maps.get(ch.map_id)
    if gmap is None:
        return False
    if gmap.props.get("cold"):
        return True
    region = gmap.region_at(ch.x, ch.y)
    if region is not None and region.props.get("cold"):
        return True
    return False


def update_character(world, ch, dt_hours: float) -> None:
    """Advance one character's needs, effects, health and morale."""
    if not ch.alive or not ch.present or dt_hours <= 0:
        return
    content = world.content
    now = world.clock.minutes
    synthetic = is_synthetic(ch)

    # timed effects run out
    if ch.effects:
        kept = [e for e in ch.effects if e.get("until") is None or e["until"] > now]
        if len(kept) != len(ch.effects):
            ch.effects = kept
            ch.invalidate()

    if not synthetic:
        if ch.asleep:
            ch.add_need("hunger", HUNGER_PER_HOUR_ASLEEP * dt_hours)
            ch.add_need("fatigue", -SLEEP_RECOVERY_PER_HOUR * dt_hours)
        else:
            ch.add_need("hunger", HUNGER_PER_HOUR * dt_hours)
            ch.add_need("fatigue", FATIGUE_PER_HOUR * dt_hours)

    # condition effects
    auto: List[str] = []
    mods = ch.mods()
    if not synthetic:
        hunger, fatigue = ch.needs["hunger"], ch.needs["fatigue"]
        if hunger >= 92:
            auto.append("starving")
        elif hunger >= 70:
            auto.append("hungry")
        if fatigue >= 92:
            auto.append("exhausted")
        elif fatigue >= 75:
            auto.append("tired")
        if not mods.get("air_supply"):
            air = world.systems.resource("air") if "air" in world.systems.resources else 100.0
            if air < 30:
                auto.append("suffocating")
            elif air < 60:
                auto.append("hypoxia")
        if "water" in world.systems.resources and world.systems.resource("water") <= 0.5:
            auto.append("parched")
        if not mods.get("cold_resist") and environment_is_cold(world, ch):
            auto.append("cold")
        if ch.morale < 25:
            auto.append("demoralised")
        elif ch.morale > 88:
            auto.append("inspired")
        if ch.ai.get("resting"):
            auto.append("resting")
    if auto != ch.auto_effects:
        gained = [e for e in auto if e not in ch.auto_effects]
        lost = [e for e in ch.auto_effects if e not in auto]
        ch.auto_effects = auto
        ch.invalidate()
        if ch.is_player:
            for eff in gained:
                edef = content.effects.get(eff, {})
                world.notify("%s" % edef.get("name", eff), "bad" if edef.get("bad") else "good")
            for eff in lost:
                edef = content.effects.get(eff, {})
                if edef.get("bad"):
                    world.notify("No longer %s" % str(edef.get("name", eff)).lower(), "good")

    # health
    hp_rate = 0.0
    morale_target = BASE_MORALE
    for eff_id in ch.active_effects():
        edef = content.effects.get(eff_id)
        if edef:
            hp_rate += float(edef.get("hp_per_hour", 0.0))
            morale_target += float(edef.get("morale", 0.0))
    if hp_rate >= 0 and not synthetic:
        hp_rate += NATURAL_REGEN * (2.5 if ch.asleep else 1.0)
    if hp_rate > 0:
        ch.heal(hp_rate * dt_hours)
    elif hp_rate < 0:
        ch.hurt(-hp_rate * dt_hours)

    # morale drifts towards a target set by how life is going
    if not synthetic:
        light = world.systems.light_level()
        if light < 0.5:
            morale_target -= 10
        morale_target += float(world.flags.get("morale_mod", 0) or 0)
        morale_target += float(ch.flags.get("morale_mod", 0) or 0)
        morale_target = max(5.0, min(95.0, morale_target))
        step = MORALE_DRIFT_PER_HOUR * dt_hours
        diff = morale_target - ch.morale
        ch.morale += max(-step, min(step, diff))

    if ch.hp <= 0:
        world.collapse(ch)
