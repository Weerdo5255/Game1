"""Grid path-finding and cross-map routing.

``find_path`` is a plain 4-direction A* over one map.  ``Navigator`` knows
about the whole world: which doors a character may open, which portals link
which maps, and how to get from anywhere to anywhere.  NPCs use it whether or
not the player can see them - there is no "abstract" off-screen movement.
"""
from __future__ import annotations

import heapq
from collections import deque
from typing import Callable, Dict, Iterable, List, Optional, Set, Tuple

Cell = Tuple[int, int]
_STEPS = ((0, -1), (1, 0), (0, 1), (-1, 0))


def find_path(start: Cell, goals: Iterable[Cell], passable: Callable[[int, int], bool],
              max_nodes: int = 8000) -> Optional[List[Cell]]:
    """Shortest path from ``start`` to the nearest of ``goals``.

    Returns the cells to step through (excluding ``start``, including the
    goal), an empty list if already there, or ``None`` if unreachable.  Goal
    cells are always considered enterable.
    """
    goal_set: Set[Cell] = set(goals)
    if not goal_set:
        return None
    if start in goal_set:
        return []
    goal_list = list(goal_set)

    if len(goal_list) == 1:
        gx, gy = goal_list[0]

        def h(x: int, y: int) -> int:
            return abs(x - gx) + abs(y - gy)
    elif len(goal_list) <= 6:
        def h(x: int, y: int) -> int:
            return min(abs(x - g[0]) + abs(y - g[1]) for g in goal_list)
    else:
        def h(x: int, y: int) -> int:
            return 0

    open_heap: List[Tuple[int, int, Cell]] = [(h(*start), 0, start)]
    came: Dict[Cell, Cell] = {}
    cost: Dict[Cell, int] = {start: 0}
    counter = 0
    expanded = 0
    while open_heap:
        _, _, cur = heapq.heappop(open_heap)
        if cur in goal_set:
            path = [cur]
            while path[-1] in came:
                path.append(came[path[-1]])
            path.pop()          # drop the start cell
            path.reverse()
            return path
        expanded += 1
        if expanded > max_nodes:
            return None
        cx, cy = cur
        g = cost[cur] + 1
        for dx, dy in _STEPS:
            nxt = (cx + dx, cy + dy)
            if nxt not in goal_set and not passable(nxt[0], nxt[1]):
                continue
            if g < cost.get(nxt, 1 << 30):
                cost[nxt] = g
                came[nxt] = cur
                counter += 1
                heapq.heappush(open_heap, (g + h(nxt[0], nxt[1]), counter, nxt))
    return None


def can_open(char, obj) -> bool:
    """May ``char`` pass through a door / portal / open a container?

    An object only restricts anyone while its ``locked`` flag is set; then the
    character needs the ``key`` item or membership of one of the object's
    ``access`` groups (granted by role or by carrying a keycard).
    """
    if not obj.get("locked"):
        return True
    key = obj.get("key")
    if key and char.inventory.has(key):
        return True
    groups = obj.access_groups()
    if groups:
        mine = char.access_groups()
        for g in groups:
            if g in mine:
                return True
    return False


class Navigator:
    def __init__(self, world):
        self.world = world
        self._portals: Optional[Dict[str, list]] = None

    def invalidate(self) -> None:
        self._portals = None

    # -- single map -------------------------------------------------------------
    def passable_fn(self, char, map_id: str) -> Callable[[int, int], bool]:
        """Passability for path planning: scenery, locked doors and portal
        tiles block (a portal is only ever stepped on as a destination)."""
        gmap = self.world.maps[map_id]
        solid_at = gmap.solid_at
        objects_at = gmap.objects_at

        def passable(x: int, y: int) -> bool:
            if solid_at(x, y):
                return False
            for obj in objects_at(x, y):
                t = obj.type
                if t == "portal":
                    return False
                if t == "door" and not can_open(char, obj):
                    return False
            return True
        return passable

    def path(self, char, map_id: str, start: Cell, goals: Iterable[Cell]) -> Optional[List[Cell]]:
        return find_path(start, goals, self.passable_fn(char, map_id))

    def approach_cells(self, char, map_id: str, x: int, y: int) -> List[Cell]:
        """Cells from which ``(x, y)`` can be used: the cell itself if it is
        walkable, otherwise its free neighbours (south side first, which is
        where consoles and lockers face)."""
        passable = self.passable_fn(char, map_id)
        if passable(x, y):
            return [(x, y)]
        cells = []
        for dx, dy in ((0, 1), (-1, 0), (1, 0), (0, -1)):
            if passable(x + dx, y + dy):
                cells.append((x + dx, y + dy))
        return cells

    def nearest_free(self, char, map_id: str, x: int, y: int, radius: int = 6) -> Optional[Cell]:
        """Closest walkable cell to (x, y) (breadth-first)."""
        passable = self.passable_fn(char, map_id)
        gmap = self.world.maps[map_id]
        seen = {(x, y)}
        queue = deque([(x, y, 0)])
        while queue:
            cx, cy, d = queue.popleft()
            if passable(cx, cy):
                return (cx, cy)
            if d >= radius:
                continue
            for dx, dy in _STEPS:
                n = (cx + dx, cy + dy)
                if n not in seen and gmap.in_bounds(*n):
                    seen.add(n)
                    queue.append((n[0], n[1], d + 1))
        return None

    # -- between maps -----------------------------------------------------------
    def portals(self) -> Dict[str, list]:
        if self._portals is None:
            table: Dict[str, list] = {}
            for map_id, gmap in self.world.maps.items():
                table[map_id] = [o for o in gmap.objects
                                 if o.type == "portal" and o.get("target_map") in self.world.maps]
            self._portals = table
        return self._portals

    def map_route(self, char, from_map: str, to_map: str) -> Optional[List[str]]:
        """Sequence of maps to pass through (excluding ``from_map``)."""
        if from_map == to_map:
            return []
        portals = self.portals()
        prev: Dict[str, str] = {from_map: ""}
        queue = deque([from_map])
        while queue:
            cur = queue.popleft()
            for portal in portals.get(cur, ()):
                nxt = portal.get("target_map")
                if nxt in prev or not can_open(char, portal):
                    continue
                prev[nxt] = cur
                if nxt == to_map:
                    route = [nxt]
                    while prev[route[-1]] != from_map:
                        route.append(prev[route[-1]])
                    route.reverse()
                    return route
                queue.append(nxt)
        return None

    def next_portal(self, char, to_map: str):
        """The portal in the character's current map to walk to in order to
        travel towards ``to_map`` (None if there is no usable route)."""
        route = self.map_route(char, char.map_id, to_map)
        if not route:
            return None
        nxt = route[0]
        best, best_d = None, 1 << 30
        for portal in self.portals().get(char.map_id, ()):
            if portal.get("target_map") == nxt and can_open(char, portal):
                d = abs(portal.x - char.x) + abs(portal.y - char.y)
                if d < best_d:
                    best, best_d = portal, d
        return best

    def travel_cost(self, char, map_id: str, x: int, y: int) -> int:
        """Rough distance used to pick the nearest of several destinations."""
        if map_id == char.map_id:
            return abs(x - char.x) + abs(y - char.y)
        route = self.map_route(char, char.map_id, map_id)
        if route is None:
            return 1 << 20
        return 40 * len(route) + 10
