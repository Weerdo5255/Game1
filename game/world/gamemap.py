"""Maps: tile layers, objects and named regions.

A map is a grid with four tile layers (see ``config.LAYERS``), a list of
*objects* (doors, containers, consoles, beds, triggers ...) and a list of
*regions* (named rectangles such as "Mess Hall" that the AI and the story
scripts can refer to).

File format (JSON, written by the map editor)::

    {
      "id": "habitat", "name": "Habitat Ring", "width": 30, "height": 20,
      "props": {"ambient": [210, 215, 235]},
      "tileset": ["", "floor_plate", "wall"],
      "layers": {"ground": [[2, 2, 2], [2, 1, 2]], "decor": ..., ...},
      "objects": [{"id": "hab_locker", "type": "container", "x": 4, "y": 2,
                   "name": "Locker", "items": {"ration_pack": 2}}],
      "regions": [{"id": "mess", "name": "Mess Hall", "x": 1, "y": 1, "w": 8, "h": 6}]
    }

Layers store indices into ``tileset`` so files stay small and survive tiles
being added to or removed from the registry.
"""
from __future__ import annotations

import json
import os
from typing import Any, Dict, Iterable, List, Optional, Set, Tuple

from .. import config
from ..art import tiles as tiledefs

# Object types understood by the engine, with the properties the map editor
# offers for each (name -> default value).  Any object may additionally carry
# "name", "cond", "dialogue", "on_interact", "sprite" and "solid".
OBJECT_TYPES: Dict[str, Dict[str, Any]] = {
    "spawn": {"label": "Player start", "color": (120, 220, 120),
              "fields": {"name": "player_start", "facing": "down"}},
    "portal": {"label": "Portal to another map", "color": (120, 200, 255),
               "fields": {"target_map": "", "target_x": 0, "target_y": 0, "facing": "",
                          "locked": False, "key": "", "access": "", "lock_level": 0, "label": ""}},
    "door": {"label": "Sliding door", "color": (90, 160, 230),
             "fields": {"sprite": "door", "locked": False, "key": "", "access": "", "lock_level": 0}},
    "container": {"label": "Container", "color": (240, 190, 90),
                  "fields": {"name": "Container", "items": {}, "locked": False, "key": "",
                             "lock_level": 0, "owner": "", "restricted": False}},
    "console": {"label": "System console", "color": (90, 230, 220),
                "fields": {"system": "", "name": ""}},
    "terminal": {"label": "Readable terminal", "color": (140, 230, 140),
                 "fields": {"name": "Terminal", "text": "", "dialogue": ""}},
    "bed": {"label": "Bed", "color": (200, 150, 240),
            "fields": {"name": "Bunk", "owner": ""}},
    "sign": {"label": "Sign / examine text", "color": (230, 230, 150),
             "fields": {"text": ""}},
    "item": {"label": "Item pickup", "color": (250, 250, 250),
             "fields": {"item": "", "qty": 1}},
    "light": {"label": "Light source", "color": (255, 240, 170),
              "fields": {"radius": 48, "color": [255, 236, 200]}},
    "marker": {"label": "AI marker", "color": (255, 130, 200),
               "fields": {"name": ""}},
    "trigger": {"label": "Step trigger", "color": (255, 110, 110),
                "fields": {"on_enter": [], "once": True, "cond": ""}},
    "interact": {"label": "Scripted interaction", "color": (255, 170, 90),
                 "fields": {"name": "", "dialogue": "", "on_interact": [], "cond": ""}},
}

COMMON_FIELDS = ("name", "cond", "dialogue", "on_interact", "sprite", "solid")


class MapObject:
    """Something placed on a map tile.

    ``props`` is the authored data from the map file; ``state`` holds whatever
    changed during play (it is what gets written to save files).  ``get``
    looks in state first, so scripts can override any property at runtime.
    """
    __slots__ = ("id", "type", "x", "y", "props", "state", "inventory", "map_id", "anim")

    def __init__(self, id: str, type: str, x: int, y: int, props: Optional[dict] = None):
        self.id = id
        self.type = type
        self.x = int(x)
        self.y = int(y)
        self.props: Dict[str, Any] = dict(props or {})
        self.state: Dict[str, Any] = {}
        self.inventory = None       # created on demand for containers
        self.map_id = ""
        self.anim = 0.0             # door open amount etc. (visual, not saved)

    def get(self, key: str, default: Any = None) -> Any:
        if key in self.state:
            return self.state[key]
        return self.props.get(key, default)

    def set(self, key: str, value: Any) -> None:
        self.state[key] = value

    @property
    def name(self) -> str:
        return str(self.get("name") or OBJECT_TYPES.get(self.type, {}).get("label", self.type))

    @property
    def pos(self) -> Tuple[int, int]:
        return (self.x, self.y)

    def access_groups(self) -> List[str]:
        raw = self.get("access") or ""
        if isinstance(raw, (list, tuple)):
            return [str(a) for a in raw if a]
        return [a.strip() for a in str(raw).split(",") if a.strip()]

    def to_dict(self) -> dict:
        data = {"id": self.id, "type": self.type, "x": self.x, "y": self.y}
        data.update(self.props)
        return data

    @classmethod
    def from_dict(cls, data: dict) -> "MapObject":
        props = {k: v for k, v in data.items() if k not in ("id", "type", "x", "y")}
        return cls(str(data.get("id", "")), str(data.get("type", "marker")),
                   int(data.get("x", 0)), int(data.get("y", 0)), props)

    def __repr__(self) -> str:
        return "<MapObject %s %s @%d,%d>" % (self.type, self.id, self.x, self.y)


class Region:
    __slots__ = ("id", "name", "x", "y", "w", "h", "props")

    def __init__(self, id: str, name: str, x: int, y: int, w: int, h: int, props: Optional[dict] = None):
        self.id = id
        self.name = name
        self.x, self.y, self.w, self.h = int(x), int(y), max(1, int(w)), max(1, int(h))
        self.props = dict(props or {})

    def contains(self, x: int, y: int) -> bool:
        return self.x <= x < self.x + self.w and self.y <= y < self.y + self.h

    @property
    def center(self) -> Tuple[int, int]:
        return (self.x + self.w // 2, self.y + self.h // 2)

    @property
    def area(self) -> int:
        return self.w * self.h

    def tiles(self) -> Iterable[Tuple[int, int]]:
        for yy in range(self.y, self.y + self.h):
            for xx in range(self.x, self.x + self.w):
                yield (xx, yy)

    def to_dict(self) -> dict:
        data = {"id": self.id, "name": self.name, "x": self.x, "y": self.y, "w": self.w, "h": self.h}
        if self.props:
            data["props"] = dict(self.props)
        return data

    @classmethod
    def from_dict(cls, data: dict) -> "Region":
        return cls(str(data.get("id", "")), str(data.get("name", data.get("id", ""))),
                   data.get("x", 0), data.get("y", 0), data.get("w", 1), data.get("h", 1),
                   data.get("props"))


class GameMap:
    def __init__(self, id: str, name: str, width: int, height: int):
        self.id = id
        self.name = name
        self.width = int(width)
        self.height = int(height)
        self.props: Dict[str, Any] = {}
        self.layers: Dict[str, List[List[str]]] = {
            layer: [[""] * self.width for _ in range(self.height)] for layer in config.LAYERS}
        self.objects: List[MapObject] = []
        self.regions: List[Region] = []
        self._solid: Optional[List[List[bool]]] = None
        self._obj_grid: Optional[Dict[Tuple[int, int], List[MapObject]]] = None
        self._masks: Dict[Tuple[str, int, int], int] = {}
        self._lights: Optional[list] = None
        self._tags: Dict[Tuple[int, int], Set[str]] = {}

    # -- construction ----------------------------------------------------------
    @classmethod
    def new(cls, id: str, name: str, width: int, height: int, fill: str = "") -> "GameMap":
        gmap = cls(id, name, width, height)
        if fill:
            for row in gmap.layers["ground"]:
                for x in range(width):
                    row[x] = fill
        return gmap

    # -- tiles -----------------------------------------------------------------
    def in_bounds(self, x: int, y: int) -> bool:
        return 0 <= x < self.width and 0 <= y < self.height

    def tile(self, layer: str, x: int, y: int) -> str:
        if 0 <= x < self.width and 0 <= y < self.height:
            return self.layers[layer][y][x]
        return ""

    def set_tile(self, layer: str, x: int, y: int, name: str) -> None:
        if self.in_bounds(x, y) and self.layers[layer][y][x] != name:
            self.layers[layer][y][x] = name
            self.invalidate()

    def invalidate(self) -> None:
        """Drop derived caches after the map was edited."""
        self._solid = None
        self._obj_grid = None
        self._masks.clear()
        self._lights = None
        self._tags.clear()

    def mask(self, layer: str, x: int, y: int) -> int:
        """Autotile neighbour mask for the tile at (layer, x, y)."""
        key = (layer, x, y)
        m = self._masks.get(key)
        if m is None:
            group = tiledefs.group_of(self.layers[layer][y][x])
            m = 0
            if group:
                rows = self.layers[layer]
                for dx, dy, bit in tiledefs.NEIGHBOURS:
                    nx, ny = x + dx, y + dy
                    if not (0 <= nx < self.width and 0 <= ny < self.height):
                        m |= bit
                    elif tiledefs.group_of(rows[ny][nx]) == group:
                        m |= bit
            self._masks[key] = m
        return m

    def _build_solid(self) -> None:
        solid = [[False] * self.width for _ in range(self.height)]
        ground = self.layers["ground"]
        for y in range(self.height):
            for x in range(self.width):
                if not ground[y][x]:
                    solid[y][x] = True          # the void outside the station
                    continue
                for layer in config.LAYERS:
                    if tiledefs.is_solid(self.layers[layer][y][x]):
                        solid[y][x] = True
                        break
        for obj in self.objects:
            if obj.props.get("solid") and self.in_bounds(obj.x, obj.y):
                solid[obj.y][obj.x] = True
        self._solid = solid

    def solid_at(self, x: int, y: int) -> bool:
        """Is the cell blocked by scenery? (Doors and characters are dynamic
        and are checked by the movement code.)"""
        if not (0 <= x < self.width and 0 <= y < self.height):
            return True
        if self._solid is None:
            self._build_solid()
        return self._solid[y][x]

    def tags_at(self, x: int, y: int) -> Set[str]:
        """Union of the tile tags on every layer of a cell (e.g. "cold")."""
        key = (x, y)
        tags = self._tags.get(key)
        if tags is None:
            tags = set()
            if self.in_bounds(x, y):
                for layer in config.LAYERS:
                    tdef = tiledefs.TILES.get(self.layers[layer][y][x])
                    if tdef and tdef.tags:
                        tags.update(tdef.tags)
            self._tags[key] = tags
        return tags

    def lights(self) -> list:
        """Static light sources: (pixel x, pixel y, radius, colour)."""
        if self._lights is None:
            lights = []
            t = config.TILE
            for layer in config.LAYERS:
                rows = self.layers[layer]
                for y in range(self.height):
                    row = rows[y]
                    for x in range(self.width):
                        name = row[x]
                        if name:
                            tdef = tiledefs.TILES.get(name)
                            if tdef is not None and tdef.light:
                                lights.append((x * t + t // 2, y * t + t // 2, tdef.light[0], tdef.light[1]))
            for obj in self.objects:
                if obj.type == "light":
                    col = obj.get("color") or (255, 236, 200)
                    lights.append((obj.x * t + t // 2, obj.y * t + t // 2,
                                   int(obj.get("radius", 48)), tuple(col[:3])))
            self._lights = lights
        return self._lights

    # -- objects ---------------------------------------------------------------
    def _build_obj_grid(self) -> None:
        grid: Dict[Tuple[int, int], List[MapObject]] = {}
        for obj in self.objects:
            grid.setdefault((obj.x, obj.y), []).append(obj)
        self._obj_grid = grid

    def objects_at(self, x: int, y: int) -> List[MapObject]:
        if self._obj_grid is None:
            self._build_obj_grid()
        return self._obj_grid.get((x, y), [])

    def object_at(self, x: int, y: int, type: Optional[str] = None) -> Optional[MapObject]:
        for obj in self.objects_at(x, y):
            if type is None or obj.type == type:
                return obj
        return None

    def get_object(self, obj_id: str) -> Optional[MapObject]:
        for obj in self.objects:
            if obj.id == obj_id:
                return obj
        return None

    def objects_of(self, type: str) -> List[MapObject]:
        return [o for o in self.objects if o.type == type]

    def new_object_id(self, type: str) -> str:
        used = {o.id for o in self.objects}
        n = 1
        while "%s_%s_%d" % (self.id, type, n) in used:
            n += 1
        return "%s_%s_%d" % (self.id, type, n)

    def add_object(self, type: str, x: int, y: int, id: Optional[str] = None, **props) -> MapObject:
        if id is None:
            id = self.new_object_id(type)
        obj = MapObject(id, type, x, y, props)
        obj.map_id = self.id
        self.objects.append(obj)
        self.invalidate()
        return obj

    def remove_object(self, obj: MapObject) -> None:
        if obj in self.objects:
            self.objects.remove(obj)
            self.invalidate()

    def move_object(self, obj: MapObject, x: int, y: int) -> None:
        obj.x, obj.y = int(x), int(y)
        self.invalidate()

    # -- regions ---------------------------------------------------------------
    def region_at(self, x: int, y: int) -> Optional[Region]:
        """Smallest region containing the cell."""
        best = None
        for region in self.regions:
            if region.contains(x, y) and (best is None or region.area < best.area):
                best = region
        return best

    def regions_at(self, x: int, y: int) -> List[Region]:
        return [r for r in self.regions if r.contains(x, y)]

    def find_region(self, region_id: str) -> Optional[Region]:
        for region in self.regions:
            if region.id == region_id:
                return region
        return None

    def place_name(self, x: int, y: int) -> str:
        region = self.region_at(x, y)
        return region.name if region else self.name

    # -- editing helpers ---------------------------------------------------------
    def resize(self, width: int, height: int, dx: int = 0, dy: int = 0) -> None:
        """Change the size; existing content is shifted by (dx, dy)."""
        width, height = max(1, int(width)), max(1, int(height))
        for layer in config.LAYERS:
            old = self.layers[layer]
            new = [[""] * width for _ in range(height)]
            for y in range(self.height):
                for x in range(self.width):
                    nx, ny = x + dx, y + dy
                    if 0 <= nx < width and 0 <= ny < height:
                        new[ny][nx] = old[y][x]
            self.layers[layer] = new
        self.width, self.height = width, height
        for obj in list(self.objects):
            obj.x += dx
            obj.y += dy
            if not self.in_bounds(obj.x, obj.y):
                self.objects.remove(obj)
        for region in list(self.regions):
            region.x += dx
            region.y += dy
            if region.x >= width or region.y >= height or region.x + region.w <= 0 or region.y + region.h <= 0:
                self.regions.remove(region)
        self.invalidate()

    # -- (de)serialisation ---------------------------------------------------------
    def to_dict(self) -> dict:
        tileset: List[str] = [""]
        index: Dict[str, int] = {"": 0}
        layers: Dict[str, List[List[int]]] = {}
        for layer in config.LAYERS:
            rows = []
            for row in self.layers[layer]:
                out = []
                for name in row:
                    i = index.get(name)
                    if i is None:
                        i = index[name] = len(tileset)
                        tileset.append(name)
                    out.append(i)
                rows.append(out)
            layers[layer] = rows
        return {
            "id": self.id,
            "name": self.name,
            "width": self.width,
            "height": self.height,
            "props": dict(self.props),
            "tileset": tileset,
            "layers": layers,
            "objects": [o.to_dict() for o in self.objects],
            "regions": [r.to_dict() for r in self.regions],
        }

    @classmethod
    def from_dict(cls, data: dict) -> "GameMap":
        gmap = cls(str(data["id"]), str(data.get("name", data["id"])), data["width"], data["height"])
        gmap.props = dict(data.get("props") or {})
        tileset = data.get("tileset") or [""]
        for layer in config.LAYERS:
            rows = (data.get("layers") or {}).get(layer)
            if not rows:
                continue
            for y in range(min(gmap.height, len(rows))):
                src = rows[y]
                dst = gmap.layers[layer][y]
                for x in range(min(gmap.width, len(src))):
                    v = src[x]
                    if isinstance(v, int):
                        dst[x] = tileset[v] if 0 <= v < len(tileset) else ""
                    else:
                        dst[x] = str(v or "")
        for od in data.get("objects") or []:
            obj = MapObject.from_dict(od)
            if not obj.id:
                obj.id = gmap.new_object_id(obj.type)
            obj.map_id = gmap.id
            gmap.objects.append(obj)
        for rd in data.get("regions") or []:
            gmap.regions.append(Region.from_dict(rd))
        return gmap

    def dumps(self) -> str:
        """JSON text with one line per tile row (diff friendly)."""
        data = self.to_dict()
        out = ["{"]
        for key in ("id", "name", "width", "height", "props", "tileset"):
            out.append("  %s: %s," % (json.dumps(key), json.dumps(data[key])))
        out.append('  "layers": {')
        for li, layer in enumerate(config.LAYERS):
            out.append("    %s: [" % json.dumps(layer))
            rows = data["layers"][layer]
            for ri, row in enumerate(rows):
                out.append("      [%s]%s" % (",".join("%2d" % v for v in row), "," if ri < len(rows) - 1 else ""))
            out.append("    ]%s" % ("," if li < len(config.LAYERS) - 1 else ""))
        out.append("  },")
        for key in ("objects", "regions"):
            items = data[key]
            if not items:
                out.append("  %s: []%s" % (json.dumps(key), "," if key == "objects" else ""))
                continue
            out.append("  %s: [" % json.dumps(key))
            for i, item in enumerate(items):
                out.append("    %s%s" % (json.dumps(item), "," if i < len(items) - 1 else ""))
            out.append("  ]%s" % ("," if key == "objects" else ""))
        out.append("}")
        return "\n".join(out) + "\n"

    def save(self, path: str) -> None:
        os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
        tmp = path + ".tmp"
        with open(tmp, "w", encoding="utf8") as fh:
            fh.write(self.dumps())
        os.replace(tmp, path)

    @classmethod
    def load(cls, path: str) -> "GameMap":
        with open(path, "r", encoding="utf8") as fh:
            return cls.from_dict(json.load(fh))

    def __repr__(self) -> str:
        return "<GameMap %s %dx%d>" % (self.id, self.width, self.height)
