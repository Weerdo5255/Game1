import pytest

from game.sim.systems import SystemNetwork, condition_factor, status_for
from tests.helpers import run_hours


def test_condition_curve():
    assert condition_factor(100) == 1.0 and condition_factor(75) == 1.0
    assert 0.35 < condition_factor(50) < 1.0
    assert condition_factor(25) == pytest.approx(0.35)
    assert condition_factor(5) == 0.0 and condition_factor(0) == 0.0
    assert [status_for(c) for c in (90, 45, 20, 3)] == ["ok", "degraded", "critical", "offline"]


def test_power_is_allocated_by_priority(world):
    s = world.systems
    for system in s.systems.values():
        system.condition = 100.0
        system.labour = 1.0
    s.get("reactor").condition = 30.0          # badly degraded: not enough for everybody
    s.tick(world, 1e-6)
    assert s.brownout and s.power_supply < s.power_demand
    order = s.consumers()
    ratios = [c.power_ratio for c in order if c.power > 0]
    assert ratios[0] == 1.0, "the first system in the queue is fully powered"
    assert ratios == sorted(ratios, reverse=True), "nobody later in the queue gets more than someone earlier"
    assert ratios[-1] == 0.0


def test_moving_a_system_down_the_queue_frees_power(world):
    s = world.systems
    s.tick(world, 1e-6)
    assert s.get("fabricator").power_ratio < 0.5, "the lab is hogging the bus at the start"
    s.get("lab").priority = 9
    s.tick(world, 1e-6)
    assert s.get("fabricator").power_ratio == 1.0
    assert s.get("lab").power_ratio < 1.0


def test_switching_off_reduces_demand(world):
    s = world.systems
    s.tick(world, 1e-6)
    before = s.power_demand
    s.get("lab").enabled = False
    s.tick(world, 1e-6)
    assert s.power_demand == pytest.approx(before - s.get("lab").power)
    assert s.get("lab").efficiency == 0.0


def test_fabricator_turns_scrap_into_parts(world):
    s = world.systems
    fab = s.get("fabricator")
    feed = world.inventory_of(world.find_object("fab_feed"))
    out = world.inventory_of(world.find_object("parts_locker"))
    feed.items.clear()
    feed.add("scrap_metal", 4)
    out.items.clear()
    s.get("lab").enabled = False
    for _ in range(100):                       # 10 hours with an operator at the console
        fab.add_labour(0.1, "pip")
        s.tick(world, 0.1)
    assert out.count("spare_parts") == 2 and feed.count("scrap_metal") == 0
    assert "needs" in fab.blocked.lower()


def test_production_respects_max_stock(world):
    s = world.systems
    dispenser = world.inventory_of(world.find_object("galley_dispenser"))
    dispenser.items["ration_pack"] = 14
    hydro = s.get("hydroponics")
    for _ in range(50):
        hydro.add_labour(0.2)
        s.tick(world, 0.2)
    assert dispenser.count("ration_pack") == 14 and hydro.blocked == "output full"


def test_reactor_burns_fuel_and_stops_without_it(world):
    s = world.systems
    reactor = s.get("reactor")
    hopper = world.inventory_of(world.find_object("reactor_hopper"))
    reactor.fuel_left = 0.5
    hopper.items.clear()
    hopper.add("fuel_cell", 1)
    s.tick(world, 1.0)
    assert hopper.count("fuel_cell") == 0 and reactor.fuel_left > 20 and reactor.output > 0
    reactor.fuel_left = 0.1
    s.tick(world, 1.0)
    assert reactor.output == 0 and reactor.blocked == "no fuel"
    assert s.power_supply == pytest.approx(s.get("rtg").output)


def test_condition_cap_limits_repairs(world):
    pump = world.systems.get("water_reclaim")
    assert pump.cap < 100
    pump.repair(80)
    assert pump.condition == pytest.approx(pump.cap)
    pump.cap = 100
    pump.repair(80)
    assert pump.condition == 100


def test_systems_wear_out_and_announce(world):
    world.characters.clear()
    world.add_character(world.player)          # nobody is maintaining anything
    world.toasts.clear()
    run_hours(world, 48)
    assert world.systems.get("reactor").condition < 40
    assert any("Fusion Reactor" in e["text"] for e in world.log if e["kind"] == "announce")


def test_air_falls_when_life_support_is_off(world):
    s = world.systems
    s.get("life_support").enabled = False
    s.get("hydroponics").enabled = False
    air = s.resource("air")
    s.tick(world, 5.0)
    assert s.resource("air") < air - 10


def test_bad_air_hurts_everyone(world):
    world.systems.resources["air"] = 40.0
    world.systems.get("life_support").enabled = False
    world.systems.get("hydroponics").enabled = False
    skill_before = world.characters["bram"].skill("engineering")
    run_hours(world, 1)
    bram = world.characters["bram"]
    assert "hypoxia" in bram.auto_effects and "hypoxia" in world.player.auto_effects
    assert bram.skill("engineering") < skill_before


def test_network_roundtrip(world):
    run_hours(world, 3)
    snap = world.systems.to_dict()
    fresh = SystemNetwork(world.content.systems)
    fresh.load_dict(snap)
    assert fresh.to_dict() == snap
