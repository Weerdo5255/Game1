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
