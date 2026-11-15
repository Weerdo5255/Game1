"""The editors and command-line tools, driven headless on a copy of the data."""
import json
import os

import pygame
import pytest

from game.content import Content, write_json


@pytest.fixture
def editor(data_copy):
    from game.tools.map_editor import MapEditor
    ed = MapEditor("hub", headless=True, content=Content.load(data_copy))
    ed.frame(0.016)
    return ed


def test_shipped_content_validates(content):
    from game.tools.validate import validate
    report = validate(content)
    assert report.errors == [], "\n".join(report.errors)
    assert report.warnings == [], "\n".join(report.warnings)


def test_validator_catches_mistakes(data_copy):
    from game.tools.validate import validate
    c = Content.load(data_copy)
    assert validate(c).errors == []
    dlg = json.load(open(c.dialogue_path("bram")))
    dlg["nodes"]["hub"]["choices"][0]["goto"] = "no_such_node"
    dlg["nodes"]["hub"]["choices"][1]["if"] = "has_item('unobtainium') and nonsense(3)"
    dlg["nodes"]["first"]["do"].append("set_stage('q_reactor', 'not_a_stage')")
    dlg["nodes"]["first"]["text"][0] = "Broken {cond('warp_core')} text"
    write_json(c.dialogue_path("bram"), dlg)
    hub = c.load_map("hub")
    hub.get_object("hub_to_command").props["target_y"] = 0          # inside a wall
    hub.set_tile("objects", 3, 3, "tile_that_does_not_exist")
    hub.save(c.map_path("hub"))
    pip = dict(c.characters["pip"])
    pip["spawn"] = {"map": "atlantis", "x": 1, "y": 1}
    pip["inventory"] = {"left_handed_spanner": 1}
    pip["goals"] = [{"id": "x", "steps": [{"do": "teleport_home"}]}]
    write_json(c.character_path("pip"), pip)
    open(os.path.join(data_copy, "quests.json"), "a").write("oops")
    report = validate(Content.load(data_copy))
    text = "\n".join(report.errors)
    for needle in ("missing node 'no_such_node'", "unknown item 'unobtainium'", "unknown function nonsense()",
                   "unknown stage 'not_a_stage'", "unknown system 'warp_core'", "is not walkable",
                   "unknown tile 'tile_that_does_not_exist'", "spawn map 'atlantis'",
                   "unknown item 'left_handed_spanner'", "unknown step verb 'teleport_home'", "quests.json"):
        assert needle in text, needle


def test_flags_that_are_never_set_are_reported(data_copy):
    from game.tools.validate import validate
    c = Content.load(data_copy)
    dlg = json.load(open(c.dialogue_path("wren")))
    dlg["nodes"]["hub"]["choices"][0]["if"] = "flag('typo_in_flag_name')"
    write_json(c.dialogue_path("wren"), dlg)
    report = validate(Content.load(data_copy))
    assert any("typo_in_flag_name" in w for w in report.warnings)


def test_seed_maps_are_reproducible_and_valid():
    from game.tools.seed_maps import build_all
    a, b = build_all(), build_all()
    assert set(a) == {"hub", "hangar", "habitat", "command", "engineering", "hydroponics", "medlab", "excavation"}
    for map_id in a:
        assert a[map_id].dumps() == b[map_id].dumps()


def test_shipped_maps_match_the_seed_script(content):
    """If this fails, run ``python -m game.tools.seed_maps --force`` (or, if the
    maps were edited by hand on purpose, delete this test)."""
    from game.tools.seed_maps import build_all
    for map_id, built in build_all().items():
        assert content.load_map(map_id).to_dict() == built.to_dict(), map_id


def test_simulate_and_validate_commands(capsys):
    from game.tools import simulate, validate
    assert simulate.main(["--days", "0.5", "--every", "6", "--crew"]) == 0
    out = capsys.readouterr().out
    assert "Day 1 19:30" in out and "Bram Kowal" in out
    assert validate.main([]) == 0
    assert "0 error(s)" in capsys.readouterr().out
    assert validate.main(["--api"]) == 0
    api = capsys.readouterr().out
    assert "set_flag(" in api and "CONDITIONS" in api and "add_goal(" in api


def test_art_and_map_dumps(tmp_path, content):
    from game.tools import artdump, mapshot
    assert artdump.main([str(tmp_path / "art")]) == 0
    for name in ("tiles.png", "props.png", "characters.png", "icons.png"):
        assert (tmp_path / "art" / name).stat().st_size > 1000
    assert mapshot.main([str(tmp_path / "maps"), "hub", "--regions", "--scale", "1"]) == 0
    img = pygame.image.load(str(tmp_path / "maps" / "hub.png"))
    assert img.get_size() == (36 * 16, 24 * 16)


# --- map editor ---------------------------------------------------------------
def test_editor_paint_erase_undo_redo(editor):
    ed = editor
    ed.brush = ("tile", "floor_grate")
    cell = (20, 12)
    ed.click(ed.cell_rect(*cell).center)
    assert ed.gmap.tile("ground", *cell) == "floor_grate" and ed.dirty
    ed.press(pygame.K_z, pygame.KMOD_CTRL)
    assert ed.gmap.tile("ground", *cell) == "floor_plate"
    ed.press(pygame.K_y, pygame.KMOD_CTRL)
    assert ed.gmap.tile("ground", *cell) == "floor_grate"
    ed.brush = ("tile", "crate")
    ed.drag(ed.cell_rect(24, 14).center, ed.cell_rect(27, 14).center)
    assert [ed.gmap.tile("objects", x, 14) for x in range(24, 28)] == ["crate"] * 4
    assert ed.gmap.solid_at(25, 14)
    ed.click(ed.cell_rect(25, 14).center, button=3)
    assert ed.gmap.tile("objects", 25, 14) == "" and ed.gmap.tile("ground", 25, 14) == "floor_plate"


def test_editor_palette_fill_rect_and_pick(editor):
    ed = editor
    ed.click((ed.pal_rect.x + 10, ed.pal_rect.y + 10))
    assert ed.brush[0] == "tile" and ed.tool == "paint"
    ed.press(pygame.K_f)
    ed.brush = ("tile", "floor_lab")
    ed.click(ed.cell_rect(13, 6).center)                      # the mess hall floor
    assert ed.gmap.tile("ground", 12, 5) == "floor_lab" and ed.gmap.tile("ground", 20, 10) == "floor_lab"
    assert ed.gmap.tile("ground", 5, 5) == "floor_plate", "the fill stops at the room's edge"
    ed.press(pygame.K_r)
    ed.brush = ("tile", "rug")
    ed.drag(ed.cell_rect(26, 18).center, ed.cell_rect(28, 20).center)
    assert all(ed.gmap.tile("decor", x, y) == "rug" for x in range(26, 29) for y in range(18, 21))
    ed.press(pygame.K_i)
    ed.click(ed.cell_rect(27, 19).center)
    assert ed.brush == ("tile", "rug")


def test_editor_props_are_placed_and_removed_whole(editor):
    ed = editor
    ed.brush = ("prop", "reactor")
    ed.set_tool("paint")
    ed.click(ed.cell_rect(26, 20).center)
    assert ed.gmap.tile("objects", 26, 20) == "reactor.0.2" and ed.gmap.tile("overhead", 28, 18) == "reactor.2.0"
    ed.click(ed.cell_rect(27, 19).center, button=3)
    assert all(ed.gmap.tile(layer, x, y) == "" for layer in ("objects", "overhead")
               for x in range(26, 29) for y in range(18, 21))


def test_editor_objects_regions_and_save(editor, data_copy):
    ed = editor
    ed.press(pygame.K_o)
    dispenser = ed.gmap.get_object("galley_dispenser")
    ed.click(ed.cell_rect(dispenser.x, dispenser.y).center)
    assert ed.sel_obj is dispenser
    fields = dict(ed.prop_rows)
    ed.click(fields["items"].rect.center)
    for _ in range(60):
        ed.press(pygame.K_BACKSPACE)
    ed.type_text('{"coffee_bulb": 4}')
    ed.press(pygame.K_RETURN)
    assert dispenser.props["items"] == {"coffee_bulb": 4}
    ed._side_pick("sign")
    ed.click(ed.cell_rect(30, 10).center)
    sign = ed.sel_obj
    assert sign.type == "sign" and sign.id == "hub_sign_1"
    ed.drag(ed.cell_rect(30, 10).center, ed.cell_rect(31, 11).center)
    assert sign.pos == (31, 11)
    ed.press(pygame.K_DELETE)
    assert ed.gmap.get_object("hub_sign_1") is None
    ed.press(pygame.K_m)
    ed.drag(ed.cell_rect(26, 17).center, ed.cell_rect(33, 21).center)
    for _ in range(12):
        ed.press(pygame.K_BACKSPACE)
    ed.type_text("kiosk")
    ed.press(pygame.K_RETURN)
    assert ed.sel_region.id == "kiosk" and (ed.sel_region.w, ed.sel_region.h) == (8, 5)
    ed.press(pygame.K_n)
    ed._side_pick("pip")
    ed.click(ed.cell_rect(28, 12).center)
    assert ed.cmd_save() and not ed.dirty and "No problems" in ed.status
    fresh = Content.load(data_copy)
    hub = fresh.load_map("hub")
    assert hub.get_object("galley_dispenser").props["items"] == {"coffee_bulb": 4}
    assert hub.find_region("kiosk") is not None
    assert fresh.characters["pip"]["spawn"]["map"] == "hub"


def test_editor_new_map_resize_and_portal_target(editor, data_copy):
    ed = editor
    ed.cmd_new()
    for value in ("annex", "Annex", "12", "9"):
        for _ in range(12):
            ed.press(pygame.K_BACKSPACE)
        ed.type_text(value)
        ed.press(pygame.K_TAB)
    ed.press(pygame.K_RETURN)
    assert ed.gmap.id == "annex" and (ed.gmap.width, ed.gmap.height) == (12, 9)
    ed.press(pygame.K_o)
    ed._side_pick("portal")
    ed.click(ed.cell_rect(0, 4).center)
    portal = ed.sel_obj
    ed.cmd_pick_target()
    picker = ed.modals[-1].list
    index = [d for _l, d in picker.items].index("hub")
    ed.click((picker.rect.x + 12, picker.rect.y + 4 + index * picker.row_h + 6))
    assert ed.gmap.id == "hub" and ed.pick_cb is not None
    ed.click(ed.cell_rect(20, 14).center)
    assert ed.gmap.id == "annex" and portal.props["target_map"] == "hub"
    assert (portal.props["target_x"], portal.props["target_y"]) == (20, 14)
    ed.cmd_resize()
    for value in ("16", "10", "2", "1"):
        for _ in range(6):
            ed.press(pygame.K_BACKSPACE)
        ed.type_text(value)
        ed.press(pygame.K_TAB)
    ed.press(pygame.K_RETURN)
    assert (ed.gmap.width, ed.gmap.height) == (16, 10) and portal.pos == (2, 5)
    assert ed.cmd_save()
    assert "annex" in Content.load(data_copy).map_ids
    ed.press(pygame.K_F5)
    assert "--map annex" in ed.status


# --- character creator ----------------------------------------------------------
def test_character_cli(data_copy, capsys):
    from game.tools import char_editor
    assert char_editor.main(["--new", "Kira Vance", "--role", "engineer", "--seed", "4", "--dialogue"]) == 0
    assert char_editor.main(["--new", "Bolt", "--role", "drone", "--map", "hub", "--pos", "20,14"]) == 0
    assert char_editor.main(["--new", "Nobody", "--role", "wizard"]) == 1
    out = capsys.readouterr().out
    assert "kira.json" in out and "bed_spare_1" in out and "unknown role" in out
    c = Content.load(data_copy)
    kira, bolt = c.characters["kira"], c.characters["bolt"]
    assert kira["job"]["system"] == "reactor" and kira["spawn"]["map"] == "engineering"
    assert kira["dialogue"] == "kira" and "kira" in c.dialogues
    assert bolt["spawn"] == {"map": "hub", "x": 20, "y": 14, "facing": "down"} and bolt["home"] == {}
    from game.tools.validate import validate
    assert validate(c).errors == []
    assert char_editor.main(["--list"]) == 0 and "Kira Vance" in capsys.readouterr().out
    assert char_editor.main(["--roles"]) == 0 and "botanist" in capsys.readouterr().out
    assert char_editor.main(["--delete", "bolt"]) == 0
    assert "bolt" not in Content.load(data_copy).characters


def test_new_characters_get_distinct_places(data_copy):
    from game.tools import chargen
    c = Content.load(data_copy)
    spots, beds = set(), set()
    for i in range(2):
        data = chargen.new_character(c, "Engineer %d" % i, "engineer")
        chargen.save_character(c, data)
        spots.add((data["spawn"]["x"], data["spawn"]["y"]))
        beds.add(data["home"].get("bed"))
    assert len(spots) == 2 and beds == {"bed_spare_1", "bed_spare_2"}
    assert chargen.new_character(c, "Third", "engineer")["home"] == {}, "no bunks left: they will sleep where they can"
    assert chargen.slugify("Dr. Odile Marchetti") == "odile" and chargen.unique_id(c, "pip") == "pip2"


def test_character_creator_window(data_copy):
    from game.tools.char_editor import CharacterCreator
    cc = CharacterCreator("pip", headless=True, content=Content.load(data_copy))
    cc.frame(0.016)
    assert cc.data["id"] == "pip" and not cc.dirty
    cc.cmd_new()
    picker = cc.modals[-1].list
    index = [d for _l, d in picker.items].index("scientist")
    cc.click((picker.rect.x + 12, picker.rect.y + 4 + index * picker.row_h + 6))
    assert cc.data["role"] == "scientist" and cc.orig_id is None
    cc.click(cc.f_name.rect.center)
    for _ in range(24):
        cc.press(pygame.K_BACKSPACE)
    cc.type_text("Dr. Odile Marchetti")
    cc.press(pygame.K_RETURN)
    assert cc.data["id"] == "odile"
    hair = cc.data["appearance"]["hair"]
    cyc = cc.app_cyclers["hair"]
    cc.click((cyc.rect.right - 8, cyc.rect.centery))
    assert cc.data["appearance"]["hair"] != hair
    spin = cc.skill_spin["science"]
    before = cc.data["skills"]["science"]
    cc.click((spin.rect.right - 6, spin.rect.centery))
    assert cc.data["skills"]["science"] == before + 1
    _thumb, rect, k = cc._minimap_layout()
    cc.click((int(rect.x + 19.5 * 16 * k), int(rect.y + 5.5 * 16 * k)))
    assert (cc.data["spawn"]["x"], cc.data["spawn"]["y"]) == (19, 5)
    cc.click((int(rect.x + 0.5 * 16 * k), int(rect.y + 0.5 * 16 * k)))       # a wall: refused
    assert (cc.data["spawn"]["x"], cc.data["spawn"]["y"]) == (19, 5)
    cc.cmd_make_dialogue()
    assert cc.data["dialogue"] == "odile"
    cc.frame(0.016)
    assert cc.cmd_save() and cc.problems == [] and not cc.dirty
    c = Content.load(data_copy)
    assert c.characters["odile"]["name"] == "Dr. Odile Marchetti" and "odile" in c.dialogues
    # and she really turns up in a new game, working
    from game.world.world import World
    w = World.new_game(c, seed=2)
    w.requests.clear()
    w.fast_forward(60 * 6)
    odile = w.characters["odile"]
    assert odile.present and odile.activity
    from tests.helpers import talk
    w.script.strict = True
    assert talk(w, "odile")
