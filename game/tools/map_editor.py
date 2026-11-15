"""Map editor.

    python map_editor.py            # open the first map
    python map_editor.py hub        # open data/maps/hub.json
    python map_editor.py --new my_map 30 20

Left panel: tools and layers.  Right panel: the tile / prop palette, and the
properties of whatever is selected.  The middle is the map.

Tools (keys in brackets)
    Paint [B]    left-drag paints the selected tile or prop, right-drag erases
    Fill  [F]    flood-fill an area of identical tiles
    Rect  [R]    drag out a filled rectangle
    Pick  [I]    pick the tile under the cursor (or hold Alt with any tool)
    Object [O]   click to select / drag to move; pick a type on the right and
                 click the map to add one; edit its properties on the right
    Region [M]   drag to create a named region; click to select (click again
                 to reach a region lying underneath)
    NPC   [N]    pick a character on the right, click the map to set where
                 they start; drag an existing marker to move them

View: mouse wheel zooms, middle-drag or arrow keys pan, [Tab] grid,
[C] collision overlay, [L] lighting preview, [1]-[4] choose the layer.
[Ctrl+S] save, [Ctrl+Z]/[Ctrl+Y] undo/redo, [Del] delete selection,
[F5] save and play-test from the cursor position.
"""
from __future__ import annotations

import copy
import json
import os
import subprocess
import sys
import types
from typing import Any, Dict, List, Optional, Tuple

import pygame

from game import config
from game.art import tiles as tiledefs
from game.art.chars import CharArt
from game.art.icons import IconArt
from game.content import Content, write_json
from game.engine import palette as P
from game.scenes.render import MapRenderer
from game.tools import gui
from game.world.gamemap import COMMON_FIELDS, OBJECT_TYPES, GameMap, MapObject, Region

T = config.TILE
TOOLS = [("paint", "Paint", pygame.K_b), ("fill", "Fill", pygame.K_f), ("rect", "Rect", pygame.K_r),
         ("pick", "Pick", pygame.K_i), ("object", "Object", pygame.K_o), ("region", "Region", pygame.K_m),
         ("npc", "NPC", pygame.K_n)]
LEFT_W, RIGHT_W, TOP_H, STATUS_H = 150, 320, 36, 26
CELL = 38                 # palette cell size
UNDO_LIMIT = 60


class MapEditor(gui.ToolApp):
    title = "Tethys Deep - Map Editor"

    def __init__(self, map_id: Optional[str] = None, headless: bool = False, content: Optional[Content] = None):
        super().__init__(headless)
        self.content = content or Content.load()
        self.tile_art = tiledefs.TileArt()
        self.char_art = CharArt()
        self.renderer = MapRenderer(self.tile_art, self.char_art, IconArt())
        self._shim = types.SimpleNamespace(content=self.content)
        self.gmap: GameMap = GameMap.new("untitled", "Untitled", 24, 16, "floor_plate")
        self.dirty = False
        self.zoom = 2
        self.cam = [0, 0]
        self.tool = "paint"
        self.layer = "ground"
        self.auto_layer = True
        self.visible = {layer: True for layer in config.LAYERS}
        self.brush: Tuple[str, str] = ("tile", "floor_plate")
        self.show_grid = True
        self.show_solid = False
        self.show_light = False
        self.undo_stack: List[Any] = []
        self.redo_stack: List[Any] = []
        self.sel_obj: Optional[MapObject] = None
        self.sel_region: Optional[Region] = None
        self.sel_char: Optional[str] = None
        self.place_type: Optional[str] = None
        self.dragging: Optional[dict] = None
        self.pick_cb = None               # callback(map_id, x, y) while picking a portal target
        self._stash: Optional[GameMap] = None
        self.char_dirty: set = set()
        self.category = "floor"
        self.pal_scroll = 0
        self.prop_rows: List[Tuple[str, gui.TextInput]] = []
        self._build_widgets()
        self.layout()
        ids = self.content.map_ids
        if map_id and map_id in ids:
            self.load_map(map_id)
        elif ids and map_id is None:
            self.load_map(ids[0])
        self._refresh_panel()

    # ------------------------------------------------------------------
    # widgets and layout
    # ------------------------------------------------------------------
    def _build_widgets(self) -> None:
        B = gui.Button
        self.top_buttons = [
            B((0, 0, 1, 1), "New", self.cmd_new), B((0, 0, 1, 1), "Open", self.cmd_open),
            B((0, 0, 1, 1), "Save", self.cmd_save), B((0, 0, 1, 1), "Resize", self.cmd_resize),
            B((0, 0, 1, 1), "Map", self.cmd_props), B((0, 0, 1, 1), "Undo", self.undo),
            B((0, 0, 1, 1), "Redo", self.redo), B((0, 0, 1, 1), "Play", self.cmd_play),
        ]
        self.tool_buttons = {key: B((0, 0, 1, 1), label, (lambda k=key: self.set_tool(k)))
                             for key, label, _ in TOOLS}
        self.layer_buttons = {layer: B((0, 0, 1, 1), layer.capitalize(), (lambda la=layer: self.set_layer(la)))
                              for layer in config.LAYERS}
        self.eye_buttons = {layer: B((0, 0, 1, 1), "■", (lambda la=layer: self.toggle_layer(la)),
                                     toggle=True, active=True) for layer in config.LAYERS}
        self.auto_button = B((0, 0, 1, 1), "Auto layer", self.toggle_auto, toggle=True, active=True)
        self.grid_button = B((0, 0, 1, 1), "Grid", lambda: self._flag("show_grid"), toggle=True, active=True)
        self.solid_button = B((0, 0, 1, 1), "Solid", lambda: self._flag("show_solid"), toggle=True)
        self.light_button = B((0, 0, 1, 1), "Light", lambda: self._flag("show_light"), toggle=True)
        cats = [c for c in tiledefs.CATEGORIES if c != "prop"] + ["props"]
        self.cat_cycler = gui.Cycler((0, 0, 1, 1), cats, "floor", self._set_category)
        self.side_list = gui.ListBox((0, 0, 1, 1), [], self._side_pick)
        self.add_field_button = B((0, 0, 1, 1), "+ Field", self.cmd_add_field)
        self.delete_button = B((0, 0, 1, 1), "Delete", self.delete_selection)
        self.target_button = B((0, 0, 1, 1), "Pick target", self.cmd_pick_target)
        self.widgets = (self.top_buttons + list(self.tool_buttons.values()) + list(self.layer_buttons.values())
                        + list(self.eye_buttons.values())
                        + [self.auto_button, self.grid_button, self.solid_button, self.light_button,
                           self.cat_cycler, self.side_list, self.add_field_button, self.delete_button,
                           self.target_button])
        self._static_widgets = list(self.widgets)

    def _flag(self, name: str) -> None:
        setattr(self, name, not getattr(self, name))

    def layout(self) -> None:
        sw, sh = self.screen.get_size()
        self.canvas = pygame.Rect(LEFT_W, TOP_H, sw - LEFT_W - RIGHT_W, sh - TOP_H - STATUS_H)
        self.right = pygame.Rect(sw - RIGHT_W, TOP_H, RIGHT_W, sh - TOP_H - STATUS_H)
        x = 4
        for b in self.top_buttons:
            b.rect = pygame.Rect(x, 4, 78, 28)
            x += 80
        y = TOP_H + 26
        for key, _label, _k in TOOLS:
            self.tool_buttons[key].rect = pygame.Rect(6, y, LEFT_W - 12, 26)
            y += 28
        y += 26
        for layer in reversed(config.LAYERS):
            self.layer_buttons[layer].rect = pygame.Rect(6, y, LEFT_W - 44, 26)
            self.eye_buttons[layer].rect = pygame.Rect(LEFT_W - 36, y, 30, 26)
            y += 28
        self.auto_button.rect = pygame.Rect(6, y + 4, LEFT_W - 12, 26)
        y += 60
        for b in (self.grid_button, self.solid_button, self.light_button):
            b.rect = pygame.Rect(6, y, LEFT_W - 12, 26)
            y += 28
        self.cat_cycler.rect = pygame.Rect(self.right.x + 8, self.right.y + 6, RIGHT_W - 16, 28)
        self.pal_rect = pygame.Rect(self.right.x + 8, self.right.y + 40, RIGHT_W - 16, 250)
        self._layout_panel()

    def _layout_panel(self) -> None:
        """Lower half of the right panel: lists and property fields."""
        r = self.right
        top = self.pal_rect.bottom + 28 if self.tool in ("paint", "fill", "rect", "pick") else r.y + 34
        self.panel_top = top
        list_h = 136 if self.tool == "object" else 200
        self.side_list.rect = pygame.Rect(r.x + 8, top, RIGHT_W - 16, list_h)
        y = top + list_h + 26
        label_w = 104
        for key, ti in self.prop_rows:
            ti.rect = pygame.Rect(r.x + 8 + label_w, y, RIGHT_W - 16 - label_w, 24)
            y += 26
        y += 4
        self.add_field_button.rect = pygame.Rect(r.x + 8, y, 96, 26)
        self.delete_button.rect = pygame.Rect(r.x + 110, y, 96, 26)
        self.target_button.rect = pygame.Rect(r.x + 212, y, 100, 26)

    def _refresh_panel(self) -> None:
        """Rebuild the right panel for the current tool and selection."""
        self.focus(None)
        painting = self.tool in ("paint", "fill", "rect", "pick")
        self.cat_cycler.visible = painting
        self.side_list.visible = self.tool in ("object", "npc", "region")
        rows: List[Tuple[str, Any]] = []
        if self.tool == "object":
            self.side_list.set_items([("+ " + spec["label"], t) for t, spec in OBJECT_TYPES.items()], keep=False)
            self.side_list.select(self.place_type)
            obj = self.sel_obj
            if obj is not None:
                rows = [("id", obj.id), ("x", obj.x), ("y", obj.y)]
                keys = list(OBJECT_TYPES.get(obj.type, {}).get("fields", {}))
                for key in obj.props:
                    if key not in keys:
                        keys.append(key)
                defaults = OBJECT_TYPES.get(obj.type, {}).get("fields", {})
                rows += [(k, obj.props.get(k, defaults.get(k, ""))) for k in keys]
        elif self.tool == "region":
            self.side_list.set_items([("%s  (%s)" % (r.name, r.id), r.id) for r in self.gmap.regions], keep=False)
            reg = self.sel_region
            if reg is not None:
                self.side_list.select(reg.id)
                rows = [("id", reg.id), ("name", reg.name), ("x", reg.x), ("y", reg.y), ("w", reg.w), ("h", reg.h),
                        ("props", reg.props)]
        elif self.tool == "npc":
            items = []
            for cid, cd in sorted(self.content.characters.items()):
                here = (cd.get("spawn") or {}).get("map") == self.gmap.id
                items.append(("%s%s" % ("• " if here else "  ", cd.get("name", cid)), cid))
            self.side_list.set_items(items, keep=False)
            self.side_list.select(self.sel_char)
        self.prop_rows = []
        for key, value in rows:
            ti = gui.TextInput((0, 0, 1, 1), gui.format_value(value),
                               on_commit=(lambda text, k=key: self._commit_field(k, text)), scale=1)
            self.prop_rows.append((key, ti))
        has_sel = (self.tool == "object" and self.sel_obj is not None) or \
                  (self.tool == "region" and self.sel_region is not None)
        self.add_field_button.visible = self.tool == "object" and self.sel_obj is not None
        self.delete_button.visible = has_sel
        self.target_button.visible = self.tool == "object" and self.sel_obj is not None and self.sel_obj.type == "portal"
        self.widgets = self._static_widgets + [ti for _k, ti in self.prop_rows]
        for key, button in self.tool_buttons.items():
            button.active = key == self.tool
        for layer, button in self.layer_buttons.items():
            button.active = layer == self.layer
        self._layout_panel()

    # ------------------------------------------------------------------
    # commands
    # ------------------------------------------------------------------
    def set_tool(self, tool: str) -> None:
        self.tool = tool
        self.dragging = None
        self._refresh_panel()

    def set_layer(self, layer: str) -> None:
        self.layer = layer
        self.auto_layer = False
        self.auto_button.active = False
        self._refresh_panel()

    def toggle_layer(self, layer: str) -> None:
        self.visible[layer] = self.eye_buttons[layer].active

    def toggle_auto(self) -> None:
        self.auto_layer = self.auto_button.active

    def _set_category(self, cat: str) -> None:
        self.category = cat
        self.pal_scroll = 0

    def _side_pick(self, data: Any) -> None:
        if self.tool == "object":
            self.place_type = data
            self.sel_obj = None
            self.set_status("Click the map to place: %s" % OBJECT_TYPES[data]["label"])
            self._refresh_panel()
        elif self.tool == "npc":
            self.sel_char = data
            spawn = (self.content.characters[data].get("spawn") or {})
            if spawn.get("map") == self.gmap.id:
                self.center_on(int(spawn.get("x", 0)), int(spawn.get("y", 0)))
            self.set_status("Click the map to set where %s starts." % self.content.characters[data].get("name", data))
        elif self.tool == "region":
            self.sel_region = self.gmap.find_region(data)
            if self.sel_region is not None:
                self.center_on(*self.sel_region.center)
            self._refresh_panel()

    def snapshot(self) -> Any:
        g = self.gmap
        return (copy.deepcopy(g.layers), [o.to_dict() for o in g.objects], [r.to_dict() for r in g.regions],
                dict(g.props), g.width, g.height, g.name)

    def restore(self, snap: Any) -> None:
        g = self.gmap
        layers, objects, regions, props, w, h, name = snap
        g.layers = copy.deepcopy(layers)
        g.objects = []
        for od in objects:
            obj = MapObject.from_dict(od)
            obj.map_id = g.id
            g.objects.append(obj)
        g.regions = [Region.from_dict(rd) for rd in regions]
        g.props, g.width, g.height, g.name = dict(props), w, h, name
        g.invalidate()
        self.sel_obj = None
        self.sel_region = None
        self._refresh_panel()

    def push_undo(self) -> None:
        self.undo_stack.append(self.snapshot())
        if len(self.undo_stack) > UNDO_LIMIT:
            self.undo_stack.pop(0)
        self.redo_stack.clear()
        self.dirty = True

    def undo(self) -> None:
        if self.undo_stack:
            self.redo_stack.append(self.snapshot())
            self.restore(self.undo_stack.pop())
            self.set_status("Undo")

    def redo(self) -> None:
        if self.redo_stack:
            self.undo_stack.append(self.snapshot())
            self.restore(self.redo_stack.pop())
            self.set_status("Redo")

    def load_map(self, map_id: str) -> bool:
        self.content._map_cache.pop(map_id, None)
        gmap = self.content.load_map(map_id)
        if gmap is None:
            self.set_status("Could not load %s" % map_id)
            return False
        self.gmap = gmap
        self.dirty = False
        self.undo_stack.clear()
        self.redo_stack.clear()
        self.sel_obj = self.sel_region = None
        self.cam = [-(self.canvas.w // self.zoom - gmap.width * T) // 2,
                    -(self.canvas.h // self.zoom - gmap.height * T) // 2]
        pygame.display.set_caption("%s - %s" % (self.title, gmap.id))
        self._refresh_panel()
        return True

    def _guard_dirty(self, then) -> None:
        if self.dirty:
            self.confirm("Unsaved changes", "Discard the changes to '%s'?" % self.gmap.id, then)
        else:
            then()

    def cmd_new(self) -> None:
        def create(values: List[str]) -> None:
            map_id = values[0].strip().lower().replace(" ", "_")
            if not map_id:
                return
            if map_id in self.content.map_ids:
                self.message("New map", "A map called '%s' already exists." % map_id)
                return
            try:
                w, h = max(4, int(values[2])), max(4, int(values[3]))
            except ValueError:
                self.message("New map", "Width and height must be numbers.")
                return
            floor = values[4].strip() if values[4].strip() in tiledefs.TILES else "floor_plate"
            self.gmap = GameMap.new(map_id, values[1].strip() or map_id.title(), w, h, floor)
            self.undo_stack.clear()
            self.redo_stack.clear()
            self.dirty = True
            self.cam = [-(self.canvas.w // self.zoom - w * T) // 2, -(self.canvas.h // self.zoom - h * T) // 2]
            self._refresh_panel()
            self.set_status("New map '%s' - remember to save." % map_id)
        self._guard_dirty(lambda: self.prompt("New map", [("Id", "new_map"), ("Name", "New Map"), ("Width", "24"),
                                                          ("Height", "16"), ("Floor tile", "floor_plate")], create))

    def cmd_open(self) -> None:
        self.content.reload()
        items = [(m, m) for m in self.content.map_ids]

        def chosen(map_id: Optional[str]) -> None:
            if map_id:
                self.load_map(map_id)
        self._guard_dirty(lambda: self.pick_list("Open map", items, chosen))

    def cmd_save(self) -> bool:
        path = self.content.map_path(self.gmap.id)
        try:
            self.gmap.save(path)
            for cid in sorted(self.char_dirty):
                write_json(self.content.character_path(cid), self.content.characters[cid])
        except OSError as exc:
            self.message("Save failed", str(exc))
            return False
        self.char_dirty.clear()
        self.dirty = False
        self.content.reload()
        problems = self.check()
        self.set_status("Saved %s.  %s" % (os.path.relpath(path), "No problems found." if not problems
                                           else "%d problem(s) - see the console." % len(problems)), 6)
        for line in problems:
            print(line)
        return True

    def check(self) -> List[str]:
        """Validator messages that concern the current map."""
        from game.tools.validate import validate
        report = validate(self.content)
        key = "maps/%s" % self.gmap.id
        return [m for m in report.errors + report.warnings if m.startswith(key)]

    def cmd_resize(self) -> None:
        g = self.gmap

        def apply(values: List[str]) -> None:
            try:
                w, h, dx, dy = (int(v) for v in values)
            except ValueError:
                self.message("Resize", "All four values must be whole numbers.")
                return
            self.push_undo()
            g.resize(w, h, dx, dy)
            self._refresh_panel()
        self.prompt("Resize map", [("Width", str(g.width)), ("Height", str(g.height)),
                                   ("Shift X", "0"), ("Shift Y", "0")], apply)

    def cmd_props(self) -> None:
        g = self.gmap

        def apply(values: List[str]) -> None:
            self.push_undo()
            g.name = values[0].strip() or g.name
            ambient = gui.parse_value(values[1])
            if isinstance(ambient, list) and len(ambient) >= 3:
                g.props["ambient"] = [int(max(0, min(255, v))) for v in ambient[:3]]
            else:
                g.props.pop("ambient", None)
            backdrop = values[2].strip()
            if backdrop in ("space", "saturn", "none", "ice"):
                g.props["backdrop"] = backdrop
            for key, text in (("cold", values[3]), ("own_light", values[4])):
                if gui.parse_value(text) is True:
                    g.props[key] = True
                else:
                    g.props.pop(key, None)
        self.prompt("Map properties", [
            ("Name", g.name),
            ("Ambient light [r,g,b]", json.dumps(g.props.get("ambient", [218, 222, 236]))),
            ("Backdrop (space/saturn/none/ice)", str(g.props.get("backdrop", "space"))),
            ("Cold (true/false)", "true" if g.props.get("cold") else "false"),
            ("Ignores station power (true/false)", "true" if g.props.get("own_light") else "false")], apply)

    def cmd_play(self) -> None:
        if not self.cmd_save():
            return
        cell = self.cell_at(self.mouse) if self.canvas.collidepoint(self.mouse) else None
        args = [sys.executable, os.path.join(config.ROOT, "main.py"), "--map", self.gmap.id]
        if cell and not self.gmap.solid_at(*cell):
            args += ["--pos", "%d,%d" % cell]
        if self.headless:
            self.set_status("Play-test: " + " ".join(args[1:]))
            return
        try:
            subprocess.Popen(args, cwd=config.ROOT)
            self.set_status("Launched play-test on '%s'." % self.gmap.id)
        except OSError as exc:
            self.message("Play-test", "Could not start the game: %s" % exc)

    def cmd_add_field(self) -> None:
        obj = self.sel_obj
        if obj is None:
            return

        def add(values: List[str]) -> None:
            key = values[0].strip()
            if key and key not in ("id", "type", "x", "y"):
                self.push_undo()
                obj.props[key] = gui.parse_value(values[1])
                self._refresh_panel()
        hint = ", ".join(COMMON_FIELDS)
        self.prompt("Add field (%s ...)" % hint, [("Name", ""), ("Value", "")], add)

    def cmd_pick_target(self) -> None:
        portal = self.sel_obj
        if portal is None or portal.type != "portal":
            return
        home = self.gmap

        def chosen(map_id: Optional[str]) -> None:
            if not map_id:
                return
            target = home if map_id == home.id else self.content.load_map(map_id)
            if target is None:
                return

            def picked(tx: int, ty: int) -> None:
                self.gmap = home
                self.pick_cb = None
                self.push_undo()
                portal.props.update(target_map=map_id, target_x=tx, target_y=ty)
                self.sel_obj = portal
                self.center_on(portal.x, portal.y)
                self._refresh_panel()
                self.set_status("Portal now leads to %s (%d, %d)." % (map_id, tx, ty))
            self.pick_cb = picked
            self.gmap = target
            self.cam = [-(self.canvas.w // self.zoom - target.width * T) // 2,
                        -(self.canvas.h // self.zoom - target.height * T) // 2]
            self.set_status("Click the tile on '%s' where travellers should arrive.  Esc cancels." % map_id, 30)
            self._stash = home
        self.pick_list("Portal target map", [(m, m) for m in self.content.map_ids], chosen)

    def delete_selection(self) -> None:
        if self.tool == "object" and self.sel_obj is not None:
            self.push_undo()
            self.gmap.remove_object(self.sel_obj)
            self.sel_obj = None
        elif self.tool == "region" and self.sel_region is not None:
            self.push_undo()
            self.gmap.regions.remove(self.sel_region)
            self.sel_region = None
        self._refresh_panel()

    def _commit_field(self, key: str, text: str) -> None:
        value = gui.parse_value(text)
        if self.tool == "object" and self.sel_obj is not None:
            obj = self.sel_obj
            self.push_undo()
            if key == "id":
                new = str(value).strip()
                if new and all(o.id != new for o in self.gmap.objects):
                    obj.id = new
                else:
                    self.set_status("That id is empty or already used on this map.")
            elif key in ("x", "y"):
                if isinstance(value, int):
                    setattr(obj, key, value)
            elif value == "" and key not in OBJECT_TYPES.get(obj.type, {}).get("fields", {}):
                obj.props.pop(key, None)
            else:
                obj.props[key] = value
            self.gmap.invalidate()
        elif self.tool == "region" and self.sel_region is not None:
            reg = self.sel_region
            self.push_undo()
            if key in ("x", "y", "w", "h") and isinstance(value, int):
                setattr(reg, key, max(1, value) if key in ("w", "h") else value)
            elif key == "id" and str(value).strip():
                reg.id = str(value).strip()
            elif key == "name":
                reg.name = str(value)
            elif key == "props" and isinstance(value, dict):
                reg.props = value
        self._refresh_panel()

    # ------------------------------------------------------------------
    # map operations
    # ------------------------------------------------------------------
    def center_on(self, x: int, y: int) -> None:
        self.cam = [x * T + T // 2 - self.canvas.w // self.zoom // 2,
                    y * T + T // 2 - self.canvas.h // self.zoom // 2]

    def cell_at(self, pos) -> Tuple[int, int]:
        return ((pos[0] - self.canvas.x) // self.zoom + self.cam[0]) // T, \
               ((pos[1] - self.canvas.y) // self.zoom + self.cam[1]) // T

    def cell_rect(self, x: int, y: int, w: int = 1, h: int = 1) -> pygame.Rect:
        z = self.zoom
        return pygame.Rect(self.canvas.x + (x * T - self.cam[0]) * z, self.canvas.y + (y * T - self.cam[1]) * z,
                           w * T * z, h * T * z)

    def _target_layer(self, tile: str) -> str:
        tdef = tiledefs.TILES.get(tile)
        return tdef.layer if (self.auto_layer and tdef is not None) else self.layer

    def paint_at(self, x: int, y: int) -> None:
        g = self.gmap
        kind, name = self.brush
        if kind == "prop":
            pdef = tiledefs.PROPS[name]
            top = y - pdef.h + 1
            for dx, dy, layer, tile in pdef.parts:
                if g.in_bounds(x + dx, top + dy):
                    g.set_tile(layer, x + dx, top + dy, tile)
        elif g.in_bounds(x, y):
            layer = self._target_layer(name)
            g.set_tile(layer, x, y, name)
            if self.auto_layer:
                self.layer = layer

    def erase_at(self, x: int, y: int) -> None:
        g = self.gmap
        if not g.in_bounds(x, y):
            return
        layer = self.layer
        if self.auto_layer:                      # erase the topmost visible thing
            for candidate in reversed(config.LAYERS):
                if self.visible[candidate] and g.tile(candidate, x, y):
                    layer = candidate
                    break
        name = g.tile(layer, x, y)
        tdef = tiledefs.TILES.get(name)
        if tdef is not None and tdef.prop:       # remove the whole prop
            pdef = tiledefs.PROPS[tdef.prop]
            _p, dx, dy = name.rsplit(".", 2)
            ox, oy = x - int(dx), y - int(dy)
            for pdx, pdy, player, tile in pdef.parts:
                if g.tile(player, ox + pdx, oy + pdy) == tile:
                    g.set_tile(player, ox + pdx, oy + pdy, "")
        else:
            g.set_tile(layer, x, y, "")

    def fill_at(self, x: int, y: int) -> None:
        g = self.gmap
        kind, name = self.brush
        if kind != "tile" or not g.in_bounds(x, y):
            return
        layer = self._target_layer(name)
        target = g.tile(layer, x, y)
        if target == name:
            return
        stack = [(x, y)]
        rows = g.layers[layer]
        while stack:
            cx, cy = stack.pop()
            if not g.in_bounds(cx, cy) or rows[cy][cx] != target:
                continue
            rows[cy][cx] = name
            stack += [(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)]
        g.invalidate()

    def pick_at(self, x: int, y: int) -> None:
        g = self.gmap
        for layer in reversed(config.LAYERS):
            if not self.visible[layer]:
                continue
            name = g.tile(layer, x, y)
            if name:
                tdef = tiledefs.TILES.get(name)
                if tdef is not None and tdef.prop:
                    self.brush = ("prop", tdef.prop)
                    self.category = "props"
                else:
                    self.brush = ("tile", name)
                    self.category = tdef.category if tdef else self.category
                    self.layer = layer
                self.cat_cycler.value = self.category
                self.pal_scroll = 0
                self.set_status("Picked %s" % name)
                self._refresh_panel()
                return

    def object_at(self, x: int, y: int) -> Optional[MapObject]:
        objs = self.gmap.objects_at(x, y)
        if not objs:
            return None
        if self.sel_obj in objs and len(objs) > 1:     # click again to cycle through a stack
            return objs[(objs.index(self.sel_obj) + 1) % len(objs)]
        return objs[-1]

    def chars_here(self) -> List[Tuple[str, dict]]:
        return [(cid, cd) for cid, cd in self.content.characters.items()
                if (cd.get("spawn") or {}).get("map") == self.gmap.id]

    def place_character(self, cid: str, x: int, y: int) -> None:
        cd = self.content.characters.get(cid)
        if cd is None or self.gmap.solid_at(x, y):
            self.set_status("Characters need a walkable tile.")
            return
        spawn = dict(cd.get("spawn") or {})
        spawn.update(map=self.gmap.id, x=x, y=y)
        spawn.setdefault("facing", "down")
        cd["spawn"] = spawn
        self.char_dirty.add(cid)
        self.dirty = True
        self.set_status("%s now starts at %s (%d, %d).  Save to keep it." % (cd.get("name", cid), self.gmap.id, x, y))
        self._refresh_panel()

    # ------------------------------------------------------------------
    # events
    # ------------------------------------------------------------------
    def palette_entries(self) -> List[Tuple[str, str]]:
        if self.category == "props":
            return [("prop", name) for name in tiledefs.PROPS]
        return [("tile", name) for name, t in tiledefs.TILES.items()
                if t.category == self.category and not t.hidden]

    def _palette_click(self, pos) -> None:
        cols = self.pal_rect.w // CELL
        col = (pos[0] - self.pal_rect.x) // CELL
        row = (pos[1] - self.pal_rect.y) // CELL + self.pal_scroll
        entries = self.palette_entries()
        i = row * cols + col
        if 0 <= col < cols and 0 <= i < len(entries):
            self.brush = entries[i]
            if self.tool not in ("paint", "fill", "rect"):
                self.set_tool("paint")
            if self.brush[0] == "tile" and self.auto_layer:
                self.layer = tiledefs.TILES[self.brush[1]].layer
                self._refresh_panel()

    def on_event(self, event) -> bool:
        if event.type == pygame.KEYDOWN:
            return self._key(event)
        if event.type == pygame.MOUSEWHEEL:
            if self.pal_rect.collidepoint(self.mouse) and self.cat_cycler.visible:
                cols = max(1, self.pal_rect.w // CELL)
                rows = (len(self.palette_entries()) + cols - 1) // cols
                self.pal_scroll = max(0, min(max(0, rows - self.pal_rect.h // CELL), self.pal_scroll - event.y))
            elif self.canvas.collidepoint(self.mouse):
                self.set_zoom(self.zoom + (1 if event.y > 0 else -1), self.mouse)
            return True
        if event.type == pygame.MOUSEBUTTONDOWN:
            if self.cat_cycler.visible and self.pal_rect.collidepoint(event.pos) and event.button == 1:
                self._palette_click(event.pos)
                return True
            if self.canvas.collidepoint(event.pos):
                self._canvas_down(event)
                return True
        elif event.type == pygame.MOUSEMOTION:
            self._canvas_move(event)
        elif event.type == pygame.MOUSEBUTTONUP:
            self._canvas_up(event)
        return False

    def set_zoom(self, zoom: int, anchor=None) -> None:
        zoom = max(1, min(5, zoom))
        if zoom == self.zoom:
            return
        ax, ay = anchor or self.canvas.center
        wx = (ax - self.canvas.x) / self.zoom + self.cam[0]
        wy = (ay - self.canvas.y) / self.zoom + self.cam[1]
        self.zoom = zoom
        self.cam = [int(wx - (ax - self.canvas.x) / zoom), int(wy - (ay - self.canvas.y) / zoom)]

    def _key(self, event) -> bool:
        ctrl = bool(event.mod & pygame.KMOD_CTRL)
        key = event.key
        if key == pygame.K_ESCAPE and self.pick_cb is not None:
            self.gmap = self._stash or self.gmap
            self.pick_cb = None
            self.set_status("Cancelled.")
            return True
        if ctrl:
            if key == pygame.K_s:
                self.cmd_save()
            elif key == pygame.K_z:
                (self.redo if event.mod & pygame.KMOD_SHIFT else self.undo)()
            elif key == pygame.K_y:
                self.redo()
            elif key == pygame.K_n:
                self.cmd_new()
            elif key == pygame.K_o:
                self.cmd_open()
            return True
        for tool, _label, k in TOOLS:
            if key == k:
                self.set_tool(tool)
                return True
        if key == pygame.K_e:
            self.set_tool("paint")
            self.brush = ("tile", "")
        elif key in (pygame.K_1, pygame.K_2, pygame.K_3, pygame.K_4):
            self.set_layer(config.LAYERS[key - pygame.K_1])
        elif key == pygame.K_TAB:
            self.show_grid = not self.show_grid
            self.grid_button.active = self.show_grid
        elif key == pygame.K_c:
            self.show_solid = not self.show_solid
            self.solid_button.active = self.show_solid
        elif key == pygame.K_l:
            self.show_light = not self.show_light
            self.light_button.active = self.show_light
        elif key in (pygame.K_DELETE, pygame.K_BACKSPACE):
            self.delete_selection()
        elif key == pygame.K_F5:
            self.cmd_play()
        elif key in (pygame.K_PLUS, pygame.K_EQUALS, pygame.K_KP_PLUS):
            self.set_zoom(self.zoom + 1)
        elif key in (pygame.K_MINUS, pygame.K_KP_MINUS):
            self.set_zoom(self.zoom - 1)
        elif key in (pygame.K_LEFT, pygame.K_a):
            self.cam[0] -= T * 2
        elif key in (pygame.K_RIGHT, pygame.K_d):
            self.cam[0] += T * 2
        elif key in (pygame.K_UP, pygame.K_w):
            self.cam[1] -= T * 2
        elif key in (pygame.K_DOWN, pygame.K_s):
            self.cam[1] += T * 2
        else:
            return False
        return True

    def _canvas_down(self, event) -> None:
        x, y = self.cell_at(event.pos)
        g = self.gmap
        if event.button == 2:
            self.dragging = {"kind": "pan", "start": event.pos, "cam": list(self.cam)}
            return
        if self.pick_cb is not None:
            if event.button == 1 and g.in_bounds(x, y):
                if g.solid_at(x, y):
                    self.set_status("That tile is solid: pick a walkable one.")
                else:
                    self.pick_cb(x, y)
            return
        alt = bool(pygame.key.get_mods() & pygame.KMOD_ALT) if not self.headless else False
        if self.tool == "pick" or (alt and event.button == 1):
            self.pick_at(x, y)
            return
        if self.tool == "paint":
            self.push_undo()
            erase = event.button == 3 or self.brush == ("tile", "")
            (self.erase_at if erase else self.paint_at)(x, y)
            self.dragging = {"kind": "erase" if erase else "paint", "last": (x, y)}
        elif self.tool == "fill" and event.button == 1:
            self.push_undo()
            self.fill_at(x, y)
        elif self.tool == "rect" and event.button in (1, 3):
            self.dragging = {"kind": "rect", "start": (x, y), "end": (x, y), "erase": event.button == 3}
        elif self.tool == "object":
            if event.button == 1 and self.place_type and g.in_bounds(x, y):
                self.push_undo()
                defaults = copy.deepcopy(OBJECT_TYPES[self.place_type]["fields"])
                self.sel_obj = g.add_object(self.place_type, x, y, **defaults)
                self.place_type = None
                self._refresh_panel()
            elif event.button == 1:
                self.sel_obj = self.object_at(x, y)
                if self.sel_obj is not None:
                    self.dragging = {"kind": "move_obj", "moved": False}
                self._refresh_panel()
            elif event.button == 3:
                self.place_type = None
                self.sel_obj = None
                self._refresh_panel()
        elif self.tool == "region":
            if event.button == 1:
                # a plain click selects; dragging out a rectangle creates
                self.dragging = {"kind": "region", "start": (x, y), "end": (x, y)}
            elif event.button == 3:
                self.sel_region = None
                self._refresh_panel()
        elif self.tool == "npc" and event.button == 1:
            for cid, cd in self.chars_here():
                sp = cd["spawn"]
                if (int(sp.get("x", -1)), int(sp.get("y", -1))) == (x, y):
                    self.sel_char = cid
                    self.dragging = {"kind": "move_npc", "cid": cid}
                    self._refresh_panel()
                    return
            if self.sel_char:
                self.place_character(self.sel_char, x, y)

    def _canvas_move(self, event) -> None:
        d = self.dragging
        if not d:
            return
        if d["kind"] == "pan":
            self.cam = [d["cam"][0] - (event.pos[0] - d["start"][0]) // self.zoom,
                        d["cam"][1] - (event.pos[1] - d["start"][1]) // self.zoom]
            return
        x, y = self.cell_at(event.pos)
        if d["kind"] in ("paint", "erase"):
            if (x, y) != d["last"]:
                d["last"] = (x, y)
                (self.erase_at if d["kind"] == "erase" else self.paint_at)(x, y)
        elif d["kind"] in ("rect", "region"):
            d["end"] = (x, y)
        elif d["kind"] == "move_obj" and self.sel_obj is not None and self.gmap.in_bounds(x, y):
            if (self.sel_obj.x, self.sel_obj.y) != (x, y):
                if not d["moved"]:
                    self.push_undo()
                    d["moved"] = True
                self.gmap.move_object(self.sel_obj, x, y)

    def _canvas_up(self, event) -> None:
        d, self.dragging = self.dragging, None
        if not d:
            return
        g = self.gmap
        if d["kind"] in ("rect", "region"):
            (x0, y0), (x1, y1) = d["start"], d["end"]
            x0, x1 = sorted((x0, x1))
            y0, y1 = sorted((y0, y1))
            if d["kind"] == "rect":
                self.push_undo()
                kind, name = self.brush
                for yy in range(y0, y1 + 1):
                    for xx in range(x0, x1 + 1):
                        if d["erase"]:
                            self.erase_at(xx, yy)
                        elif kind == "tile" and g.in_bounds(xx, yy):
                            g.set_tile(self._target_layer(name), xx, yy, name)
            elif (x0, y0) == (x1, y1):
                hit = sorted((r for r in g.regions if r.contains(x0, y0)), key=lambda r: r.area)
                if hit and self.sel_region in hit:      # click again to reach the region underneath
                    hit = hit[hit.index(self.sel_region) + 1:] + hit[:hit.index(self.sel_region) + 1]
                self.sel_region = hit[0] if hit else None
                self._refresh_panel()
            else:
                rect = (max(0, x0), max(0, y0), x1 - x0 + 1, y1 - y0 + 1)

                def create(values: List[str]) -> None:
                    rid = values[0].strip().lower().replace(" ", "_")
                    if not rid:
                        return
                    self.push_undo()
                    self.sel_region = Region(rid, values[1].strip() or rid.replace("_", " ").title(), *rect)
                    g.regions.append(self.sel_region)
                    self._refresh_panel()
                self.prompt("New region", [("Id", "region_%d" % (len(g.regions) + 1)), ("Name", "")], create)
        elif d["kind"] == "move_obj":
            self._refresh_panel()
        elif d["kind"] == "move_npc":
            x, y = self.cell_at(event.pos)
            self.place_character(d["cid"], x, y)

    def can_quit(self) -> bool:
        if not self.dirty:
            return True
        self.confirm("Quit", "There are unsaved changes. Quit anyway?", lambda: setattr(self, "running", False))
        return False

    # ------------------------------------------------------------------
    # drawing
    # ------------------------------------------------------------------
    def draw(self) -> None:
        scr = self.screen
        scr.fill(gui.BG)
        self._draw_canvas()
        sw, sh = scr.get_size()
        f = gui.font()
        small = gui.font(1)
        pygame.draw.rect(scr, gui.PANEL, (0, 0, sw, TOP_H))
        pygame.draw.rect(scr, gui.PANEL, (0, TOP_H, LEFT_W, sh - TOP_H))
        pygame.draw.rect(scr, gui.PANEL, self.right)
        pygame.draw.rect(scr, P.NAVY[0], (0, sh - STATUS_H, sw, STATUS_H))
        g = self.gmap
        f.draw(scr, "%s%s   %dx%d   zoom x%d" % (g.id, " *" if self.dirty else "", g.width, g.height, self.zoom),
               (sw - 10, 8), gui.ACCENT, align="right")
        f.draw(scr, "TOOLS", (8, TOP_H + 4), gui.ACCENT)
        f.draw(scr, "LAYERS", (8, self.layer_buttons["overhead"].rect.y - 22), gui.ACCENT)
        f.draw(scr, "VIEW", (8, self.grid_button.rect.y - 22), gui.ACCENT)
        if self.cat_cycler.visible:
            self._draw_palette()
        self._draw_panel(f, small)
        # status bar
        text = self.status
        if not text and self.canvas.collidepoint(self.mouse):
            x, y = self.cell_at(self.mouse)
            if g.in_bounds(x, y):
                tiles = [g.tile(layer, x, y) for layer in config.LAYERS if g.tile(layer, x, y)]
                region = g.region_at(x, y)
                text = "(%d, %d)  %s%s%s" % (x, y, " / ".join(tiles) or "empty",
                                             "   region: %s" % region.id if region else "",
                                             "   SOLID" if g.solid_at(x, y) else "")
        f.draw(scr, text or "Ctrl+S save   Ctrl+Z undo   wheel zoom   middle-drag pan   F5 play-test",
               (8, sh - STATUS_H + 3), gui.TEXT if text else gui.DIM)

    def _draw_canvas(self) -> None:
        scr = self.screen
        c = self.canvas
        g = self.gmap
        z = self.zoom
        vw, vh = c.w // z + 2, c.h // z + 2
        view = pygame.Surface((vw, vh))
        cx, cy = self.cam
        r = self.renderer
        r.draw_backdrop(view, g, cx, cy, self.time)
        for layer in ("ground", "decor", "objects"):
            if self.visible[layer]:
                r.draw_layer(view, g, layer, cx, cy, self.time)
        r.draw_objects(view, g, cx, cy, self.time, self._shim)
        for cid, cd in self.chars_here():
            sp = cd["spawn"]
            sprite = self.char_art.sprite(cd.get("appearance") or {}, str(sp.get("facing", "down")), 0)
            view.blit(sprite, (int(sp.get("x", 0)) * T - cx, int(sp.get("y", 0)) * T - cy - (config.CHAR_H - T)))
        if self.visible["overhead"]:
            r.draw_layer(view, g, "overhead", cx, cy, self.time)
        if self.show_light:
            r.draw_lighting(view, g, cx, cy, r.ambient_for(g, 1.0))
        clip = scr.get_clip()
        scr.set_clip(c)
        scr.blit(pygame.transform.scale(view, (vw * z, vh * z)), c.topleft)
        small = gui.font(1)
        bounds = self.cell_rect(0, 0, g.width, g.height)
        if self.show_solid:
            veil = pygame.Surface((T * z, T * z), pygame.SRCALPHA)
            veil.fill((230, 60, 60, 70))
            x0, y0 = max(0, cx // T), max(0, cy // T)
            for yy in range(y0, min(g.height, y0 + vh // T + 2)):
                for xx in range(x0, min(g.width, x0 + vw // T + 2)):
                    if g.solid_at(xx, yy):
                        scr.blit(veil, self.cell_rect(xx, yy).topleft)
        if self.show_grid and z >= 2:
            col = (255, 255, 255, 26)
            grid = pygame.Surface(c.size, pygame.SRCALPHA)
            step = T * z
            ox = (-cx * z) % step
            oy = (-cy * z) % step
            for x in range(ox, c.w, step):
                grid.fill(col, (x, 0, 1, c.h))
            for y in range(oy, c.h, step):
                grid.fill(col, (0, y, c.w, 1))
            scr.blit(grid, c.topleft)
        pygame.draw.rect(scr, P.CYAN[2], bounds, 1)
        if self.tool == "region" or self.sel_region is not None:
            for reg in g.regions:
                rr = self.cell_rect(reg.x, reg.y, reg.w, reg.h)
                sel = reg is self.sel_region
                pygame.draw.rect(scr, gui.SELECT if sel else P.AMBER[1], rr, 2 if sel else 1)
                small.draw(scr, reg.id, (rr.x + 3, rr.y + 2), gui.SELECT if sel else P.AMBER[2], shadow=P.BLACK)
        for obj in g.objects:
            rr = self.cell_rect(obj.x, obj.y)
            color = OBJECT_TYPES.get(obj.type, {}).get("color", (255, 0, 255))
            sel = obj is self.sel_obj
            if self.tool == "object" or sel:
                pygame.draw.rect(scr, color, rr.inflate(-2, -2), 2 if sel else 1)
                if z >= 2:
                    small.draw(scr, obj.type[:4], (rr.x + 3, rr.y + 2), color, shadow=P.BLACK)
            else:
                pygame.draw.rect(scr, color, (rr.x + 1, rr.y + 1, 4, 4))
            if sel:
                pygame.draw.rect(scr, gui.SELECT, rr, 2)
                if obj.type == "portal" and obj.props.get("target_map") == g.id:
                    tr = self.cell_rect(int(obj.props.get("target_x", 0)), int(obj.props.get("target_y", 0)))
                    pygame.draw.line(scr, gui.SELECT, rr.center, tr.center, 1)
        if self.tool == "npc":
            for cid, cd in self.chars_here():
                sp = cd["spawn"]
                rr = self.cell_rect(int(sp.get("x", 0)), int(sp.get("y", 0)))
                pygame.draw.rect(scr, gui.SELECT if cid == self.sel_char else P.GREEN[3], rr, 1)
                small.draw(scr, cid, (rr.x, rr.bottom + 1), P.GREEN[4], shadow=P.BLACK)
        # cursor / previews
        if c.collidepoint(self.mouse) and not self.modals:
            x, y = self.cell_at(self.mouse)
            d = self.dragging
            if d and d["kind"] in ("rect", "region"):
                (x0, y0), (x1, y1) = d["start"], d["end"]
                rr = self.cell_rect(min(x0, x1), min(y0, y1), abs(x1 - x0) + 1, abs(y1 - y0) + 1)
                pygame.draw.rect(scr, gui.SELECT, rr, 2)
            elif self.tool in ("paint", "rect", "fill") and self.pick_cb is None:
                kind, name = self.brush
                if kind == "prop":
                    pdef = tiledefs.PROPS[name]
                    for dx, dy, _layer, tile in pdef.parts:
                        ghost = pygame.transform.scale(self.tile_art.get(tile), (T * z, T * z)).copy()
                        ghost.set_alpha(150)
                        scr.blit(ghost, self.cell_rect(x + dx, y - pdef.h + 1 + dy).topleft)
                    pygame.draw.rect(scr, gui.SELECT, self.cell_rect(x, y - pdef.h + 1, pdef.w, pdef.h), 1)
                else:
                    if name:
                        ghost = pygame.transform.scale(self.tile_art.get(name), (T * z, T * z)).copy()
                        ghost.set_alpha(150)
                        scr.blit(ghost, self.cell_rect(x, y).topleft)
                    pygame.draw.rect(scr, gui.SELECT, self.cell_rect(x, y), 1)
            else:
                pygame.draw.rect(scr, P.WHITE, self.cell_rect(x, y), 1)
        if self.pick_cb is not None:
            gui.font().draw(scr, "PICK THE ARRIVAL TILE  (Esc cancels)", (c.centerx, c.y + 8), gui.SELECT,
                            shadow=P.BLACK, align="center")
        scr.set_clip(clip)

    def _draw_palette(self) -> None:
        scr = self.screen
        pr = self.pal_rect
        pygame.draw.rect(scr, P.BLACK, pr.inflate(4, 4))
        pygame.draw.rect(scr, P.NAVY[0], pr)
        cols = max(1, pr.w // CELL)
        entries = self.palette_entries()
        clip = scr.get_clip()
        scr.set_clip(pr)
        hover_label = ""
        for i, (kind, name) in enumerate(entries):
            row = i // cols - self.pal_scroll
            if row < 0:
                continue
            cell = pygame.Rect(pr.x + (i % cols) * CELL, pr.y + row * CELL, CELL, CELL)
            if cell.y > pr.bottom:
                break
            inner = cell.inflate(-6, -6)
            pygame.draw.rect(scr, P.STEEL[1], inner)
            if kind == "tile":
                tdef = tiledefs.TILES[name]
                mask = 0 if tdef.group != "wall" else 0
                img = self.tile_art.get(name, self.tile_art.frame_at(name, self.time), 0, mask)
                scr.blit(pygame.transform.scale(img, inner.size), inner.topleft)
                label = "%s  [%s]%s" % (tdef.label, tdef.layer, "  solid" if tdef.solid else "")
            else:
                pdef = tiledefs.PROPS[name]
                thumb = pygame.Surface((pdef.w * T, pdef.h * T), pygame.SRCALPHA)
                for dx, dy, _layer, tile in pdef.parts:
                    thumb.blit(self.tile_art.get(tile, self.tile_art.frame_at(tile, self.time)), (dx * T, dy * T))
                k = min(inner.w / thumb.get_width(), inner.h / thumb.get_height())
                size = (max(1, int(thumb.get_width() * k)), max(1, int(thumb.get_height() * k)))
                scr.blit(pygame.transform.scale(thumb, size),
                         (inner.centerx - size[0] // 2, inner.centery - size[1] // 2))
                label = "%s  (%dx%d prop)" % (pdef.label, pdef.w, pdef.h)
            if (kind, name) == self.brush:
                pygame.draw.rect(scr, gui.SELECT, cell.inflate(-2, -2), 2)
            if cell.collidepoint(self.mouse):
                pygame.draw.rect(scr, P.WHITE, cell.inflate(-2, -2), 1)
                hover_label = label
        scr.set_clip(clip)
        f = gui.font()
        kind, name = self.brush
        current = "eraser" if not name else (tiledefs.TILES[name].label if kind == "tile" else tiledefs.PROPS[name].label)
        f.draw(scr, hover_label or "Brush: %s" % current, (pr.x, pr.bottom + 6), gui.TEXT if hover_label else gui.ACCENT)

    def _draw_panel(self, f, small) -> None:
        scr = self.screen
        r = self.right
        if self.tool in ("paint", "fill", "rect", "pick"):
            y = self.pal_rect.bottom + 34
            hints = {"paint": "Left-drag paints, right-drag erases.", "fill": "Click to flood-fill.",
                     "rect": "Drag out a rectangle.", "pick": "Click a tile to pick it."}
            f.draw(scr, hints[self.tool], (r.x + 8, y), gui.DIM)
            f.draw(scr, "Layer: %s%s" % (self.layer, "  (auto)" if self.auto_layer else ""), (r.x + 8, y + 22), gui.DIM)
            return
        titles = {"object": "ADD AN OBJECT", "region": "REGIONS", "npc": "CHARACTERS"}
        f.draw(scr, titles[self.tool], (r.x + 8, r.y + 8), gui.ACCENT)
        y = self.side_list.rect.bottom + 4
        if self.tool == "object":
            obj = self.sel_obj
            f.draw(scr, "SELECTED: %s" % (obj.type if obj else "nothing"), (r.x + 8, y), gui.ACCENT)
            if obj is None:
                gui.font(1).draw_wrapped(scr, "Click an object on the map to edit it, or pick a type above and "
                                         "click the map to add one. Values are JSON: 12, true, [1,2], "
                                         "{\"ration_pack\": 3} or plain text.", (r.x + 8, y + 24, RIGHT_W - 16, 80),
                                         gui.DIM, line_gap=2)
        elif self.tool == "region":
            f.draw(scr, "SELECTED: %s" % (self.sel_region.id if self.sel_region else "nothing"), (r.x + 8, y),
                   gui.ACCENT)
            if self.sel_region is None:
                gui.font(1).draw_wrapped(scr, "Drag on the map to create a region. Regions name places for the HUD, "
                                         "for NPC schedules (\"region\": \"mess_hall\") and for scripts "
                                         "(in_region). Click to select; click again for the one underneath.",
                                         (r.x + 8, y + 24, RIGHT_W - 16, 80), gui.DIM, line_gap=2)
        else:
            cid = self.sel_char
            cd = self.content.characters.get(cid or "")
            if cd:
                sp = cd.get("spawn") or {}
                f.draw(scr, cd.get("name", cid), (r.x + 8, y), gui.SELECT)
                small.draw(scr, "%s   starts on %s (%s, %s)" % (cd.get("title", ""), sp.get("map", "?"),
                                                                 sp.get("x", "?"), sp.get("y", "?")),
                           (r.x + 8, y + 22), gui.DIM)
                portrait = pygame.transform.scale(self.char_art.portrait(cd.get("appearance") or {}), (64, 64))
                scr.blit(portrait, (r.x + 8, y + 40))
            gui.font(1).draw_wrapped(scr, "• marks characters already on this map. Pick one and click a "
                                     "walkable tile to move their start here. New characters are made with "
                                     "character_creator.py.", (r.x + 80, y + 44, RIGHT_W - 90, 80), gui.DIM,
                                     line_gap=2)
        for key, ti in self.prop_rows:
            small.draw(scr, key, (r.x + 10, ti.rect.y + 7), gui.DIM)


def main(argv=None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv and argv[0] in ("-h", "--help"):
        print(__doc__)
        return 0
    content = Content.load()
    map_id = None
    if argv and argv[0] == "--new":
        if len(argv) < 2:
            print("usage: map_editor.py --new <id> [width height]")
            return 1
        map_id = argv[1]
        w = int(argv[2]) if len(argv) > 2 else 24
        h = int(argv[3]) if len(argv) > 3 else 16
        if map_id in content.map_ids:
            print("map %r already exists" % map_id)
            return 1
        GameMap.new(map_id, map_id.replace("_", " ").title(), w, h, "floor_plate").save(content.map_path(map_id))
        content.reload()
    elif argv:
        map_id = argv[0]
        if map_id not in content.map_ids:
            print("no such map %r; available: %s" % (map_id, ", ".join(content.map_ids)))
            return 1
    MapEditor(map_id, content=content).run()
    return 0


if __name__ == "__main__":
    sys.exit(main())
