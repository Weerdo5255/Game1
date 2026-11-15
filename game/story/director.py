"""The story director: rules that react to world events.

``data/story/rules.json`` holds a list of rules::

    {"id": "reactor_warning",
     "on": "hour",
     "if": "cond('reactor') < 40",
     "do": ["announce('Reactor integrity is falling.')", "start_quest('q_reactor')"],
     "once": true}

``on`` names one event or a list of events.  The simulation emits (among
others): ``start``, ``tick`` (every 10 game minutes), ``hour``, ``day``,
``flag:<name>``, ``quest:<id>:<stage>``, ``quest_done:<id>``,
``enter_map:<map>``, ``enter_region:<region>``, ``talk:<npc>``,
``talk_end:<npc>``, ``item:<item>``, ``system:<id>:<status>``,
``goal_done:<npc>:<goal>``, ``goal_failed:<npc>:<goal>``, ``theft``,
``collapse:<npc>``, ``sleep`` and ``wake``.  Scripts can raise their own with
``emit('name')``.

Rules fire once by default.  Set ``"once": false`` (optionally with a
``"cooldown"`` in game minutes) for repeating rules, and ``"delay"`` to run
the effects some game minutes after the trigger.
"""
from __future__ import annotations

from typing import Any, Dict, List, Optional


class Director:
    def __init__(self, world, rules: Optional[List[dict]] = None):
        self.world = world
        self.rules: List[dict] = []
        self.fired: Dict[str, float] = {}          # rule id -> game minute it last fired
        self.pending: List[Dict[str, Any]] = []    # delayed effects: {"at", "rule", "ctx"}
        self._by_event: Dict[str, List[dict]] = {}
        self.set_rules(rules or [])
        world.events.on("*", self._on_event)

    def set_rules(self, rules: List[dict]) -> None:
        self.rules = []
        self._by_event = {}
        for i, rule in enumerate(rules):
            rule = dict(rule)
            rule.setdefault("id", "rule_%d" % i)
            self.rules.append(rule)
            events = rule.get("on", "tick")
            if isinstance(events, str):
                events = [events]
            for ev in events:
                self._by_event.setdefault(ev, []).append(rule)

    def _on_event(self, name: str, data: Dict[str, Any]) -> None:
        rules = self._by_event.get(name)
        if not rules:
            return
        now = self.world.clock.minutes
        ctx = {k: v for k, v in data.items() if isinstance(v, (str, int, float, bool))}
        for rule in rules:
            rid = rule["id"]
            once = rule.get("once", True)
            if rid in self.fired:
                if once:
                    continue
                if now - self.fired[rid] < float(rule.get("cooldown", 0)):
                    continue
            if not self.world.script.eval(rule.get("if"), ctx):
                continue
            self.fired[rid] = now
            delay = float(rule.get("delay", 0))
            if delay > 0:
                self.pending.append({"at": now + delay, "rule": rid, "ctx": ctx})
            else:
                self.world.script.run(rule.get("do"), ctx)

    def update(self) -> None:
        """Run delayed effects whose time has come."""
        if not self.pending:
            return
        now = self.world.clock.minutes
        due = [p for p in self.pending if p["at"] <= now]
        if not due:
            return
        self.pending = [p for p in self.pending if p["at"] > now]
        rules = {r["id"]: r for r in self.rules}
        for p in due:
            rule = rules.get(p["rule"])
            if rule:
                self.world.script.run(rule.get("do"), p.get("ctx"))

    def to_dict(self) -> dict:
        return {"fired": dict(self.fired), "pending": list(self.pending)}

    def load_dict(self, data: Optional[dict]) -> None:
        data = data or {}
        self.fired = {k: float(v) for k, v in (data.get("fired") or {}).items()}
        self.pending = list(data.get("pending") or [])
