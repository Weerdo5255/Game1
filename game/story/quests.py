"""Quest journal.

Quests are defined in ``data/quests.json``::

    "q_reactor": {
      "name": "Keep the Lights On",
      "desc": "The reactor is failing. Help Chief Kowal stabilise it.",
      "main": false, "xp": 80,
      "stages": {
        "find_parts": "Find spare parts for the containment coil.",
        "repair": "Bring the reactor above 75% integrity."
      },
      "outcomes": {"done": "The reactor is holding.", "failed": "The reactor was lost."}
    }

A quest is a little state machine: scripts call ``start_quest``,
``set_stage``, ``complete_quest`` and ``fail_quest``; conditions read it back
with ``quest_active``, ``stage`` and friends.  Stages may be visited in any
order, which is what lets a quest branch.
"""
from __future__ import annotations

from typing import Any, Dict, List, Optional


class QuestLog:
    def __init__(self, defs: Optional[Dict[str, dict]] = None):
        self.defs: Dict[str, dict] = defs or {}
        self.state: Dict[str, Dict[str, Any]] = {}
        self.notes: List[Dict[str, Any]] = []      # free-form journal notes

    # -- queries -----------------------------------------------------------------
    def status(self, qid: str) -> str:
        """"inactive", "active", "done" or "failed"."""
        st = self.state.get(qid)
        return st["status"] if st else "inactive"

    def stage(self, qid: str) -> str:
        st = self.state.get(qid)
        return st["stage"] if st else ""

    def started(self, qid: str) -> bool:
        return qid in self.state

    def visited(self, qid: str, stage: str) -> bool:
        st = self.state.get(qid)
        return bool(st and stage in st["history"])

    def name(self, qid: str) -> str:
        return str(self.defs.get(qid, {}).get("name", qid))

    def stage_text(self, qid: str, stage: Optional[str] = None) -> str:
        stage = self.stage(qid) if stage is None else stage
        return str((self.defs.get(qid, {}).get("stages") or {}).get(stage, ""))

    def by_status(self, status: str) -> List[str]:
        ids = [q for q, st in self.state.items() if st["status"] == status]
        ids.sort(key=lambda q: (not self.defs.get(q, {}).get("main", False), self.state[q]["started"]))
        return ids

    # -- changes (each returns True if something actually changed) ---------------
    def start(self, qid: str, now: float = 0.0, stage: Optional[str] = None) -> bool:
        if qid in self.state:
            return False
        stages = list((self.defs.get(qid, {}).get("stages") or {}).keys())
        first = stage or (stages[0] if stages else "")
        self.state[qid] = {"status": "active", "stage": first, "history": [first] if first else [],
                           "started": now, "ended": None}
        return True

    def set_stage(self, qid: str, stage: str, now: float = 0.0) -> bool:
        if qid not in self.state:
            self.start(qid, now, stage)
            return True
        st = self.state[qid]
        if st["status"] != "active" or st["stage"] == stage:
            return False
        st["stage"] = stage
        if stage not in st["history"]:
            st["history"].append(stage)
        return True

    def finish(self, qid: str, status: str, now: float = 0.0) -> bool:
        if qid not in self.state:
            self.start(qid, now)
        st = self.state[qid]
        if st["status"] != "active":
            return False
        st["status"] = status
        st["ended"] = now
        return True

    def add_note(self, text: str, now: float = 0.0) -> None:
        self.notes.append({"t": now, "text": text})

    # -- save / load ---------------------------------------------------------------
    def to_dict(self) -> dict:
        return {"state": {q: dict(st, history=list(st["history"])) for q, st in self.state.items()},
                "notes": list(self.notes)}

    def load_dict(self, data: Optional[dict]) -> None:
        data = data or {}
        self.state = {q: dict(st) for q, st in (data.get("state") or {}).items()}
        for st in self.state.values():
            st.setdefault("history", [])
            st.setdefault("started", 0.0)
            st.setdefault("ended", None)
        self.notes = list(data.get("notes") or [])
