import json
import os

from game import save
from game.world.world import World
from tests.helpers import place_player, run_hours, talk
from tests.test_ai import signature


def test_save_file_roundtrip(world):
    run_hours(world, 9)
    talk(world, "bram", ["Who is giving you forms"])
    world.player.inventory.add("stim", 2)
    world.player.equip("multitool")
    world.script.run(["spawn_item('hub', 20, 14, 'medkit')", "set_tile('hub', 'objects', 21, 14, 'crate')",
                      "unlock('cell_door')", "set_flag('custom', 12)"])
    world.inventory_of(world.find_object("player_locker")).add("fuel_cell", 1)
    place_player(world, "hub", 18, 15)
    path = save.save_game(world, "slot1")
    assert os.path.exists(path)

    header = save.read_header("slot1")
    assert header["name"] == "Tester" and header["stamp"] == world.clock.stamp() and header["map"] == "Central Ring"
    assert save.latest_slot() == "slot1"
    assert [e["slot"] for e in save.list_saves() if e["header"]] == ["slot1"]

    loaded = save.load_game(world.content, "slot1")
    assert loaded.clock.minutes == world.clock.minutes
    assert loaded.flags == world.flags and loaded.flags["custom"] == 12
    assert loaded.quests.to_dict() == world.quests.to_dict()
    assert loaded.systems.to_dict() == world.systems.to_dict()
    assert loaded.player.to_dict() == world.player.to_dict()
    for cid, ch in world.characters.items():
        assert loaded.characters[cid].to_dict() == ch.to_dict(), cid
    assert loaded.inventory_of(loaded.find_object("player_locker")).count("fuel_cell") == 1
    assert not loaded.find_object("cell_door").get("locked")
    assert loaded.maps["hub"].tile("objects", 21, 14) == "crate" and loaded.maps["hub"].solid_at(21, 14)
    assert any(o.type == "item" and o.get("item") == "medkit" for o in loaded.maps["hub"].objects_at(20, 14))
    assert loaded.director.to_dict() == world.director.to_dict()
    assert json.dumps(loaded.to_dict(), sort_keys=True) == json.dumps(world.to_dict(), sort_keys=True)


def test_loaded_game_continues_exactly_like_the_original(content):
    original = World.new_game(content, {"background": "analyst"}, seed=11)
    original.requests.clear()
    run_hours(original, 20)                   # mid-night: goals in progress, people asleep, Dex at work
    save.save_game(original, "autosave")
    resumed = save.load_game(content, "autosave")
    run_hours(original, 30)
    run_hours(resumed, 30)
    assert signature(resumed) == signature(original)
    assert resumed.log[-5:] == original.log[-5:]


def test_removed_pickups_and_departed_crew_stay_gone(world):
    shard = world.find_object("chamber_shard")
    world.remove_object(shard)
    world.script.run(["remove_npc('dex')", "remove_obj('magpie')"])
    save.save_game(world, "slot2")
    loaded = save.load_game(world.content, "slot2")
    assert loaded.find_object("chamber_shard") is None and loaded.find_object("magpie") is None
    assert not loaded.characters["dex"].present
    assert "dex" not in [c.id for c in loaded.npcs()]


def test_characters_added_after_a_save_join_the_world(world, tmp_path):
    save.save_game(world, "slot3")
    from game.content import Content
    from game.tools import chargen
    import shutil
    data = tmp_path / "data"
    shutil.copytree(world.content.dir, data)
    richer = Content.load(str(data))
    newcomer = chargen.new_character(richer, "Kira Vance", "engineer")
    chargen.save_character(richer, newcomer)
    loaded = save.load_game(richer, "slot3")
    kira = loaded.characters["kira"]
    assert kira.present and kira.map_id == "engineering" and kira.dialogue == "generic"
    run_hours(loaded, 3)
    assert kira.activity


def test_newer_save_versions_are_refused(world):
    path = save.save_game(world, "slot4")
    with open(path) as fh:
        data = json.load(fh)
    data["world"]["version"] = 999
    with open(path, "w") as fh:
        json.dump(data, fh)
    try:
        save.load_game(world.content, "slot4")
    except ValueError as exc:
        assert "newer" in str(exc)
    else:
        raise AssertionError("expected a refusal")


def test_empty_slots_and_delete(world):
    assert save.read_header("slot5") is None and save.latest_slot() is None
    save.save_game(world, "quicksave")
    assert save.latest_slot() == "quicksave"
    save.delete_save("quicksave")
    assert save.latest_slot() is None
    assert save.format_playtime(3725) == "1:02"
