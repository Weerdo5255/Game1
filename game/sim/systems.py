"""Interdependent station systems.

``data/systems.json`` describes shared *resources* (water, air ...) and the
*systems* that produce and consume them.  Every system has a condition that
decays, an efficiency derived from what it depends on, and optional item
production.  The dependencies form loops, for example:

    reactor --power--> life support --air--> crew health
    reactor --power--> hydroponics --rations--> crew hunger --> crew work rate
    crew work rate --labour/repairs--> reactor

Efficiency of a consumer is the product of four factors:

* condition   worn machinery works worse (1.0 above 75%, falling to 0);
              an optional ``condition_cap`` limits how far ordinary repairs
              can restore it until a script lifts the cap (a missing part)
* power       its share of the supply; allocated strictly by ``priority``
* labour      how much skilled operator time it has had lately
* inputs      availability of the resources it consumes

Example system definition::

    "hydroponics": {
      "name": "Hydroponics", "map": "hydroponics", "console": "hydro_console",
      "power": 18, "priority": 3, "decay": 0.3, "start_condition": 85,
      "skill": "science", "operator": 0.6,
      "consumes": {"water": 5}, "produces": {"air": 0.8},
      "makes": [{"item": "ration_pack", "hours": 2.5, "into": "galley_dispenser",
                 "max_stock": 14}]
    }

Rates are per game hour at 100% efficiency.  Nothing here knows about
specific systems: add a new entry to the JSON file and it joins the network.
"""
from __future__ import annotations

from typing import Any, Dict, List, Optional

STATUS_ORDER = ["offline", "critical", "degraded", "ok"]

LABOUR_GAIN = 0.35        # labour gained per hour of work at rate 1.0
LABOUR_DECAY = 0.06       # labour lost per hour
LABOUR_CAP = 1.2


def condition_factor(condition: float) -> float:
    if condition >= 75:
        return 1.0
    if condition >= 25:
        return 0.35 + 0.65 * (condition - 25) / 50.0
    if condition > 5:
        return 0.35 * (condition - 5) / 20.0
    return 0.0


def status_for(condition: float) -> str:
    if condition <= 5:
        return "offline"
    if condition < 30:
        return "critical"
    if condition < 60:
        return "degraded"
    return "ok"


class StationSystem:
    def __init__(self, sid: str, data: dict):
        self.id = sid
        self.name = str(data.get("name", sid))
        self.desc = str(data.get("desc", ""))
        self.map = str(data.get("map", ""))
        self.console = str(data.get("console", ""))
        self.kind = str(data.get("kind", "consumer"))        # "generator" or "consumer"
        self.power_out = float(data.get("power_out", 0.0))
        self.power = float(data.get("power", 0.0))
        self.base_priority = int(data.get("priority", 5))
        self.decay = float(data.get("decay", 0.3))
        self.skill = str(data.get("skill", "engineering"))
        self.operator = float(data.get("operator", 0.0))     # share of output needing labour
        self.consumes: Dict[str, float] = dict(data.get("consumes") or {})
        self.produces: Dict[str, float] = dict(data.get("produces") or {})
        self.makes: List[dict] = [dict(m) for m in data.get("makes") or []]
        self.fuel: Optional[dict] = dict(data["fuel"]) if data.get("fuel") else None
        self.essential = bool(data.get("essential", False))  # cannot be switched off
        self.lights = bool(data.get("lights", False))        # drives station lighting

        # --- state ---
        self.cap = float(data.get("condition_cap", 100.0))   # repairs cannot exceed this
        self.condition = min(self.cap, float(data.get("start_condition", 100.0)))
        self.enabled = bool(data.get("enabled", True))
        self.priority = self.base_priority
        self.labour = float(data.get("start_labour", 0.6))
        self.efficiency = 0.0
        self.power_ratio = 1.0
        self.output = 0.0                                    # kW produced (generators)
        self.status = status_for(self.condition)
        self.fuel_left = float((self.fuel or {}).get("start_hours", 0.0))
        self.progress: List[float] = [0.0 for _ in self.makes]
        self.blocked = ""                                    # why production is stalled
        self.operator_id = ""                                # who is working it right now
        self._operator_ttl = 0.0

    # -- actions ---------------------------------------------------------------
    def add_labour(self, amount: float, operator_id: str = "") -> None:
        self.labour = min(LABOUR_CAP, self.labour + amount * LABOUR_GAIN)
        if operator_id:
            self.operator_id = operator_id
            self._operator_ttl = 0.25

    def repair(self, amount: float) -> float:
        before = self.condition
        limit = max(self.cap, before) if amount > 0 else 100.0
        self.condition = max(0.0, min(limit, self.condition + amount))
        return self.condition - before

    def damage(self, amount: float) -> float:
        return -self.repair(-amount)

    @property
    def labour_factor(self) -> float:
        return (1.0 - self.operator) + self.operator * min(1.0, self.labour)

    def to_dict(self) -> dict:
        return {"condition": self.condition, "cap": self.cap, "enabled": self.enabled, "priority": self.priority,
                "labour": self.labour, "efficiency": self.efficiency, "power_ratio": self.power_ratio,
                "output": self.output, "status": self.status, "fuel_left": self.fuel_left,
                "progress": list(self.progress), "blocked": self.blocked}

    def load_dict(self, data: dict) -> None:
        self.condition = float(data.get("condition", self.condition))
        self.cap = float(data.get("cap", self.cap))
        self.enabled = bool(data.get("enabled", self.enabled))
        self.priority = int(data.get("priority", self.priority))
        self.labour = float(data.get("labour", self.labour))
        self.efficiency = float(data.get("efficiency", 0.0))
        self.power_ratio = float(data.get("power_ratio", 1.0))
        self.output = float(data.get("output", 0.0))
        self.status = str(data.get("status", status_for(self.condition)))
        self.fuel_left = float(data.get("fuel_left", self.fuel_left))
        progress = list(data.get("progress") or [])
        self.progress = [float(progress[i]) if i < len(progress) else 0.0 for i in range(len(self.makes))]
        self.blocked = str(data.get("blocked", ""))


class SystemNetwork:
    def __init__(self, data: Optional[dict] = None):
        data = data or {}
        self.resource_defs: Dict[str, dict] = {k: dict(v) for k, v in (data.get("resources") or {}).items()}
        self.resources: Dict[str, float] = {k: float(v.get("start", 0.0)) for k, v in self.resource_defs.items()}
        self.resource_levels: Dict[str, str] = {k: "ok" for k in self.resource_defs}
        self.systems: Dict[str, StationSystem] = {
            sid: StationSystem(sid, sd) for sid, sd in (data.get("systems") or {}).items()}
        self.power_supply = 0.0
        self.power_demand = 0.0
        self.power_used = 0.0

    # -- queries -----------------------------------------------------------------
    def get(self, sid: str) -> Optional[StationSystem]:
        return self.systems.get(sid)

    def resource(self, name: str) -> float:
        return self.resources.get(name, 0.0)

    def resource_cap(self, name: str) -> float:
        return float(self.resource_defs.get(name, {}).get("cap", 100.0))

    def resource_frac(self, name: str) -> float:
        cap = self.resource_cap(name)
        return self.resources.get(name, 0.0) / cap if cap > 0 else 0.0

    def add_resource(self, name: str, amount: float) -> None:
        if name in self.resources:
            self.resources[name] = max(0.0, min(self.resource_cap(name), self.resources[name] + amount))

    @property
    def brownout(self) -> bool:
        return self.power_demand > self.power_supply + 0.01

    def light_level(self) -> float:
        """0..1 lighting level, from whichever system is flagged ``lights``."""
        levels = [s.efficiency for s in self.systems.values() if s.lights]
        if not levels:
            return 1.0 if not self.brownout else 0.6
        return max(0.0, min(1.0, sum(levels) / len(levels)))

    def generators(self) -> List[StationSystem]:
        return [s for s in self.systems.values() if s.kind == "generator"]

    def consumers(self) -> List[StationSystem]:
        return sorted((s for s in self.systems.values() if s.kind != "generator"),
                      key=lambda s: (s.priority, s.id))

    # -- simulation ----------------------------------------------------------------
    def tick(self, world, dt_hours: float) -> None:
        """Advance every system by ``dt_hours`` of game time."""
        if dt_hours <= 0:
            return
        res = self.resources

        # 1. generation (burning fuel from the hopper container)
        supply = 0.0
        for s in self.generators():
            fueled = True
            if s.fuel and s.enabled:
                s.fuel_left -= dt_hours
                if s.fuel_left <= 0:
                    hopper = world.find_object(str(s.fuel.get("hopper", "")))
                    inv = world.inventory_of(hopper) if hopper is not None else None
                    item = str(s.fuel.get("item", ""))
                    if inv is not None and inv.remove(item, 1):
                        s.fuel_left += float(s.fuel.get("hours", 24))
                        world.events.emit("fuel_used:" + s.id, system=s.id)
                    else:
                        s.fuel_left = 0.0
                        fueled = False
            s.blocked = "" if fueled else "no fuel"
            cf = condition_factor(s.condition)
            s.efficiency = cf * s.labour_factor if (s.enabled and fueled) else 0.0
            s.output = s.power_out * s.efficiency
            s.power_ratio = 1.0
            supply += s.output
        self.power_supply = supply

        # 2. distribution by priority
        remaining = supply
        demand = 0.0
        consumers = self.consumers()
        for s in consumers:
            if not s.enabled or s.power <= 0:
                s.power_ratio = 1.0 if s.enabled else 0.0
                continue
            demand += s.power
            give = min(s.power, remaining)
            s.power_ratio = give / s.power
            remaining -= give
        self.power_demand = demand
        self.power_used = supply - remaining

        # 3. consumption / production
        for s in consumers:
            if not s.enabled:
                s.efficiency = 0.0
                continue
            eff = condition_factor(s.condition) * s.power_ratio * s.labour_factor
            for name, rate in s.consumes.items():
                need = rate * dt_hours
                if need > 0:
                    have = res.get(name, 0.0)
                    if have < need:
                        eff *= have / need
            s.efficiency = eff
            for name, rate in s.consumes.items():
                self.add_resource(name, -rate * eff * dt_hours)
            for name, rate in s.produces.items():
                self.add_resource(name, rate * eff * dt_hours)
            if s.makes:
                self._produce_items(world, s, eff, dt_hours)

        # 4. wear, labour fading, status changes
        for s in self.systems.values():
            if s.decay > 0:
                s.condition = max(0.0, s.condition - s.decay * dt_hours * (1.0 if s.enabled else 0.3))
            s.labour = max(0.0, s.labour - LABOUR_DECAY * dt_hours)
            if s._operator_ttl > 0:
                s._operator_ttl -= dt_hours
                if s._operator_ttl <= 0:
                    s.operator_id = ""
            status = status_for(s.condition)
            if status != s.status:
                old, s.status = s.status, status
                world.events.emit("system:%s:%s" % (s.id, status), system=s.id, status=status, old=old)
                world.events.emit("system_status", system=s.id, status=status, old=old)

        # 5. the crew breathes and drinks
        crew = world.crew_count()
        for name, rdef in self.resource_defs.items():
            per = float(rdef.get("per_crew", 0.0))
            if per:
                self.add_resource(name, per * crew * dt_hours)
            frac = self.resource_frac(name)
            level = "critical" if frac <= float(rdef.get("critical", 0.15)) else \
                "low" if frac <= float(rdef.get("low", 0.35)) else "ok"
            if level != self.resource_levels.get(name):
                old = self.resource_levels.get(name, "ok")
                self.resource_levels[name] = level
                world.events.emit("resource:%s:%s" % (name, level), resource=name, level=level, old=old)
                world.events.emit("resource_level", resource=name, level=level, old=old)

    def _produce_items(self, world, s: StationSystem, eff: float, dt_hours: float) -> None:
        s.blocked = ""
        for i, recipe in enumerate(s.makes):
            hours = float(recipe.get("hours", 1.0))
            item = str(recipe.get("item", ""))
            dest = world.find_object(str(recipe.get("into", "")))
            dest_inv = world.inventory_of(dest) if dest is not None else None
            if dest_inv is None or not item:
                s.blocked = "no output bin"
                continue
            max_stock = recipe.get("max_stock")
            if max_stock is not None and dest_inv.count(item) >= int(max_stock):
                s.blocked = "output full"
                continue
            needs = recipe.get("needs") or {}
            src_inv = None
            if needs:
                src = world.find_object(str(recipe.get("from", "")))
                src_inv = world.inventory_of(src) if src is not None else None
                if src_inv is None or any(not src_inv.has(k, int(v)) for k, v in needs.items()):
                    s.blocked = "needs " + ", ".join(world.item_name(k) for k in needs)
                    continue
            s.progress[i] += eff * dt_hours / max(0.01, hours)
            while s.progress[i] >= 1.0:
                if needs:
                    if any(not src_inv.has(k, int(v)) for k, v in needs.items()):
                        s.progress[i] = 0.99
                        break
                    for k, v in needs.items():
                        src_inv.remove(k, int(v))
                s.progress[i] -= 1.0
                dest_inv.add(item, int(recipe.get("count", 1)))
                world.events.emit("produced:" + item, system=s.id, item=item)
                if max_stock is not None and dest_inv.count(item) >= int(max_stock):
                    s.progress[i] = min(s.progress[i], 0.0)
                    break

    # -- save / load ---------------------------------------------------------------
    def to_dict(self) -> dict:
        return {
            "resources": dict(self.resources),
            "levels": dict(self.resource_levels),
            "systems": {sid: s.to_dict() for sid, s in self.systems.items()},
            "power": [self.power_supply, self.power_demand, self.power_used],
        }

    def load_dict(self, data: Optional[dict]) -> None:
        data = data or {}
        for name, value in (data.get("resources") or {}).items():
            if name in self.resources:
                self.resources[name] = float(value)
        self.resource_levels.update(data.get("levels") or {})
        for sid, sd in (data.get("systems") or {}).items():
            if sid in self.systems:
                self.systems[sid].load_dict(sd)
        power = data.get("power") or [0, 0, 0]
        self.power_supply, self.power_demand, self.power_used = (float(p) for p in power[:3])
