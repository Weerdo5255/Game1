"""Seeded random-input soak tests.

Nothing a player can press, and nothing an editor user can click, should ever
raise - whatever state the game or the tool happens to be in.  These runs are
short so the suite stays quick; raise ``SOAK_FRAMES`` / ``SOAK_STEPS`` in the
environment for a longer hunt::

    SOAK_FRAMES=20000 SOAK_STEPS=5000 python -m pytest tests/test_soak.py
"""
import os
import random

import pygame
import pytest

from game.tools.bot import DT, Bot

FRAMES = int(os.environ.get("SOAK_FRAMES", "1500"))
STEPS = int(os.environ.get("SOAK_STEPS", "250"))

ACTIONS = ["up", "down", "left", "right", "confirm", "cancel", "menu", "tab_prev", "tab_next",
           "inventory", "journal", "crew", "station", "wait", "quicksave", "quickload", "debug"]
WEIGHTS = [6, 6, 6, 6, 12, 3, 1, 2, 2, 1, 1, 1, 1, 1, 0.3, 0.3, 0.2]
RAW_KEYS = [pygame.K_BACKSPACE, pygame.K_RETURN, pygame.K_DELETE, pygame.K_F6, pygame.K_F7]
FACING = {(0, 1): "up", (0, -1): "down", (1, 0): "left", (-1, 0): "right"}


def _fresh_game(app, content, rng, seed):
    """A new game some hours in, with the player standing next to something."""
    from game.scenes.play import PlayScene
    from game.world.world import World
    backgrounds = [b["id"] for b in content.game["backgrounds"]]
    world = World.new_game(content, {"name": "Monkey", "background": rng.choice(backgrounds)},
                           seed=seed)
    world.flags["audit_authority"] = True
    world.fast_forward(rng.randint(0, 60) * 60)
    targets = [(g.id, o.x, o.y) for g in world.maps.values() for o in g.objects
               if o.type in ("console", "container", "terminal", "bed", "interact", "door", "sign")]
    targets += [(n.map_id, n.x, n.y) for n in world.npcs()]
    rng.shuffle(targets)
    for map_id, x, y in targets:
        free = [(dx, dy) for dx, dy in FACING
                if not world.maps[map_id].solid_at(x + dx, y + dy) and world.char_at(map_id, x + dx, y + dy) is None]
        if free:
            dx, dy = rng.choice(free)
            world.transfer(world.player, map_id, x + dx, y + dy)
            world.player.facing = FACING[(dx, dy)]
            break
    app.set_scene(PlayScene(app, world))
    return world


@pytest.mark.parametrize("seed", [11, 12])
def test_random_input_never_breaks_the_game(content, seed):
    rng = random.Random(seed)
    bot = Bot.new_game({"name": "Monkey"}, seed=seed, content=content)
    app = bot.app
    _fresh_game(app, content, rng, seed)
    endings = sorted(content.endings)
    seen, held = set(), None
    for frame in range(FRAMES):
        if not app.running or not app.scenes or app.scene.name in ("title", "newgame") or frame % 500 == 499:
            app.running = True
            _fresh_game(app, content, rng, seed * 1000 + frame)
            held = None
        seen.add(app.scene.name)
        app.input.begin_frame()
        r = rng.random()
        if r < 0.30:
            app.input.inject(rng.choices(ACTIONS, WEIGHTS)[0])
        elif r < 0.45:
            if held and rng.random() < 0.3:
                app.input.hold(held, False)
                held = None
            elif not held:
                held = rng.choice(["up", "down", "left", "right"])
                app.input.hold(held, True)
        elif r < 0.47:
            app.input.type_text(rng.choice("abcXYZ 19!?"))
        elif r < 0.485:
            app.input.keys_pressed.add(rng.choice(RAW_KEYS))
        elif r < 0.487 and bot.world is not None:
            bot.world.end_game(rng.choice(endings))
        elif r < 0.489 and bot.world is not None and app.scene.name == "play":
            bot.world.fast_forward(rng.randint(10, 600))
        app.frame_step(rng.choice([DT, DT, 1 / 60.0, 0.1, 0.25]), None)
        if bot.world is not None:
            assert not bot.world.script.errors, bot.world.script.errors[:3]
    assert {"play", "dialogue", "datapad"} <= seen, "the monkey got around: %s" % sorted(seen)


TOOL_KEYS = [pygame.K_z, pygame.K_y, pygame.K_s, pygame.K_n, pygame.K_o, pygame.K_g, pygame.K_b, pygame.K_f,
             pygame.K_r, pygame.K_i, pygame.K_e, pygame.K_1, pygame.K_2, pygame.K_3, pygame.K_TAB, pygame.K_DELETE,
             pygame.K_BACKSPACE, pygame.K_RETURN, pygame.K_ESCAPE, pygame.K_UP, pygame.K_DOWN, pygame.K_LEFT,
             pygame.K_RIGHT, pygame.K_F5, pygame.K_SPACE, pygame.K_PAGEUP, pygame.K_PAGEDOWN, pygame.K_HOME,
             pygame.K_END, pygame.K_x, pygame.K_c, pygame.K_v, pygame.K_a, pygame.K_d, pygame.K_w, pygame.K_q]
TOOL_MODS = [0, 0, 0, pygame.KMOD_CTRL, pygame.KMOD_SHIFT, pygame.KMOD_CTRL | pygame.KMOD_SHIFT]
TOOL_TEXT = ["a", "7", "x", "hub", "-1", "{", "}", '"', "true", " ", "99999", "0", "[1,2]", "floor_plate"]


def _tool_monkey(app, rng, steps):
    w, h = app.screen.get_size()
    for _ in range(steps):
        r = rng.random()
        pos = (rng.randrange(w), rng.randrange(h))
        if r < 0.45:
            app.click(pos, rng.choice([1, 1, 1, 3]))
        elif r < 0.60:
            end = (min(w - 1, max(0, pos[0] + rng.randint(-200, 200))),
                   min(h - 1, max(0, pos[1] + rng.randint(-200, 200))))
            app.drag(pos, end, rng.choice([1, 1, 3]))
        elif r < 0.80:
            key, mod = rng.choice(TOOL_KEYS), rng.choice(TOOL_MODS)
            name = pygame.key.name(key)
            app.press(key, mod, name if mod == 0 and len(name) == 1 else "")
        elif r < 0.90:
            app.type_text(rng.choice(TOOL_TEXT))
        elif r < 0.96:
            app.mouse = pos
            app.frame(0.016, [pygame.event.Event(pygame.MOUSEWHEEL, x=0, y=rng.choice([-3, -1, 1, 3]))])
        else:
            app.frame(0.3, [])
        if not app.running:
            break


@pytest.mark.parametrize("tool", ["map_editor", "character_creator"])
def test_random_clicks_never_break_the_editors(data_copy, monkeypatch, tool):
    """The editors may wreck the (copied) content - a monkey deletes things -
    but they must not raise, and what they save must still load."""
    import subprocess

    from game.content import Content
    from game.tools.validate import validate
    monkeypatch.setattr(subprocess, "Popen", lambda *a, **k: None)      # "Play" must not start the game
    rng = random.Random(5)
    content = Content.load()
    if tool == "map_editor":
        from game.tools.map_editor import MapEditor
        app = MapEditor("hub", headless=True, content=content)
    else:
        from game.tools.char_editor import CharacterCreator
        app = CharacterCreator(headless=True, content=content)
    app.layout()
    _tool_monkey(app, rng, STEPS)
    after = Content.load()
    assert not after.problems, after.problems
    assert after.fresh_maps()
    validate(after)                                # may complain, must not raise
