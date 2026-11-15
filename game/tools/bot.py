"""Drive the real game with synthetic input (tests, demos, screenshots).

    from game.tools.bot import Bot
    bot = Bot.new_game()            # headless app, fresh world, PlayScene on top
    bot.walk_to(14, 2)              # path-find and walk like a player would
    bot.interact()                  # press the confirm key
    bot.choose("Where do I find")   # pick a dialogue choice by its text
    bot.shot("/tmp/after.png")

The bot presses the same named actions a keyboard would, so it exercises the
scenes, not just the simulation underneath.
"""
from __future__ import annotations

from typing import Optional, Tuple

from game import config
from game.world import movement
from game.world.pathfinding import find_path

DT = 1.0 / 30.0


class Bot:
    def __init__(self, app):
        self.app = app

    # -- construction --------------------------------------------------------------
    @classmethod
    def new_game(cls, setup: Optional[dict] = None, seed: int = 1, content=None, intro: bool = False) -> "Bot":
        from game.content import Content
        from game.engine.app import App
        from game.scenes.play import PlayScene
        from game.world.world import World
        content = content or Content.load()
        app = App(headless=True, audio=False, content=content, persist_options=False)
        world = World.new_game(content, setup, seed=seed)
        if intro and content.game.get("intro") in content.dialogues:
            world.request("dialogue", dialogue=content.game["intro"])
        app.push_scene(PlayScene(app, world))
        return cls(app)

    # -- access ----------------------------------------------------------------------
    @property
    def scene(self):
        return self.app.scene

    @property
    def play(self):
        for scene in self.app.scenes:
            if scene.name == "play":
                return scene
        return None

    @property
    def world(self):
        play = self.play
        return play.world if play is not None else None

    @property
    def player(self):
        return self.world.player

    # -- input -----------------------------------------------------------------------
    def frames(self, n: int = 1, dt: float = DT) -> None:
        for _ in range(n):
            self.app.input.begin_frame()
            self.app.frame_step(dt, None)

    def tap(self, action: str, frames_after: int = 2) -> None:
        self.app.input.begin_frame()
        self.app.input.inject(action)
        self.app.frame_step(DT, None)
        self.frames(frames_after)

    def wait(self, seconds: float) -> None:
        self.frames(max(1, int(seconds / DT)))

    def type(self, text: str) -> None:
        self.app.input.begin_frame()
        self.app.input.type_text(text)
        self.app.frame_step(DT, None)

    def key(self, key: int) -> None:
        self.app.input.begin_frame()
        self.app.input.keys_pressed.add(key)
        self.app.frame_step(DT, None)

    # -- overworld ---------------------------------------------------------------------
    def _settle(self, limit: int = 600) -> None:
        """Let fades and portal transitions finish."""
        for _ in range(limit):
            if not self.app.fading and not (self.world and any(
                    r.get("type") == "portal" for r in self.world.requests)):
                break
            self.frames(1, 0.05)

    def step(self, direction: str) -> bool:
        """Walk one tile.  Returns False if the way was blocked."""
        p = self.player
        before = (p.map_id, p.x, p.y)
        inp = self.app.input
        for _ in range(40):
            inp.begin_frame()
            inp.hold(direction, True)
            self.app.frame_step(DT, None)
            if (p.map_id, p.x, p.y) != before:
                break
        inp.hold(direction, False)
        for _ in range(40):
            if not p.moving:
                break
            self.frames(1)
        self._settle()
        return (p.map_id, p.x, p.y) != before

    def face(self, direction: str) -> None:
        p = self.player
        if p.facing != direction:
            p.facing = direction
        self.frames(1)

    def walk_to(self, x: int, y: int, max_steps: int = 400) -> bool:
        """Walk to a cell on the current map along the shortest path."""
        world = self.world
        for _ in range(max_steps):
            p = self.player
            if (p.x, p.y) == (x, y):
                return True
            if self.scene is not self.play:
                return False

            def passable(cx: int, cy: int) -> bool:
                return movement.blocked(world, p, cx, cy, ignore_chars=True) is None
            path = find_path((p.x, p.y), [(x, y)], passable)
            if not path:
                return False
            nx, ny = path[0]
            direction = config.dir_from_delta(nx - p.x, ny - p.y)
            map_before = p.map_id
            if not self.step(direction):
                self.wait(0.5)          # somebody in the way: wait, they move or we swap
            if p.map_id != map_before:
                return (p.x, p.y) == (x, y)
        return False

    def walk_next_to(self, x: int, y: int) -> bool:
        """Stand beside a (solid) cell, facing it."""
        world = self.world
        p = self.player
        for dx, dy in ((0, 1), (-1, 0), (1, 0), (0, -1)):
            if movement.blocked(world, p, x + dx, y + dy, ignore_chars=True) is None:
                if self.walk_to(x + dx, y + dy):
                    self.face(config.dir_from_delta(x - p.x, y - p.y))
                    return True
        return False

    def go_to_object(self, obj_id: str) -> bool:
        obj = self.world.find_object(obj_id)
        if obj is None or obj.map_id != self.player.map_id:
            return False
        return self.walk_next_to(obj.x, obj.y)

    def go_to_char(self, char_id: str, tries: int = 6) -> bool:
        ch = self.world.characters[char_id]
        for _ in range(tries):
            if ch.map_id != self.player.map_id:
                return False
            if self.walk_next_to(ch.x, ch.y) and abs(ch.x - self.player.x) + abs(ch.y - self.player.y) == 1:
                self.face(config.dir_from_delta(ch.x - self.player.x, ch.y - self.player.y))
                return True
        return False

    def use_portal(self, portal_id: str) -> bool:
        obj = self.world.find_object(portal_id)
        if obj is None or obj.map_id != self.player.map_id:
            return False
        target = obj.get("target_map")
        self.walk_to(obj.x, obj.y)
        self._settle()
        return self.player.map_id == target

    def interact(self) -> None:
        self.tap("confirm", 3)

    # -- dialogue ------------------------------------------------------------------------
    @property
    def in_dialogue(self) -> bool:
        return self.scene is not None and self.scene.name == "dialogue"

    def dialogue_choices(self):
        if not self.in_dialogue:
            return []
        self.skip_text()
        if not self.in_dialogue:
            return []
        return self.scene._choices()

    def skip_text(self, limit: int = 60) -> None:
        """Advance pages until a choice is offered or the dialogue ends."""
        for _ in range(limit):
            if not self.in_dialogue:
                return
            scene = self.scene
            if scene.writer is not None:
                scene.writer.skip()
            if scene._choices():
                return
            self.tap("confirm", 1)

    def choose(self, text: str) -> bool:
        """Pick the first enabled choice whose text contains ``text``."""
        choices = self.dialogue_choices()
        for i, choice in enumerate(choices):
            if text.lower() in choice.text.lower() and choice.enabled:
                self.scene.index = i
                self.tap("confirm", 1)
                return True
        return False

    def end_dialogue(self, limit: int = 40) -> None:
        """Click through, always taking the last choice (usually 'goodbye')."""
        for _ in range(limit):
            if not self.in_dialogue:
                return
            choices = self.dialogue_choices()
            if not self.in_dialogue:
                return
            if choices:
                enabled = [i for i, c in enumerate(choices) if c.enabled]
                self.scene.index = enabled[-1] if enabled else 0
                self.tap("confirm", 1)

    def close_overlays(self, limit: int = 12) -> None:
        for _ in range(limit):
            if self.scene is self.play or self.scene is None:
                return
            if self.in_dialogue:
                self.end_dialogue()
            else:
                self.tap("cancel", 2)

    # -- misc ----------------------------------------------------------------------------
    def shot(self, path: str, scale: int = 3) -> str:
        self.app.draw()
        return self.app.screenshot(path, scale)
