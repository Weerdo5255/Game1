from game.world import movement
from game.world.character import Character, relation_label
from game.world.clock import GameClock
from game.world.gamemap import GameMap
from game.world.items import Inventory, format_item_list, parse_item_list
from game.world.pathfinding import can_open, find_path


def test_clock():
    clock = GameClock(GameClock.to_minutes(2, 23, 30))
    assert (clock.day, clock.hour, clock.minute) == (2, 23, 30)
    assert clock.is_between(22, 6) and not clock.is_between(6, 22)
    clock.advance(45)
    assert (clock.day, clock.time_str()) == (3, "00:15")
    assert clock.stamp() == "Day 3 00:15"


def test_inventory():
    inv = Inventory({"ration_pack": 2})
    inv.add("spare_parts", 3)
    assert inv.has("spare_parts", 3) and not inv.has("spare_parts", 4)
    assert inv.remove("spare_parts", 5) == 3 and "spare_parts" not in inv.items
    other = Inventory()
    assert inv.transfer(other, "ration_pack", 1) == 1 and other.count("ration_pack") == 1
    assert parse_item_list("ration_pack:3, multitool ; junk:x") == {"ration_pack": 3, "multitool": 1, "junk": 1}
    assert format_item_list({"b": 1, "a": 2}) == "a:2, b"


def test_item_tags(content):
    inv = Inventory({"ration_pack": 1, "spare_parts": 2, "keycard_eng": 1})
    assert inv.find_tag(content.items, "food") == "ration_pack"
    assert inv.count_tag(content.items, "part") == 2
    assert content.items["keycard_eng"].access_groups() == ["engineering"]
    assert abs(inv.weight(content.items) - (0.4 + 3.0)) < 1e-6


def test_map_roundtrip(tmp_path):
    gmap = GameMap.new("t", "Test", 6, 5, "floor_plate")
    for x in range(6):
        gmap.set_tile("ground", x, 0, "wall")
    gmap.set_tile("objects", 2, 2, "crate")
    gmap.add_object("container", 2, 2, id="box", name="Box", items={"ration_pack": 2})
    gmap.add_object("portal", 5, 4, target_map="t", target_x=1, target_y=1)
    from game.world.gamemap import Region
    gmap.regions.append(Region("r", "Room", 1, 1, 3, 3, {"cold": True}))
    path = tmp_path / "t.json"
    gmap.save(str(path))
    again = GameMap.load(str(path))
    assert again.to_dict() == gmap.to_dict()
    assert again.solid_at(0, 0) and again.solid_at(2, 2) and not again.solid_at(1, 1)
    assert again.solid_at(-1, 0) and again.solid_at(6, 0)
    assert again.get_object("box").get("items") == {"ration_pack": 2}
    assert again.region_at(2, 2).id == "r" and again.place_name(4, 4) == "Test"
    assert again.objects_of("portal")[0].id == "t_portal_1"


def test_map_resize_keeps_content():
    gmap = GameMap.new("t", "Test", 4, 4, "floor_plate")
    gmap.add_object("marker", 1, 1, id="m")
    gmap.resize(6, 6, 1, 2)
    assert (gmap.width, gmap.height) == (6, 6)
    assert gmap.tile("ground", 1, 2) == "floor_plate" and gmap.tile("ground", 0, 0) == ""
    assert gmap.get_object("m").pos == (2, 3)


def test_autotile_mask_and_void(content):
    hub = content.load_map("hub")
    from game.art import tiles
    top_wall = hub.mask("ground", 2, 0)
    assert top_wall & tiles.E and top_wall & tiles.W and not top_wall & tiles.SO
    hangar = content.load_map("hangar")
    assert hangar.tile("ground", 0, 23) == "" and hangar.solid_at(0, 23)


def test_find_path():
    walls = {(1, 0), (1, 1), (1, 2)}

    def passable(x, y):
        return 0 <= x < 4 and 0 <= y < 4 and (x, y) not in walls
    path = find_path((0, 0), [(3, 0)], passable)
    assert path[-1] == (3, 0) and len(path) == 9
    assert find_path((0, 0), [(0, 0)], passable) == []
    assert find_path((0, 0), [(9, 9)], passable) is None
    # goal cells are enterable even if "solid" (consoles, portals)
    assert find_path((0, 0), [(1, 0)], passable) == [(1, 0)]


def test_character_stats(content):
    ch = Character.from_def("x", {"attrs": {"body": 6, "dexterity": 8}, "skills": {"engineering": 4},
                                  "inventory": {"multitool": 1, "thermal_suit": 1}}, content)
    assert ch.skill("engineering") == 4 + 2           # dexterity 8 gives +2
    assert ch.equip("multitool") and ch.skill("engineering") == 7
    ch.auto_effects = ["hungry"]
    ch.invalidate()
    assert ch.skill("engineering") == 6 and ch.skill("stealth") == 1
    assert ch.max_hp == 20 + 6 * 4
    ch.inventory.remove("multitool", 1)
    ch.check_equipment()
    assert not ch.is_equipped("multitool")
    assert ch.equip("thermal_suit") and ch.mods().get("cold_resist") == 1


def test_xp_and_skill_points(content):
    ch = Character.from_def("x", {}, content)
    assert ch.add_xp(300) == 2 and ch.level == 3 and ch.skill_points == 4
    assert ch.raise_skill("tech") and ch.skills["tech"] == 1 and ch.skill_points == 3
    assert not ch.raise_skill("no_such_skill")


def test_relations(content):
    ch = Character.from_def("x", {"relations": {"player": 10}}, content)
    assert ch.adjust_relation("player", 500) == 100 and relation_label(100) == "Devoted"
    assert relation_label(-60) == "Hostile" and relation_label(0) == "Neutral"


def test_character_roundtrip(world):
    bram = world.characters["bram"]
    bram.add_effect("stimmed", until=999.0)
    bram.wants["spare_parts"] = 2
    clone = Character.from_dict(bram.to_dict(), world.content)
    assert clone.to_dict() == bram.to_dict()
    assert clone.skill("engineering") == bram.skill("engineering")


def test_locked_doors_and_access(world):
    cell = world.find_object("cell_door")
    player, halloran = world.player, world.characters["halloran"]
    assert not can_open(player, cell) and can_open(halloran, cell)
    world.give_item(player, "keycard_sec", 1, quiet=True)
    assert can_open(player, cell)
    fuel = world.find_object("stores_fuel")
    assert can_open(world.characters["bram"], fuel) and not can_open(world.characters["wren"], fuel)


def test_movement_and_portals(world):
    player = world.player
    assert movement.try_step(world, player, "up") is None and player.moving
    for _ in range(40):
        movement.advance(world, player, 0.05)
    assert not player.moving and player.pos == (15, 19)
    assert movement.blocked(world, player, 12, 21) == "solid"
    # walking onto a portal asks the view to change maps; the logic does it via use_portal
    portal = world.find_object("hangar_to_hub")
    world.transfer(player, "hangar", portal.x, portal.y + 1)
    world.requests.clear()
    movement.try_step(world, player, "up")
    for _ in range(40):
        movement.advance(world, player, 0.05)
    assert world.requests and world.requests[0]["type"] == "portal"
    movement.use_portal(world, player, portal)
    assert player.map_id == "hub" and not world.maps["hub"].solid_at(player.x, player.y)


def test_every_portal_pair_is_consistent(world):
    for map_id, gmap in world.maps.items():
        for portal in gmap.objects_of("portal"):
            target = world.maps[portal.get("target_map")]
            tx, ty = portal.get("target_x"), portal.get("target_y")
            assert not target.solid_at(tx, ty), portal.id
            back = [p for p in target.objects_of("portal") if p.get("target_map") == map_id]
            assert back, "no way back from %s" % portal.id


def test_everyone_can_reach_work_and_bed(world):
    for npc in world.npcs():
        targets = []
        if npc.home.get("bed"):
            targets.append(world.find_object(npc.home["bed"]))
        system = world.systems.get((npc.job or {}).get("system", ""))
        if system is not None and system.console:
            targets.append(world.find_object(system.console))
        for obj in targets:
            route = world.nav.map_route(npc, npc.map_id, obj.map_id)
            assert route is not None, "%s cannot reach %s" % (npc.id, obj.id)
            assert world.nav.approach_cells(npc, obj.map_id, obj.x, obj.y), obj.id
