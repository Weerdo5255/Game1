"""NPC behaviour: routines, self-set goals and off-screen activity."""
from game.sim import ai
from tests.helpers import place_player, run_hours


def signature(world):
    """A compact fingerprint of the simulation state."""
    chars = tuple((c.id, c.map_id, c.x, c.y, round(c.needs["hunger"], 2), round(c.hp, 2), c.ai.get("goal", ""))
                  for c in sorted(world.characters.values(), key=lambda c: c.id))
    systems = tuple((s.id, round(s.condition, 3), round(s.efficiency, 3)) for s in world.systems.systems.values())
    return (round(world.clock.minutes, 3), chars, systems, tuple(sorted(world.systems.resources.items())))


def test_schedule_puts_people_where_they_should_be(world):
    run_hours(world, 3.0)                     # 10:30 on day 1
    wren, bram = world.characters["wren"], world.characters["bram"]
    assert wren.map_id == "hydroponics" and "Hydroponics" in wren.activity
    # the chief engineer goes wherever something needs him, so only check that he is at work
    assert bram.activity and not bram.asleep and bram.ai.get("goal")
    assert world.systems.get("hydroponics").operator_id == "wren"
    run_hours(world, 16.5)                    # 03:00 on day 2
    asleep = [c.id for c in world.npcs() if c.asleep]
    assert "wren" in asleep and "bram" in asleep and "pell" in asleep
    assert world.characters["wren"].map_id == "habitat"
    assert "mote" not in asleep, "drones do not sleep"


def test_npcs_cross_maps_while_the_player_is_elsewhere(world):
    place_player(world, "excavation", 8, 3)
    visited = {c.id: {c.map_id} for c in world.npcs()}
    for _ in range(24):
        run_hours(world, 1)
        for c in world.npcs():
            visited[c.id].add(c.map_id)
    assert world.player.map_id == "excavation"
    travellers = [cid for cid, maps in visited.items() if len(maps) >= 3]
    assert len(travellers) >= 7, visited
    assert {"hub", "habitat"} <= visited["bram"]


def test_hungry_npcs_find_food_for_themselves(world):
    items = world.content.items
    wren = world.characters["wren"]
    wren.needs["hunger"] = 82.0

    def food_on_station():
        return (sum(world.inventory_of(o).count_tag(items, "food") for o in world.containers())
                + sum(c.inventory.count_tag(items, "food") for c in world.characters.values()))
    world.systems.get("hydroponics").enabled = False      # no new rations during the test
    before = food_on_station()
    run_hours(world, 1.5)
    assert wren.needs["hunger"] < 50
    assert food_on_station() < before, "she ate something real, from her pack or the nearest store"


def test_blocked_goal_records_what_is_missing(world):
    lin = world.characters["lin"]
    run_hours(world, 0.2)
    goal = ai.find_goal(lin, "decode_signal")
    assert goal["status"] == "active" and goal.get("blocks", 0) >= 1
    assert lin.wants.get("signal_amplifier") == 1
    assert any("stuck" in e["text"] and "Lin" in e["text"] for e in world.log)
    # hand it over and she gets on with it
    lin.inventory.add("signal_amplifier", 1)
    goal["retry_at"] = 0
    run_hours(world, 3)
    assert "signal_amplifier" not in lin.wants
    assert world.flags.get("signal_progress", 0) > 0


def test_engineer_repairs_with_parts_he_fetches(world):
    reactor = world.systems.get("reactor")
    reactor.condition = 40.0
    locker = world.inventory_of(world.find_object("parts_locker"))
    locker.items["spare_parts"] = 3
    run_hours(world, 4)
    assert reactor.condition > 75
    assert locker.count("spare_parts") < 3
    assert any("repaired Fusion Reactor" in e["text"] for e in world.log)


def test_engineer_cannot_raid_the_quartermasters_stores(world):
    for obj_id in ("parts_locker", "eng_locker", "tool_locker"):
        world.inventory_of(world.find_object(obj_id)).items.pop("spare_parts", None)
    world.systems.get("fabricator").enabled = False
    world.systems.get("reactor").condition = 55.0
    stores = world.inventory_of(world.find_object("stores_parts"))
    before = stores.count("spare_parts")
    run_hours(world, 6)
    assert stores.count("spare_parts") == before
    assert world.characters["bram"].wants.get("spare_parts")
    # ...until the Administrator opens them
    world.script.run("set_obj('stores_parts', 'access', 'engineering,stores')")
    run_hours(world, 6)
    assert stores.count("spare_parts") < before


def test_starving_crew_help_themselves_to_the_stores(world):
    """Hungry people respect the quartermaster's shelves; starving people do not."""
    items = world.content.items
    world.systems.get("hydroponics").enabled = False

    def strip_food(inventory):
        for item_id in [i for i in inventory.items if "food" in items[i].tags]:
            inventory.items.pop(item_id)
    for obj in world.containers():
        if obj.id != "stores_food":                 # the only food left is on Dex's restricted shelf
            strip_food(world.inventory_of(obj))
    for ch in world.characters.values():
        strip_food(ch.inventory)
        ch.needs["hunger"] = 0.0
    dex = world.characters["dex"]                   # ...and he is not handing any of it out today
    dex.goals = [g for g in dex.goals if g["id"] != "restock_galley"]
    stores = world.inventory_of(world.find_object("stores_food"))
    before = stores.count_tag(items, "food")
    tuck = world.characters["tuck"]
    tuck.needs["hunger"] = 70.0
    run_hours(world, 2)
    assert stores.count_tag(items, "food") == before, "merely hungry: he goes without"
    assert tuck.wants and tuck.needs["hunger"] > 70
    tuck.needs["hunger"] = 92.0
    run_hours(world, 2.5)
    assert stores.count_tag(items, "food") < before
    assert tuck.needs["hunger"] < 85, "he ate what he took"
    assert any("Tuck" in e["text"] and "without asking" in e["text"] for e in world.log)


def test_reactor_gets_refuelled(world):
    reactor = world.systems.get("reactor")
    reactor.fuel_left = 2.0
    hopper = world.inventory_of(world.find_object("reactor_hopper"))
    hopper.items.clear()
    run_hours(world, 6)
    assert reactor.fuel_left > 6 or hopper.count("fuel_cell") > 0
    assert reactor.output > 0


def test_dex_stocks_the_shuttle_at_night_unseen(world):
    place_player(world, "habitat", 3, 3)
    hold = world.inventory_of(world.find_object("shuttle_hold"))
    assert hold.empty
    run_hours(world, 21)                      # 04:30 on day 2: the first night run is over
    assert hold.count("ration_pack") >= 4
    assert not world.flags.get("saw_dex_loading")
    run_hours(world, 48)
    assert hold.count("fuel_cell") == 2 and world.flags.get("magpie_stocked")


def test_goal_templates_and_scripted_goals(world):
    pip = world.characters["pip"]
    world.script.run("add_goal('pip', 'stockpile', item='coffee_bulb', count=2, stash='tool_locker', label='coffee', id='hoard')")
    goal = ai.find_goal(pip, "hoard")
    assert goal["desc"] == "Stockpile coffee" and goal["steps"][0]["count"] == 2
    run_hours(world, 4)
    assert world.inventory_of(world.find_object("tool_locker")).count("coffee_bulb") >= 1
    assert world.script.eval("goal_done('pip', 'hoard')")
    world.script.run("cancel_goal('pip', 'repair_comms')")
    assert ai.find_goal(pip, "repair_comms") is None


def test_when_starts_a_goal_and_the_job_is_then_finished(world):
    """Emptying the shelf makes the start condition false; she must still deliver."""
    wren = world.characters["wren"]
    wren.needs["hunger"] = 0.0
    shelf = world.inventory_of(world.find_object("hydro_shelf"))
    store = world.inventory_of(world.find_object("hydro_store"))
    have = shelf.count("nutrient_paste")
    assert have > 0 and store.count("nutrient_paste") == 0
    ai.add_goal(world, wren, ai.make_goal("tidy", "Tidy the shelf", 75, [
        {"do": "acquire", "item": "nutrient_paste", "count": have, "from": ["hydro_shelf"]},
        {"do": "deliver", "item": "nutrient_paste", "to": "hydro_store"},
    ], when="container_count('hydro_shelf', 'nutrient_paste') > 0"))
    run_hours(world, 1.5)
    assert shelf.count("nutrient_paste") == 0 and wren.inventory.count("nutrient_paste") == 0
    assert store.count("nutrient_paste") == have
    assert ai.find_goal(wren, "tidy")["status"] == "done"


def test_when_is_not_met_so_the_goal_never_starts(world):
    wren = world.characters["wren"]
    shelf = world.inventory_of(world.find_object("hydro_shelf"))
    before = shelf.count("nutrient_paste")
    ai.add_goal(world, wren, ai.make_goal("tidy", "Tidy the shelf", 75, [
        {"do": "acquire", "item": "nutrient_paste", "count": 1, "from": ["hydro_shelf"]},
        {"do": "deliver", "item": "nutrient_paste", "to": "hydro_store"},
    ], when="flag('inspection_due')"))
    run_hours(world, 1.5)
    assert shelf.count("nutrient_paste") == before and not ai.find_goal(wren, "tidy").get("begun")
    world.flags["inspection_due"] = True
    run_hours(world, 1.5)
    assert ai.find_goal(wren, "tidy")["status"] == "done"


def test_while_pauses_a_goal_where_it_stands(world):
    """`while` is checked the whole time: the goal is put down and picked up again."""
    wren = world.characters["wren"]
    wren.needs["hunger"] = 0.0
    store = world.inventory_of(world.find_object("hydro_store"))
    wren.inventory.add("nutrient_paste", 2)
    world.flags["quiet"] = True
    ai.add_goal(world, wren, ai.make_goal("tidy", "Tidy up", 75, [
        {"do": "run", "script": ["set_flag('tidy_begun')"]},
        {"do": "deliver", "item": "nutrient_paste", "to": "hydro_store"},
    ], **{"while": "not flag('quiet')"}))
    run_hours(world, 1)
    assert not world.flags.get("tidy_begun") and store.count("nutrient_paste") == 0
    world.flags["quiet"] = False
    run_hours(world, 0.05)
    assert world.flags.get("tidy_begun") and ai.find_goal(wren, "tidy")["step"] == 1
    world.flags["quiet"] = True                  # interrupted on the way to the store room
    wren.ai["think"] = 0.0                       # (she reconsiders at once rather than in a minute or two)
    run_hours(world, 1)
    goal = ai.find_goal(wren, "tidy")
    assert goal["status"] == "active" and goal["step"] == 1 and store.count("nutrient_paste") == 0
    world.flags["quiet"] = False
    run_hours(world, 1)
    assert store.count("nutrient_paste") == 2 and goal["status"] == "done"


def test_quartermaster_delivers_what_he_takes_for_the_galley(world):
    """Taking the rations used to make his own start condition false, leaving
    him wandering about with the station's lunch in his pockets."""
    dex = world.characters["dex"]
    dex.needs["hunger"] = 0.0
    galley = world.inventory_of(world.find_object("galley_dispenser"))
    stores = world.inventory_of(world.find_object("stores_food"))
    galley.items.pop("ration_pack", None)
    stores.items["ration_pack"] = 5              # after he takes four, fewer than the trigger asks for
    run_hours(world, 3)
    assert galley.count("ration_pack") >= 3, "allowing for somebody having an early lunch"
    assert dex.inventory.count("ration_pack") == 0


def test_a_broken_goal_fails_without_stopping_the_world(world):
    world.script.strict = False
    pip = world.characters["pip"]
    ai.add_goal(world, pip, ai.make_goal("bad", "Do something impossible", 95,
                                         [{"do": "acquire", "item": "ration_pack", "count": "lots"}]))
    run_hours(world, 1)
    assert ai.find_goal(pip, "bad")["status"] == "failed"
    assert any("bad" in e for e in world.script.errors)
    assert "Fabricator" in pip.activity or pip.ai.get("goal")


def test_simulation_is_deterministic(content):
    from game.world.world import World
    runs = []
    for _ in range(2):
        w = World.new_game(content, {"background": "scout"}, seed=42)
        w.requests.clear()
        run_hours(w, 30)
        runs.append(signature(w))
    assert runs[0] == runs[1]


def test_fast_forward_matches_frame_by_frame(content):
    from game.world.world import World
    a = World.new_game(content, seed=5)
    b = World.new_game(content, seed=5)
    a.fast_forward(120)                       # two game hours in big slices
    for _ in range(int(120 / (1 / 60.0) / 1)):   # the same two hours at 60 frames per second
        b.update(1 / 60.0)
        if b.clock.minutes >= a.clock.minutes:
            break
    for sid in a.systems.systems:
        assert abs(a.systems.get(sid).condition - b.systems.get(sid).condition) < 1.5, sid
    assert abs(a.systems.resource("water") - b.systems.resource("water")) < 8
    same_map = sum(1 for cid in a.characters if a.characters[cid].map_id == b.characters[cid].map_id)
    assert same_map >= len(a.characters) - 2
