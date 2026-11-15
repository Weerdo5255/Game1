"""World event bus.

Systems announce what happened ("system:reactor:critical", "hour",
"goal_done:dex:prep_shuttle", "flag:met_lin" ...) and anything interested
subscribes.  This is the glue between otherwise independent systems: the
story director reacts to simulation events, NPC morale reacts to station
failures, the UI turns events into notifications.

Events are queued and delivered by :meth:`EventBus.pump`, never in the middle
of whatever raised them, so handlers are free to change the world.
"""
from __future__ import annotations

from collections import deque
from typing import Any, Callable, Deque, Dict, List, Tuple

Handler = Callable[[str, Dict[str, Any]], None]


class EventBus:
    def __init__(self):
        self._handlers: Dict[str, List[Handler]] = {}
        self._queue: Deque[Tuple[str, Dict[str, Any]]] = deque()
        self._pumping = False
        self.history: Deque[str] = deque(maxlen=200)   # recent event names (debug overlay)

    def on(self, name: str, handler: Handler) -> None:
        """Subscribe to an event name; ``"*"`` receives everything."""
        self._handlers.setdefault(name, []).append(handler)

    def emit(self, name: str, **data: Any) -> None:
        self._queue.append((name, data))

    def pump(self, limit: int = 500) -> int:
        """Deliver queued events (including ones raised while delivering)."""
        if self._pumping:
            return 0
        self._pumping = True
        delivered = 0
        try:
            while self._queue and delivered < limit:
                name, data = self._queue.popleft()
                delivered += 1
                self.history.append(name)
                for handler in list(self._handlers.get(name, ())):
                    handler(name, data)
                for handler in list(self._handlers.get("*", ())):
                    handler(name, data)
        finally:
            self._pumping = False
        return delivered

    def clear(self) -> None:
        self._queue.clear()
