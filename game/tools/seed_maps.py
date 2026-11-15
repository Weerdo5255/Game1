"""Generate the demo campaign's maps from ASCII layouts.

    python -m game.tools.seed_maps            # write missing maps only
    python -m game.tools.seed_maps --force    # regenerate everything

The maps in ``data/maps`` are ordinary files that you are expected to edit
with the map editor, so by default this script refuses to overwrite them.  It
exists so the campaign can be rebuilt from scratch, and as a worked example
of creating maps in code with the :class:`~game.world.gamemap.GameMap` API.

Each map is drawn twice: a *base* grid (floors, walls, furniture) and an
optional *over* grid (floor decals, wall signs, ceiling fixtures).  Digits in
the base grid are anchors for doors and markers; furniture is addressed as
``("L", 2)`` - "the third locker in reading order".
"""
from __future__ import annotations

import os
import sys
from typing import Any, Dict, List, Optional, Tuple

from game import config
from game.art import tiles as tiledefs
from game.world.gamemap import GameMap, Region

# symbol -> ("tile", name) for single tiles or ("prop", name) for multi-tile props
FURNITURE = {
    "c": ("tile", "crate"), "C": ("tile", "crate_stack"), "b": ("tile", "barrel"),
    "T": ("tile", "table"), "t": ("tile", "chair"), "d": ("tile", "desk"),
    "s": ("tile", "stool"), "h": ("tile", "bench"),
    "k": ("tile", "console"), "a": ("tile", "console_amber"), "g": ("tile", "console_green"),
    "r": ("tile", "console_red"), "m": ("tile", "terminal"),
    "P": ("tile", "planter"), "p": ("tile", "pot_plant"), "u": ("tile", "pump"),
    "n": ("tile", "generator"), "x": ("tile", "rubble"), "^": ("tile", "ice_spike"),
    "*": ("tile", "crystal"), "e": ("tile", "ore_cart"), "l": ("tile", "lamp"),
    "L": ("prop", "locker"), "S": ("prop", "shelf"), "V": ("prop", "server"),
    "B": ("prop", "bed"), "E": ("prop", "bed_red"), "A": ("prop", "bed_amber"),
    "M": ("prop", "medbed"), "Y": ("prop", "cryo_pod"), "G": ("prop", "dispenser"),
    "w": ("prop", "tank_water"), "q": ("prop", "tank_coolant"), "j": ("prop", "tank_algae"),
    "F": ("prop", "fabricator"), "R": ("prop", "reactor"), "H": ("prop", "holo_table"),
    "X": ("prop", "comm_dish"), "Z": ("prop", "drill"), "O": ("prop", "artifact"),
    "U": ("prop", "shuttle"),
}
FIXED_GROUND = {"_": "floor_grate", "=": "floor_hazard", ";": "floor_threshold", "o": "floor_pad",
                "~": "floor_glass", "&": "wall", "W": "window", "%": "wall_dark"}

# over grid: symbol -> (layer, tile)
OVER = {
    "-": ("decor", "conduit_h"), "|": ("decor", "conduit_v"), "+": ("decor", "conduit_x"),
    "v": ("decor", "vent_floor"), "z": ("decor", "hazard_line"), ">": ("decor", "guide_h"),
    "^": ("decor", "guide_v"), "o": ("decor", "stain"), "i": ("decor", "floor_light"),
    "f": ("decor", "frost"), "y": ("decor", "glyph_floor"), "r": ("decor", "rug"),
    "!": ("decor", "sign_warn"), "m": ("decor", "sign_med"), "e": ("decor", "sign_eng"),
    "b": ("decor", "sign_bio"), "c": ("decor", "sign_cmd"), "s": ("decor", "sign_sci"),
    "n": ("decor", "screen"), "w": ("decor", "vent_wall"), "p": ("decor", "pipes_wall"),
    "k": ("decor", "locker_wall"),
    "=": ("overhead", "beam_h"), '"': ("overhead", "beam_v"), "x": ("overhead", "pipes_over"),
    "l": ("overhead", "ceiling_light"), "g": ("overhead", "grow_light"), "q": ("overhead", "icicles"),
}


class MapBuilder:
    def __init__(self, map_id: str, name: str, base: List[str], over: Optional[List[str]] = None,
                 wall: str = "wall", floors: Optional[Dict[str, str]] = None, props: Optional[dict] = None):
        width = len(base[0])
        for i, row in enumerate(base):
            if len(row) != width:
                raise ValueError("%s: row %d is %d wide, expected %d" % (map_id, i, len(row), width))
        self.base = base
        self.gmap = GameMap(map_id, name, width, len(base))
        self.gmap.props = dict(props or {})
        self.wall = wall
        self.floors = {".": "floor_plate", ",": "floor_hab", ":": "floor_lab"}
        self.floors.update(floors or {})
        self.anchors: Dict[str, Tuple[int, int]] = {}
        self.symbols: Dict[str, List[Tuple[int, int]]] = {}
        self._paint_base()
        self._paint_over(over or [])

    # -- painting ------------------------------------------------------------------
    def _floor_near(self, x: int, y: int) -> str:
        for dx, dy in ((-1, 0), (1, 0), (0, -1), (0, 1)):
            nx, ny = x + dx, y + dy
            if 0 <= ny < len(self.base) and 0 <= nx < len(self.base[0]):
                ch = self.base[ny][nx]
                if ch in self.floors:
                    return self.floors[ch]
                if ch in FIXED_GROUND and not tiledefs.is_solid(FIXED_GROUND[ch]):
                    return FIXED_GROUND[ch]
        return self.floors["."]

    def _paint_base(self) -> None:
        g = self.gmap
        stamps = []
        for y, row in enumerate(self.base):
            for x, ch in enumerate(row):
                if ch == " ":
                    continue
                if ch == "#":
                    g.layers["ground"][y][x] = self.wall
                elif ch in self.floors:
                    g.layers["ground"][y][x] = self.floors[ch]
                elif ch in FIXED_GROUND:
                    g.layers["ground"][y][x] = FIXED_GROUND[ch]
                elif ch.isdigit():
                    g.layers["ground"][y][x] = self._floor_near(x, y)
                    self.anchors[ch] = (x, y)
                elif ch in FURNITURE:
                    g.layers["ground"][y][x] = self._floor_near(x, y)
                    self.symbols.setdefault(ch, []).append((x, y))
                    stamps.append((x, y, FURNITURE[ch]))
                else:
                    raise ValueError("%s: unknown base symbol %r at %d,%d" % (g.id, ch, x, y))
        for x, y, (kind, name) in stamps:
            if kind == "tile":
                g.layers["objects"][y][x] = name
            else:
                self.stamp(name, x, y)
        g.invalidate()

    def stamp(self, prop: str, x: int, y: int) -> None:
        """Place a multi-tile prop with its bottom-left corner at (x, y)."""
        pdef = tiledefs.PROPS[prop]
        top = y - pdef.h + 1
        g = self.gmap
        for dx, dy, layer, tile in pdef.parts:
            px, py = x + dx, top + dy
            if g.in_bounds(px, py):
                g.layers[layer][py][px] = tile
                if not g.layers["ground"][py][px]:
                    g.layers["ground"][py][px] = self._floor_near(px, py)
        g.invalidate()

    def _paint_over(self, over: List[str]) -> None:
        g = self.gmap
        for y, row in enumerate(over[:g.height]):
            for x, ch in enumerate(row[:g.width]):
                if ch in (" ", "."):
                    continue
                if ch == "(":
                    for dx, dy, layer, tile in tiledefs.PROPS["viewscreen"].parts:
                        if g.in_bounds(x + dx, y + dy):
                            g.layers[layer][y + dy][x + dx] = tile
                    continue
                if ch not in OVER:
                    raise ValueError("%s: unknown over symbol %r at %d,%d" % (g.id, ch, x, y))
                layer, tile = OVER[ch]
                g.layers[layer][y][x] = tile
        g.invalidate()

    # -- addressing ----------------------------------------------------------------
    def at(self, ref) -> Tuple[int, int]:
        """Resolve "3" (anchor digit), ("L", 2) (third locker) or (x, y)."""
        if isinstance(ref, str):
            return self.anchors[ref]
        if isinstance(ref[0], str):
            return self.symbols[ref[0]][ref[1]]
        return int(ref[0]), int(ref[1])

    def arrival(self, digit: str) -> Tuple[int, int]:
        """The walkable cell next to a door anchor (where arrivals appear)."""
        x, y = self.anchors[digit]
        for dx, dy in ((0, 1), (0, -1), (1, 0), (-1, 0)):
            if not self.gmap.solid_at(x + dx, y + dy) and (x + dx, y + dy) not in self.anchors.values():
                return x + dx, y + dy
        raise ValueError("%s: door %s has no free neighbour" % (self.gmap.id, digit))

    # -- objects -------------------------------------------------------------------
    def obj(self, type: str, ref, id: Optional[str] = None, dx: int = 0, dy: int = 0, **props):
        x, y = self.at(ref)
        return self.gmap.add_object(type, x + dx, y + dy, id=id, **props)

    def region(self, rid: str, name: str, x: int, y: int, w: int, h: int, **props) -> None:
        self.gmap.regions.append(Region(rid, name, x, y, w, h, props))


def build_all() -> Dict[str, GameMap]:
    b: Dict[str, MapBuilder] = {}

    # ------------------------------------------------------------------ hub
    b["hub"] = m = MapBuilder("hub", "Central Ring", [
        "####WWWWWWWW####1####WWWWWWWW#######",
        "#..................................#",
        "#..h.h.h....................h.h.h..#",
        "#..................................#",
        "2..........############............3",
        "#..........#,G,,,,,,p,#............#",
        "#..........#,,,,,,,,,,#............#",
        "#..p.......#,sTs,,sTs,#.......p....#",
        "#...........,,,,,,,,,,.............#",
        "#...........,sTs,,sTs,.............#",
        "#...........,,,,,,,,,,.............#",
        "#..................................#",
        "#........................8.........#",
        "4..................................5",
        "#..................................#",
        "#..................................#",
        "#####7####.........................#",
        "#q.k...wj#.........................#",
        "#........#.....................m.m.#",
        "#.n...u..#.........................#",
        "#........#.........................#",
        "#........#.........................#",
        "#........#.........................#",
        "#################6##################",
    ], over=[
        "               c c",
        "",
        "",
        "      l           l          l",
        "              n    n",
        "",
        "                l",
        "", "", "", "",
        "     l                       l",
        "",
        " >>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>",
        "", "",
        "   !    w",
        "",
        "    l",
        "                 l            l",
        "  v   v",
        "",
        "    -----",
    ], props={"backdrop": "saturn", "ambient": [222, 226, 240]})
    m.obj("portal", "1", id="hub_to_command", sprite="door", target=("command", "1"), label="Command Deck")
    m.obj("portal", "2", id="hub_to_hydro", sprite="door_v", target=("hydroponics", "1"), label="Hydroponics")
    m.obj("portal", "3", id="hub_to_medlab", sprite="door_v", target=("medlab", "1"), label="Medical & Research")
    m.obj("portal", "4", id="hub_to_eng", sprite="door_v", target=("engineering", "1"), label="Engineering")
    m.obj("portal", "5", id="hub_to_habitat", sprite="door_v", target=("habitat", "1"), label="Crew Quarters")
    m.obj("portal", "6", id="hub_to_hangar", sprite="door_h", target=("hangar", "1"), label="Hangar Bay")
    m.obj("door", "7", id="ls_door", sprite="door")
    m.obj("marker", "8", id="patrol_hub", name="patrol_hub")
    m.obj("container", ("G", 0), id="galley_dispenser", name="Galley dispenser",
          items={"ration_pack": 9, "nutrient_paste": 6})
    m.obj("console", ("k", 0), id="ls_console", system="life_support", name="Life support console")
    m.obj("terminal", ("m", 0), id="hub_terminal_1", name="Public terminal", dialogue="terminal_public")
    m.obj("terminal", ("m", 1), id="hub_terminal_2", name="Crew noticeboard", dialogue="terminal_notices")
    m.obj("sign", (5, 0), id="hub_window_1", text="Saturn fills a third of the sky. The rings are a blade of light. It never gets old; the crew say it never gets warm, either.")
    m.obj("sign", (24, 0), id="hub_window_2", text="Somewhere out there the tug Calypso's Thread is swinging round the planet. Five days until she is back.")
    m.region("concourse", "Central Ring", 1, 1, 34, 22)
    m.region("gallery", "Observation Gallery", 1, 1, 34, 2)
    m.region("mess_hall", "Mess Hall", 12, 5, 10, 6)
    m.region("life_support", "Life Support", 1, 17, 8, 6)

    # --------------------------------------------------------------- hangar
    b["hangar"] = m = MapBuilder("hangar", "Hangar Bay", [
        "##############1###############",
        "#.................#S.S.L.L.S.#",
        "#..========.......#..........#",
        "#..=oooooo=..cc...#..dk......#",
        "#..=oooooo=..cC...#..t.......#",
        "#..=oooooo=.......####2#######",
        "#..=oooooo=..................#",
        "#..=oooooo=......cc..........#",
        "#..=oooooo=......Cc....bb....#",
        "#..=oooooo=.............b....#",
        "#..=oUoooo=..................3",
        "#..=oooooo=..................#",
        "#..========.................l#",
        "#............................#",
        "#...l.......6.........ccc....#",
        "#.....................cCc....#",
        "#............................#",
        "#.......b....................#",
        "#......bb....=====...........#",
        "#............=...=...........#",
        "#............=.4.=...........#",
        "#############=...=############",
        "             #.5.#            ",
        "             #####            ",
    ], over=[
        "             ! !     p     w",
        "",
        "",
        "",
        "",
        "                   e",
        "      o",
        "",
        "     o",
        "",
        "                        >>>>>",
        "",
        "",
        "       l              l",
        "",
        "",
        "             l",
        "",
        "",
        "",
        "",
        "            !",
    ], props={"backdrop": "space", "ambient": [200, 206, 226]})
    m.obj("portal", "1", id="hangar_to_hub", sprite="door", target=("hub", "6"), label="Central Ring")
    m.obj("door", "2", id="stores_door", sprite="door")
    m.obj("portal", "3", id="hangar_to_lift", sprite="door_v", target=("excavation", "1"), label="Excavation Lift")
    m.obj("spawn", "4", id="player_start", name="player_start", facing="up")
    m.obj("interact", "5", id="airlock", name="Main airlock", dialogue="airlock")
    m.obj("marker", "6", id="patrol_hangar", name="patrol_hangar")
    m.obj("container", ("S", 0), id="stores_food", name="Provisions shelf", owner="dex", restricted=True,
          items={"ration_pack": 14, "nutrient_paste": 10, "coffee_bulb": 4})
    m.obj("container", ("S", 1), id="stores_parts", name="Hardware shelf", owner="dex", restricted=True,
          items={"spare_parts": 3, "scrap_metal": 8})
    m.obj("container", ("L", 0), id="stores_fuel", name="Fuel locker", owner="dex", restricted=True,
          locked=True, access="engineering,stores", lock_level=13, items={"fuel_cell": 4})
    m.obj("container", ("L", 1), id="dex_locker", name="Quartermaster's locker", owner="dex", restricted=True,
          locked=True, key="dex_key", lock_level=15,
          items={"spare_parts": 4, "stim": 3, "pump_seal": 1, "cargo_manifest": 1, "synth_whiskey": 2})
    m.obj("container", ("S", 2), id="stores_misc", name="Sundries shelf", owner="dex", restricted=True,
          items={"medkit": 1, "thermal_suit": 1, "scrap_metal": 3})
    m.obj("container", ("U", 0), id="shuttle_hold", dx=1, name="Magpie cargo hatch", owner="dex", restricted=True,
          locked=True, key="magpie_key", lock_level=18, machine=True, items={})
    m.obj("interact", ("U", 0), id="magpie", dx=2, name="The Magpie", dialogue="magpie")
    m.obj("sign", ("d", 0), id="dex_desk", text="A desk buried in requisition forms. Most are stamped APPROVED in the same handwriting that filled them in.")
    m.region("cargo", "Cargo Floor", 11, 6, 18, 12)
    m.region("shuttle_pad", "Shuttle Pad", 3, 2, 8, 11)
    m.region("stores", "Quartermaster's Stores", 19, 1, 10, 4)
    m.region("airlock_bay", "Main Airlock", 13, 18, 5, 5)

    # -------------------------------------------------------------- habitat
    b["habitat"] = m = MapBuilder("habitat", "Crew Quarters", [
        "##############################",
        "#...L....#...L....#...L......#",
        "#B.....E.#A.....B.#E......A..#",
        "#........#........#..........#",
        "#..p.....#.....p..#....dm....#",
        "#........#........#....t.....#",
        "####2########3########4#######",
        "#............................#",
        "1..............7............p#",
        "#............................#",
        "####5########6######....######",
        "#.....L..#.....L..#..........#",
        "#B.E...A.#E.A...B.#..TT....p.#",
        "#........#........#..........#",
        "#........#........#.h........#",
        "#..p.....#.....p..#.....Tt...#",
        "#........#........#..........#",
        "##############################",
    ], over=[
        "      k        k        n",
        "",
        "",
        "    l        l        l",
        "",
        "",
        "",
        "       l       l       l",
        " >>>>>>>>>>>>>>>>>>>>>>>>>>>",
        "",
        "                          n",
        "",
        "                    rrrrrr",
        "    l        l      rrrrrr",
        "                    rrrrrr",
        "                       l",
    ], wall="wall_hab", floors={".": "floor_hab"}, props={"backdrop": "space", "ambient": [226, 222, 214]})
    m.obj("portal", "1", id="habitat_to_hub", sprite="door_v", target=("hub", "5"), label="Central Ring")
    for digit in "23456":
        m.obj("door", digit, id="hab_door_" + digit, sprite="door")
    m.obj("marker", "7", id="patrol_habitat", name="patrol_habitat")
    beds = [(("B", 0), "bed_player", "player"), (("E", 0), "bed_pip", "pip"), (("A", 0), "bed_wren", "wren"),
            (("B", 1), "bed_tuck", "tuck"), (("E", 1), "bed_osei", "osei"), (("A", 1), "bed_halloran", "halloran"),
            (("B", 2), "bed_bram", "bram"), (("A", 2), "bed_dex", "dex"), (("E", 3), "bed_lin", "lin"),
            (("B", 3), "bed_pell", "pell"),
            # two unclaimed bunks for characters added later with the character creator
            (("E", 2), "bed_spare_1", ""), (("A", 3), "bed_spare_2", "")]
    for ref, bed_id, owner in beds:
        m.obj("bed", ref, id=bed_id, dy=-1, owner=owner, name="Bunk")
    m.obj("container", ("L", 0), id="player_locker", name="Your locker",
          items={"ration_pack": 2, "coffee_bulb": 1})
    m.obj("container", ("L", 1), id="locker_wren", name="Wren's locker", owner="wren", restricted=True,
          items={"fresh_greens": 2, "seed_stock": 1})
    m.obj("container", ("L", 2), id="locker_osei", name="Administrator's locker", owner="osei", restricted=True,
          locked=True, access="command", lock_level=16, items={"keycard_cmd": 1, "synth_whiskey": 1})
    m.obj("container", ("L", 3), id="locker_bram", name="Bram's locker", owner="bram", restricted=True,
          items={"multitool": 1, "coffee_bulb": 2})
    m.obj("container", ("L", 4), id="locker_lin", name="Lin's locker", owner="lin", restricted=True,
          locked=True, access="science", lock_level=14, items={"field_notes": 1, "nutrient_paste": 2})
    m.obj("terminal", ("m", 0), id="osei_quarters_terminal", name="Personal terminal", dialogue="terminal_osei_private")
    m.region("crew_corridor", "Crew Corridor", 1, 7, 28, 3)
    m.region("quarters_a", "Bunk Room A", 1, 1, 8, 5)
    m.region("quarters_b", "Bunk Room B", 10, 1, 8, 5)
    m.region("quarters_senior", "Senior Quarters", 19, 1, 10, 5)
    m.region("quarters_c", "Bunk Room C", 1, 11, 8, 6)
    m.region("quarters_d", "Bunk Room D", 10, 11, 8, 6)
    m.region("rec_room", "Rec Room", 19, 11, 10, 6)

    # -------------------------------------------------------------- command
    b["command"] = m = MapBuilder("command", "Command Deck", [
        "#########WWWW####WWWW#########",
        "#.S....p#.k.k....k.k.#.L.S...#",
        "#.......#............#.......#",
        "#..dm...#............#..dk...#",
        "#..t....#............#..t....#",
        "#.......2............3.......#",
        "#.......#.....H......#.......#",
        "#.......#............#.......#",
        "#.......#............#.......#",
        "#########............#########",
        "#.......#......7.....#.......#",
        "#.a.V.V.#............#.......#",
        "#.......#............#..##6###",
        "#.......4............5..#....#",
        "#.......#............#..#...B#",
        "#.......#............#..#....#",
        "#.......#............#..#....#",
        "#########............#########",
        "        #............#        ",
        "        #######1######        ",
    ], over=[
        "    c        (           !",
        "",
        " rrrrr",
        " rrrrr       l",
        " rrrrr",
        " rrrrr",
        " rrrrr",
        "    l                    l",
        "",
        "   p  w               !",
        "",
        "",
        "              l",
        "",
        "    l",
        "                          l",
        "",
        "",
        "         >>>>>>>>>>>",
    ], props={"backdrop": "saturn", "ambient": [208, 214, 236]})
    m.obj("portal", "1", id="command_to_hub", sprite="door_h", target=("hub", "1"), label="Central Ring")
    m.obj("door", "2", id="admin_door", sprite="door_v")
    m.obj("door", "3", id="security_door", sprite="door_v")
    m.obj("door", "4", id="comms_door", sprite="door_v")
    m.obj("door", "5", id="brig_door", sprite="door_v")
    m.obj("door", "6", id="cell_door", sprite="door_secure", locked=True, access="security", lock_level=19)
    m.obj("marker", "7", id="patrol_command", name="patrol_command")
    m.obj("terminal", ("m", 0), id="osei_terminal", name="Administrator's terminal", dialogue="terminal_osei")
    m.obj("terminal", ("k", 0), id="ops_console_1", name="Station status board", dialogue="terminal_status")
    m.obj("terminal", ("k", 2), id="ops_console_2", name="Traffic console", dialogue="terminal_traffic")
    m.obj("terminal", ("k", 4), id="security_terminal", name="Security terminal", dialogue="terminal_security")
    m.obj("console", ("a", 0), id="comms_console", system="comms", name="Comms array console")
    m.obj("container", ("L", 0), id="sec_locker", name="Security locker", owner="halloran", restricted=True,
          locked=True, access="security", lock_level=17,
          items={"mining_charge": 2, "signal_amplifier": 1})
    m.obj("container", ("S", 0), id="admin_shelf", name="Records shelf", owner="osei", restricted=True,
          items={"station_charter": 1})
    m.obj("container", ("S", 1), id="evidence_shelf", name="Evidence shelf", owner="halloran", restricted=True,
          items={})
    m.obj("bed", ("B", 0), id="brig_bunk", dy=-1, owner="", name="Brig bunk")
    m.obj("sign", ("H", 0), id="holo_table", text="A slow-turning hologram of Tethys. A red pin marks Ithaca Station on the lip of the great chasm. A second, unlabelled pin sits a kilometre beneath it.")
    m.region("ops", "Operations", 9, 1, 12, 17)
    m.region("admin_office", "Administrator's Office", 1, 1, 7, 8)
    m.region("security", "Security Office", 22, 1, 7, 8)
    m.region("comms_room", "Comms Room", 1, 10, 7, 7)
    m.region("brig_room", "Brig", 22, 10, 7, 7)
    m.region("brig_cell", "Holding Cell", 25, 13, 4, 4)

    # ---------------------------------------------------------- engineering
    b["engineering"] = m = MapBuilder("engineering", "Engineering", [
        "################################",
        "#q.q.....w.w.#..L.S......n.n...#",
        "#............#.................#",
        "#............#.................#",
        "#..._______..#.................#",
        "#..._______..#...F..k.....a....#",
        "#..._______..#.................#",
        "#..._______..2.................#",
        "#..._R_____..#....TT...........1",
        "#..._______..#.................#",
        "#...__k__c_..#........3........#",
        "#............#......cC.........#",
        "#............#.................#",
        "#.b.b........#..C.c........L.L.#",
        "#............#.................#",
        "################################",
    ], over=[
        "     ! pp       e    p   w",
        "",
        "  x       x      l         l",
        "",
        "",
        "",
        "",
        "         ----",
        "",
        "    l                l",
        "",
        " o          ",
        "  z z z z z",
        "",
        "       l              l",
    ], wall="wall_dark", floors={".": "floor_dark"}, props={"backdrop": "none", "ambient": [196, 200, 216]})
    m.obj("portal", "1", id="eng_to_hub", sprite="door_v", target=("hub", "4"), label="Central Ring")
    m.obj("door", "2", id="reactor_door", sprite="door_v")
    m.obj("marker", "3", id="patrol_eng", name="patrol_eng")
    m.obj("console", ("k", 0), id="fab_console", system="fabricator", name="Fabricator console")
    m.obj("console", ("k", 1), id="reactor_console", system="reactor", name="Reactor control")
    m.obj("console", ("a", 0), id="grid_console", system="grid", name="Power distribution board")
    m.obj("container", ("c", 0), id="reactor_hopper", name="Reactor fuel hopper", machine=True,
          owner="engineering", restricted=True, items={"fuel_cell": 1})
    # engineering starts with empty shelves: every spare on the station is in the quartermaster's hands
    m.obj("container", ("L", 0), id="parts_locker", name="Parts locker", owner="engineering", restricted=True,
          items={})
    m.obj("container", ("S", 0), id="fab_feed", name="Fabricator feed bin", machine=True,
          owner="engineering", restricted=True, items={"scrap_metal": 4})
    m.obj("container", ("L", 1), id="tool_locker", name="Tool locker", owner="engineering", restricted=True,
          items={"diag_scanner": 1, "scrap_metal": 2})
    m.obj("container", ("L", 2), id="eng_locker", name="Engineering locker", owner="engineering", restricted=True,
          locked=True, access="engineering", lock_level=14, items={"keycard_eng": 1})
    m.obj("sign", ("R", 0), id="reactor_sign", dx=1, text="The containment vessel hums a note you feel in your teeth. Someone has chalked a tally on the casing: days since last scram. The number has been rubbed out and rewritten many times.")
    m.region("reactor_hall", "Reactor Hall", 1, 1, 12, 14)
    m.region("workshop", "Workshop", 14, 1, 17, 14)

    # ---------------------------------------------------------- hydroponics
    b["hydroponics"] = m = MapBuilder("hydroponics", "Hydroponics", [
        "############################",
        "#w.w.j.j.#.................#",
        "#........#.PPPP.PPPP.PPPP..#",
        "#.u..k...#.................#",
        "#........#.PPPP.PPPP.PPPP..#",
        "#........2.................#",
        "#........#.PPPP.PPPP.PPPP..#",
        "#.b.b....#.................#",
        "#........#.PPPP.PPPP.PPPP..1",
        "##########.................#",
        "#S.S.L...#.PPPP.PPPP.PPPP..#",
        "#........#.................#",
        "#........3.g...............#",
        "#........#.................#",
        "#..T.t...#..p...........p..#",
        "############################",
    ], over=[
        "     p  !       b      w",
        "            g    g    g",
        "",
        "            g    g    g",
        "  |",
        "  +-----    g    g    g",
        "",
        "            g    g    g",
        "",
        "            g    g    g",
        "",
        "",
        "    l",
        "                  l",
    ], wall="wall_lab", floors={".": "floor_lab"}, props={"backdrop": "none", "ambient": [226, 232, 226]})
    m.obj("portal", "1", id="hydro_to_hub", sprite="door_v", target=("hub", "2"), label="Central Ring")
    m.obj("door", "2", id="water_door", sprite="door_v")
    m.obj("door", "3", id="seed_door", sprite="door_v")
    m.obj("console", ("k", 0), id="water_console", system="water_reclaim", name="Water reclamation console")
    m.obj("console", ("g", 0), id="hydro_console", system="hydroponics", name="Hydroponics console")
    m.obj("container", ("S", 0), id="hydro_store", name="Seed store", owner="wren", restricted=True,
          items={"seed_stock": 3, "fresh_greens": 3})
    m.obj("container", ("S", 1), id="hydro_shelf", name="Nutrient shelf", owner="wren", restricted=True,
          items={"nutrient_paste": 4})
    m.obj("container", ("L", 0), id="hydro_locker", name="Botany locker", owner="wren", restricted=True,
          items={"thermal_suit": 1})
    m.obj("interact", ("u", 0), id="pump", name="Reclamation pump", dialogue="pump")
    m.region("grow_bay", "Grow Bay", 10, 1, 17, 14)
    m.region("water_plant", "Water Reclamation", 1, 1, 8, 8)
    m.region("seed_store", "Seed Store", 1, 10, 8, 5)

    # --------------------------------------------------------------- medlab
    b["medlab"] = m = MapBuilder("medlab", "Medical & Research", [
        "##############################",
        "#.L.S........#..V.V.V....L...#",
        "#............#...............#",
        "#.M..M..M....#...k....TT.....#",
        "#............#...............#",
        "#.........dg.2...............#",
        "#.........t..#......*........#",
        "#............#...............#",
        "1............#..Y.Y.Y........#",
        "#............#...............#",
        "#..p.........#..........p....#",
        "##############################",
    ], over=[
        "      m   w           s    n",
        "",
        "    l                  l",
        "",
        "",
        "",
        "",
        "    l                    l",
        " >>>>>>>>>>>",
    ], wall="wall_lab", floors={".": "floor_lab"}, props={"backdrop": "none", "ambient": [228, 232, 240]})
    m.obj("portal", "1", id="medlab_to_hub", sprite="door_v", target=("hub", "3"), label="Central Ring")
    m.obj("door", "2", id="lab_door", sprite="door_v")
    m.obj("console", ("g", 0), id="med_console", system="medbay", name="Medbay console")
    m.obj("console", ("k", 0), id="lab_console", system="lab", name="Signal analysis rig")
    m.obj("bed", ("M", 0), id="medbay_bed", dy=-1, owner="", name="Med bed")
    m.obj("bed", ("M", 1), id="medbay_bed_2", dy=-1, owner="", name="Med bed")
    m.obj("container", ("L", 0), id="med_cabinet", name="Medicine cabinet", owner="pell", restricted=True,
          locked=True, access="medical", lock_level=14, items={"medkit": 3, "stim": 1, "sedative": 2})
    m.obj("container", ("S", 0), id="med_shelf", name="Medical supplies", owner="pell", restricted=True,
          items={"medkit": 1, "nutrient_paste": 2})
    m.obj("container", ("L", 1), id="lab_locker", name="Lab locker", owner="lin", restricted=True,
          locked=True, access="science", lock_level=15, items={"keycard_sci": 1, "artifact_shard": 1})
    m.obj("interact", ("*", 0), id="lab_sample", name="Crystal sample", dialogue="lab_sample")
    m.obj("sign", ("Y", 1), id="cryo_sign", text="Emergency cryo berths. Three pods, all dark. The inspection tags expired the year the Combine cut the budget.")
    m.region("medbay", "Medbay", 1, 1, 12, 10)
    m.region("lab", "Research Lab", 14, 1, 15, 10)

    # ----------------------------------------------------------- excavation
    b["excavation"] = m = MapBuilder("excavation", "The Dig", [
        "&&&&&&&###########################",
        "&_____&......#######......########",
        "&_____&..l.....c###..........#####",
        "1_____..............,,,.......####",
        "&_____&......,,,,a...,,,,....l.###",
        "&_____&###...,Z,,,.....,,.......##",
        "&&&&&&&####..,,,,,..##.....2.....#",
        "##########....,,,..####....^.....#",
        "###########.......######.........#",
        "############.....#######....x....#",
        "############....########.........#",
        "###########....#########........##",
        "##########l...##########......####",
        "##########...###########.....#####",
        "#########....############...######",
        "########.....############...######",
        "########......###########.3.######",
        "#########......####.......,,.#####",
        "###########.........,,,,,,,,,,####",
        "#############....,,,,,,,,,,,,,,###",
        "###############.,,,,,,O,,4,,,,,###",
        "###############,,,,,,,,,,,,,,,,###",
        "################,,,*,,,,,,*,,,####",
        "##################,,,,,,,,,,######",
        "##################################",
    ], over=[
        "   !",
        "  l          q     q      q",
        "",
        " >>>>>     f       f",
        "",
        "            f           f",
        "",
        "                          f",
        "             f",
        "",
        "                           i",
        "             i",
        "",
        "                           f",
        "           f",
        "            i",
        "",
        "            f",
        "              i    yy  yy",
        "                   y      y",
        "                  yy      yy",
        "                   y      y",
        "                    yy  yy",
    ], wall="wall_ice", floors={".": "ice_floor", ",": "rock_floor"},
        props={"backdrop": "none", "ambient": [78, 96, 150], "cold": True, "own_light": True})
    m.obj("portal", "1", id="lift_to_hangar", sprite="door_v", target=("hangar", "3"), label="Hangar Bay")
    m.obj("marker", "2", id="dig_spot", name="dig_spot")
    m.obj("trigger", "3", id="chamber_trigger", on_enter=["start_dialogue('chamber_first')"], once=True)
    m.obj("marker", "4", id="chamber_spot", name="chamber_spot")
    m.obj("console", ("a", 0), id="drill_console", system="drill", name="Bore rig controls")
    m.obj("container", ("c", 0), id="dig_crate", name="Core sample crate", items={"ice_core": 3, "scrap_metal": 3})
    m.obj("interact", ("O", 0), id="artifact", name="The artifact", dialogue="artifact")
    m.obj("interact", ("O", 0), id="artifact_side", dx=1, name="The artifact", dialogue="artifact")
    m.obj("item", "4", id="chamber_shard", dx=2, dy=1, item="artifact_shard", qty=1)
    m.obj("sign", ("^", 0), id="spire_sign", text="A spire of blue ice, older than the station by a few million years. Someone has hung a hard hat on it.")
    m.region("lift", "Excavation Lift", 1, 1, 5, 5)
    m.region("dig_site", "Dig Site", 7, 1, 26, 10)
    m.region("tunnels", "Ice Tunnels", 8, 11, 21, 6)
    m.region("artifact_chamber", "The Chamber", 15, 17, 16, 7)

    # resolve portal targets now that every map exists
    maps: Dict[str, GameMap] = {}
    for map_id, builder in b.items():
        for obj in builder.gmap.objects:
            target = obj.props.pop("target", None)
            if target:
                other, digit = target
                tx, ty = b[other].arrival(digit)
                obj.props["target_map"] = other
                obj.props["target_x"] = tx
                obj.props["target_y"] = ty
                dx, dy = tx - b[other].anchors[digit][0], ty - b[other].anchors[digit][1]
                obj.props["facing"] = config.dir_from_delta(dx, dy)
        maps[map_id] = builder.gmap
    return maps


def main(argv=None) -> int:
    import argparse
    parser = argparse.ArgumentParser(
        prog="python -m game.tools.seed_maps",
        description="Rebuild the shipped station maps from the layouts in this file. "
                    "Existing map files are kept unless --force is given.")
    parser.add_argument("--force", action="store_true",
                        help="overwrite existing maps (hand edits made in the map editor are lost)")
    parser.add_argument("--data", default=None, help="content directory (default: ./data)")
    opts = parser.parse_args(argv)
    force = opts.force
    out_dir = os.path.join(opts.data or config.data_dir(), "maps")
    os.makedirs(out_dir, exist_ok=True)
    written = 0
    for map_id, gmap in build_all().items():
        path = os.path.join(out_dir, map_id + ".json")
        if os.path.exists(path) and not force:
            print("keep   %s (exists; use --force to overwrite)" % path)
            continue
        gmap.save(path)
        written += 1
        print("wrote  %s  (%dx%d, %d objects)" % (path, gmap.width, gmap.height, len(gmap.objects)))
    print("%d map(s) written" % written)
    return 0


if __name__ == "__main__":
    sys.exit(main())
