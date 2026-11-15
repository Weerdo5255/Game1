import pygame
import pytest

from game.art import chars, icons, tiles
from game.engine import palette
from game.engine.font import get_font, strip_markup, visible_prefix


@pytest.fixture(scope="module", autouse=True)
def _display():
    pygame.init()
    pygame.display.set_mode((16, 16))


def test_every_tile_renders():
    art = tiles.TileArt()
    assert len(tiles.TILES) > 80
    for name, tdef in tiles.TILES.items():
        for frame in range(tdef.frames):
            for mask in ((0, tiles.SO, tiles.E | tiles.W, 255) if tdef.group else (0,)):
                surf = art.get(name, frame, 0, mask)
                assert surf.get_size() == (16, 16), name
        assert tdef.layer in ("ground", "decor", "objects", "overhead"), name


def test_props_cover_their_footprint():
    for name, pdef in tiles.PROPS.items():
        cells = {(dx, dy) for dx, dy, _layer, _tile in pdef.parts}
        assert cells == {(x, y) for x in range(pdef.w) for y in range(pdef.h)}, name
        for _dx, _dy, _layer, tile in pdef.parts:
            assert tiles.TILES[tile].prop == name


def test_floor_is_walkable_and_walls_are_not():
    assert not tiles.is_solid("floor_plate") and tiles.is_solid("wall") and tiles.is_solid("crate")
    assert tiles.group_of("wall") == tiles.group_of("window") == "wall"


def test_character_templates_are_well_formed():
    assert chars.validate_templates() == []


def test_every_appearance_option_builds_sprites():
    art = chars.CharArt()
    for hair in chars.HAIR_STYLES:
        for outfit in chars.OUTFIT_STYLES:
            look = {"hair": hair, "outfit": outfit, "extras": ["glasses", "beard"]}
            frames = art.frames(look)
            assert len(frames) == 16
            assert all(s.get_size() == (16, 24) for s in frames.values())
    for skin in palette.SKIN_TONES:
        assert art.portrait({"skin": skin}).get_size() == (32, 32)
    drone = {"body": "drone"}
    assert art.sprite(drone, "left", 1).get_size() == (16, 24)
    assert art.portrait(drone).get_size() == (32, 32)
    assert art.sleeping({"hair": "long"}).get_width() == 16


def test_normalize_repairs_bad_appearance():
    look = chars.normalize({"hair": "mullet", "skin": "plaid", "extras": ["monocle", "beard"]})
    assert look["hair"] in chars.HAIR and look["skin"] in palette.SKIN_TONES and look["extras"] == ["beard"]


def test_icons_render():
    art = icons.IconArt()
    for name in icons.ICONS:
        assert art.get(name, "amber").get_size() == (16, 16)
    assert art.get("no_such_icon") is not None


def test_font_measures_and_wraps():
    font = get_font(1)
    text = "The quick brown fox jumps over the lazy dog 0123456789 !?.,:;'\"()[]{}<<>+-*/=%&@#$^_`~|\\"
    assert font.render(text).get_width() >= font.width(text)
    lines = font.wrap("one two three four five six seven eight nine ten", 60)
    assert len(lines) > 2 and all(font.width(line) <= 60 for line in lines)
    marked = "plain <y>gold words here</y> plain"
    wrapped = font.wrap(marked, 50)
    assert strip_markup(" ".join(wrapped)) == strip_markup(marked)
    assert all(line.count("<y>") == line.count("</y>") for line in wrapped)
    assert strip_markup(visible_prefix("ab<y>cdef</y>", 4)) == "abcd"
