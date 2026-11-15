"""The overworld: walking around, talking, using things.

``PlayScene`` owns the camera and the HUD and translates input into actions
on the :class:`~game.world.world.World`.  All game rules live in the world;
this scene only decides *what the player is pointing at* and opens the right
window for it.
"""
from __future__ import annotations

import math
from typing import Any, List, Optional, Tuple

from .. import config, save
from ..engine import palette as P
from ..engine import ui
from ..engine.scene import Scene
from ..world import movement
from ..world.pathfinding import can_open
from .render import MapRenderer

T = config.TILE
TOAST_COLORS = {"info": P.UI["text"], "good": P.UI["good"], "bad": P.RED[3], "warn": P.AMBER[3],
                "quest": P.CYAN[3]}
DOOR_SPRITES = ("door", "door_secure", "door_v", "door_h")


class PlayScene(Scene):
    name = "play"

    def __init__(self, app, world):
        super().__init__(app)
        self.world = world
        self.renderer = MapRenderer(app.tiles, app.chars, app.icons)
        self.renderer.lighting = bool(app.options.get("lighting", True))
        self.toasts = ui.Toasts()
        self.cam = (0, 0)
        self.target: Optional[Tuple[str, Any, str]] = None
        self.debug = False
        self._turn = 0.0
        self._push = 0.0
        self._lock_msg = 0.0
        self._alert_cooldown = 0.0
        self._place_flash = 3.0
        self._last_place = ""
        self._fps = 60.0

    # ------------------------------------------------------------------
    def enter(self) -> None:
        self.app.audio.ambient("hum", 0.35)
        self._snap_camera()

    def exit(self) -> None:
        self.app.audio.ambient(None)

    def resume(self) -> None:
        self.input.reset()
        self.renderer.lighting = bool(self.app.options.get("lighting", True))

    # ------------------------------------------------------------------
    # update
    # ------------------------------------------------------------------
    def update(self, dt: float) -> None:
        world = self.world
        app = self.app
        inp = self.input
        self._fps += (1.0 / max(dt, 1e-4) - self._fps) * 0.05
        world.busy = False
        if self._handle_requests():
            world.busy = True
            return

        if inp.pressed("menu"):
            self._open_menu("status")
            return
        for action, tab in (("inventory", "items"), ("journal", "journal"), ("crew", "crew"),
                            ("station", "station")):
            if inp.pressed(action):
                self._open_menu(tab)
                return
        if inp.pressed("wait"):
            from .panels import WaitScene
            app.push_scene(WaitScene(app, world, self))
            return
        if inp.pressed("quicksave"):
            self.quicksave()
        if inp.pressed("quickload"):
            self.quickload()
            return
        if inp.pressed("debug"):
            self.debug = not self.debug
        if self.debug:
            import pygame
            if pygame.K_F6 in inp.keys_pressed:
                world.fast_forward(60)
                self.toasts.add("debug: +1 hour")
            if pygame.K_F7 in inp.keys_pressed:
                world.fast_forward(360)
                self.toasts.add("debug: +6 hours")

        self._update_player(dt)
        self.target = self._find_target()
        if inp.pressed("confirm") and not world.player.moving:
            if self.target is not None:
                self._interact(self.target)
                if app.scene is not self:
                    world.busy = True
                    return
        world.update(dt)
        self._update_doors(dt)
        self._collect_toasts()
        self.toasts.update(dt)
        self._place_flash = max(0.0, self._place_flash - dt)
        self._lock_msg = max(0.0, self._lock_msg - dt)
        self._alert_cooldown = max(0.0, self._alert_cooldown - dt)
        self._update_camera(dt)

    # -- requests from the simulation -------------------------------------------
    def _handle_requests(self) -> bool:
        """Process one queued UI request.  True if a scene/fade took over."""
        world, app = self.world, self.app
        while world.requests:
            req = world.requests.pop(0)
            kind = req.get("type")
            if kind == "sound":
                app.audio.play(str(req.get("name", "")))
                continue
            if kind == "fade":
                self._snap_camera()
                continue
            if kind == "portal":
                portal = world.find_object(str(req.get("obj", "")))
                if portal is not None:
                    app.audio.play("door")

                    def go(portal=portal):
                        movement.use_portal(world, world.player, portal)
                        self._snap_camera()
                        self._place_flash = 3.0
                        self.autosave()
                    app.fade(go, 0.2)
                    return True
                continue
            if kind == "dialogue":
                from .dialogue import DialogueScene
                dlg = str(req.get("dialogue") or "")
                npc_id = str(req.get("npc") or "")
                if not dlg and npc_id in world.characters:
                    dlg = world.characters[npc_id].dialogue
                if dlg in world.content.dialogues:
                    npc = world.characters.get(npc_id)
                    if npc is not None and npc.map_id == world.player.map_id:
                        world.player.face_towards(npc.x, npc.y)
                        npc.face_towards(world.player.x, world.player.y)
                    app.push_scene(DialogueScene(app, world, dlg, npc_id, str(req.get("obj") or ""),
                                                 req.get("node")))
                    return True
                continue
            if kind == "trade":
                from .panels import TradeScene
                npc = world.characters.get(str(req.get("npc", "")))
                if npc is not None:
                    app.push_scene(TradeScene(app, world, npc))
                    return True
                continue
            if kind == "container":
                obj = world.find_object(str(req.get("obj", "")))
                if obj is not None:
                    from .panels import ContainerScene
                    app.push_scene(ContainerScene(app, world, obj))
                    return True
                continue
            if kind == "collapse":
                self._player_collapse()
                return True
            if kind == "ending":
                from .ending import EndingScene
                ending = str(req.get("id", ""))
                app.fade(lambda: app.set_scene(EndingScene(app, world, ending)), 0.6)
                return True
        return False

    def _player_collapse(self) -> None:
        world = self.world
        player = world.player

        def wake() -> None:
            bed = world.find_object(str(world.content.game.get("medbay_bed", "")))
            if bed is not None:
                cells = world.nav.approach_cells(player, bed.map_id, bed.x, bed.y)
                if cells:
                    world.transfer(player, bed.map_id, cells[0][0], cells[0][1], "down")
            world.fast_forward(360)
            player.hp = max(player.hp, player.max_hp * 0.4)
            player.needs["hunger"] = min(player.needs["hunger"], 60.0)
            player.needs["fatigue"] = min(player.needs["fatigue"], 40.0)
            world.notify("You wake in the medbay. Six hours are gone.", "warn")
            world.add_log("You collapsed and were carried to the medbay.", "player")
            self._snap_camera()
        self.app.audio.play("power_down")
        self.app.fade(wake, 0.8, hold=0.6)

    # -- player movement -----------------------------------------------------------
    def _update_player(self, dt: float) -> None:
        world = self.world
        p = world.player
        inp = self.input
        p.speed = config.RUN_SPEED if inp.held("run") else config.WALK_SPEED
        remaining = dt
        walking = p.moving                      # already in motion when this frame began
        for _ in range(4):
            if p.moving:
                remaining = movement.advance(world, p, remaining)
                if p.moving or world.requests:
                    return
            direction = inp.direction()
            if direction is None:
                self._turn = 0.0
                self._push = 0.0
                return
            if p.facing != direction:
                p.facing = direction
                if not walking:
                    self._turn = 0.085          # from a standstill, a tap just turns on the spot
            if self._turn > 0:
                self._turn -= remaining
                p.anim_time = 0.0
                return
            reason = movement.try_step(world, p, direction)
            if reason is None:
                self._push = 0.0
                continue
            p.anim_time = 0.0
            if reason == "char":
                self._push += remaining
                if self._push > 0.28:
                    self._swap_places(direction)
                    self._push = 0.0
            elif reason == "locked" and self._lock_msg <= 0:
                self._lock_msg = 1.5
                self.toasts.add("Locked.", P.AMBER[3])
                self.app.audio.play("error")
            return

    def _swap_places(self, direction: str) -> None:
        """Squeeze past an NPC who is standing in the way."""
        world = self.world
        p = world.player
        dx, dy = config.DIRS[direction]
        other = world.char_at(p.map_id, p.x + dx, p.y + dy, exclude=p)
        if other is None or other.moving:
            return
        other.path = []
        movement.start_step(world, other, -dx, -dy)
        other.facing = config.OPPOSITE[direction]
        movement.start_step(world, p, dx, dy)

    # -- targeting -------------------------------------------------------------------
    def _object_label(self, obj) -> Optional[str]:
        world = self.world
        if obj.get("cond") and not world.script.eval(obj.get("cond"), {"obj": obj.id}):
            return None
        t = obj.type
        player = world.player
        if t == "item":
            return "Take: %s" % world.item_name(obj.get("item"))
        if t == "container":
            if obj.get("locked") and not can_open(player, obj):
                return "Locked: %s" % obj.name
            return "Open: %s" % obj.name
        if t == "console":
            system = world.systems.get(str(obj.get("system", "")))
            return "Use: %s" % (obj.get("name") or (system.name if system else "Console"))
        if t == "terminal":
            return "Read: %s" % obj.name
        if t == "bed":
            sleeper = self._sleeper(obj)
            if sleeper is not None:
                return "Asleep: %s" % sleeper.short_name
            owner = str(obj.get("owner") or "")
            return "Sleep" if owner in ("", "player") else "%s's bunk" % self._owner_name(owner)
        if t in ("door", "portal"):
            if obj.get("locked") and not can_open(player, obj):
                return "Locked: %s" % (obj.get("label") or "door")
            return None
        if t == "sign":
            return "Look"
        if t == "interact" or obj.get("dialogue") or obj.get("on_interact"):
            return "Examine: %s" % obj.name
        return None

    def _owner_name(self, owner: str) -> str:
        ch = self.world.characters.get(owner)
        return ch.short_name if ch else owner.capitalize()

    def _sleeper(self, bed):
        for ch in self.world.chars_in(bed.map_id):
            if ch.asleep and ch.ai.get("bed") == bed.id:
                return ch
        return None

    def _find_target(self):
        world = self.world
        p = world.player
        if p.moving:
            return None
        gmap = world.maps[p.map_id]
        fx, fy = movement.facing_cell(p)
        npc = world.char_at(p.map_id, fx, fy, exclude=p, include_asleep=True)
        if npc is not None and not (npc.asleep and npc.ai.get("bed")):
            name = npc.name if npc.met else (npc.title or "Stranger")
            return ("npc", npc, ("Asleep: %s" if npc.asleep else "Talk: %s") % name)
        for cx, cy in ((fx, fy), (p.x, p.y)):
            for obj in gmap.objects_at(cx, cy):
                label = self._object_label(obj)
                if label:
                    return ("obj", obj, label)
        return None

    # -- interaction -------------------------------------------------------------------
    def _interact(self, target) -> None:
        from .dialogue import DialogueScene
        from .panels import ConsoleScene, ContainerScene, MessageScene, WaitScene
        world, app = self.world, self.app
        kind, thing, _label = target
        player = world.player
        if kind == "npc":
            npc = thing
            if npc.asleep:
                self.toasts.add("%s is fast asleep." % npc.short_name)
                return
            if not npc.moving:
                npc.face_towards(player.x, player.y)
            npc.path = []
            dlg = npc.dialogue if npc.dialogue in world.content.dialogues else "generic"
            if dlg not in world.content.dialogues:
                self.toasts.add("%s has nothing to say." % npc.short_name)
                return
            app.audio.play("confirm")
            app.push_scene(DialogueScene(app, world, dlg, npc.id))
            return
        obj = thing
        ctx = {"obj": obj.id, "map": obj.map_id}
        t = obj.type
        if t == "item":
            world.give_item(player, str(obj.get("item")), int(obj.get("qty", 1)))
            world.remove_object(obj)
            app.audio.play("pickup")
            return
        if t in ("door", "portal", "container") and obj.get("locked") and not can_open(player, obj):
            self._lock_prompt(obj)
            return
        if obj.get("on_interact"):
            world.script.run(obj.get("on_interact"), ctx)
        if t == "container":
            app.audio.play("confirm")
            app.push_scene(ContainerScene(app, world, obj))
        elif t == "console":
            system = world.systems.get(str(obj.get("system", "")))
            if system is not None:
                app.audio.play("confirm")
                app.push_scene(ConsoleScene(app, world, system, obj))
        elif t == "bed":
            sleeper = self._sleeper(obj)
            owner = str(obj.get("owner") or "")
            if sleeper is not None:
                self.toasts.add("%s is fast asleep." % sleeper.short_name)
            elif owner in ("", "player"):
                app.push_scene(WaitScene(app, world, self, bed=obj))
            else:
                self.toasts.add("That is %s's bunk." % self._owner_name(owner))
        elif obj.get("dialogue") and obj.get("dialogue") in world.content.dialogues:
            app.audio.play("confirm")
            app.push_scene(DialogueScene(app, world, str(obj.get("dialogue")), "", obj.id))
        elif obj.get("text"):
            app.push_scene(MessageScene(app, world.text(str(obj.get("text")), ctx), obj.get("name") or ""))

    def _lock_prompt(self, obj) -> None:
        from .panels import ChoiceScene
        world, app = self.world, self.app
        player = world.player
        level = int(obj.get("lock_level", 0) or 0)
        need = []
        if obj.get("key"):
            need.append(world.item_name(str(obj.get("key"))))
        for group in obj.access_groups():
            need.append("%s access" % group.capitalize())
        text = "It is locked."
        if need:
            text += " Opens with: " + " or ".join(need) + "."
        options: List[Tuple[str, Any]] = []
        if level > 0:
            chance = world.check_chance(player, "tech", level)

            def attempt() -> None:
                world.fast_forward(10)
                if world.skill_check(player, "tech", level):
                    obj.set("locked", False)
                    world.nav.invalidate()
                    world.notify("Lock bypassed.", "good")
                    app.audio.play("skill_pass")
                    world.api.add_xp(15)
                else:
                    world.notify("The lock holds. Ten minutes wasted.", "bad")
                    app.audio.play("skill_fail")
                if obj.get("restricted") or obj.get("owner"):
                    world.report_theft(obj)
            options.append(("Bypass the lock  <y>[Tech %d%%]</y>" % chance, attempt))
        options.append(("Leave it", None))
        app.audio.play("error")
        app.push_scene(ChoiceScene(app, obj.get("label") or obj.name, text, options))

    def _open_menu(self, tab: str) -> None:
        from .menu import DatapadScene
        self.app.audio.play("confirm")
        self.app.push_scene(DatapadScene(self.app, self.world, self, tab))

    # -- saving -------------------------------------------------------------------------
    def autosave(self) -> None:
        try:
            save.save_game(self.world, "autosave")
        except OSError:
            pass

    def quicksave(self) -> None:
        try:
            save.save_game(self.world, "quicksave")
            self.toasts.add("Quick saved.", P.UI["good"])
            self.app.audio.play("save")
        except OSError as exc:
            self.toasts.add("Save failed: %s" % exc, P.RED[3])

    def quickload(self) -> None:
        if save.read_header("quicksave") is None:
            self.toasts.add("No quick save yet.", P.AMBER[3])
            return
        self.load_slot("quicksave")

    def load_slot(self, slot: str) -> bool:
        app = self.app
        try:
            world = save.load_game(app.content, slot)
        except (OSError, ValueError, KeyError) as exc:
            self.toasts.add("Load failed: %s" % exc, P.RED[3])
            return False
        app.fade(lambda: app.set_scene(PlayScene(app, world)), 0.25)
        return True

    # -- ambience -----------------------------------------------------------------------
    def _update_doors(self, dt: float) -> None:
        world = self.world
        p = world.player
        gmap = world.maps[p.map_id]
        chars = world.chars_in(p.map_id)
        for obj in gmap.objects:
            if obj.type not in ("door", "portal"):
                continue
            want = 0.0
            for ch in chars:
                if abs(ch.x - obj.x) <= 1 and abs(ch.y - obj.y) <= 1 and not ch.asleep and can_open(ch, obj):
                    want = 1.0
                    break
            if want > obj.anim:
                if obj.anim <= 0.01 and abs(p.x - obj.x) + abs(p.y - obj.y) <= 4 and obj.type == "door":
                    self.app.audio.play("door", 0.6)
                obj.anim = min(1.0, obj.anim + dt * 6.0)
            elif want < obj.anim:
                obj.anim = max(0.0, obj.anim - dt * 4.0)

    def _collect_toasts(self) -> None:
        world = self.world
        if not world.toasts:
            return
        for text, kind in world.toasts:
            self.toasts.add(text, TOAST_COLORS.get(kind, P.UI["text"]))
            if kind == "quest":
                self.app.audio.play("quest")
            elif kind == "warn" and self._alert_cooldown <= 0:
                self.app.audio.play("alert", 0.7)
                self._alert_cooldown = 4.0
            elif kind == "good":
                self.app.audio.play("pickup", 0.5)
        world.toasts.clear()

    def _snap_camera(self) -> None:
        p = self.world.player
        gmap = self.world.maps[p.map_id]
        px, py = p.pixel_pos()
        self.cam = MapRenderer.camera_for(gmap, px + 8, py + 4, config.VIEW_W, config.VIEW_H)

    def _update_camera(self, dt: float) -> None:
        self._snap_camera()

    # ------------------------------------------------------------------
    # draw
    # ------------------------------------------------------------------
    def draw(self, surf) -> None:
        world = self.world
        p = world.player
        gmap = world.maps[p.map_id]
        cam_x, cam_y = self.cam
        chars = world.chars_in(p.map_id)
        px, py = p.pixel_pos()
        self.renderer.draw(surf, gmap, cam_x, cam_y, self.app.time, world, chars,
                           light_level=world.systems.light_level(),
                           extra_lights=[(px + 8, py + 2, 46, (150, 146, 130))])
        if self.debug:
            self._draw_debug(surf, gmap, chars)
        top = self.app.scene
        if top is self:
            self._draw_target_marker(surf)
        # windows that dim the screen bring their own header; keep the HUD for the rest
        if top is self or top.name in ("dialogue", "message"):
            self._draw_hud(surf, gmap)

    def _draw_target_marker(self, surf) -> None:
        if self.target is None:
            return
        kind, thing, _ = self.target
        cam_x, cam_y = self.cam
        if kind == "npc":
            px, py = thing.pixel_pos()
            x, y = int(px) - cam_x + 5, int(py) - cam_y - 18
        else:
            x, y = thing.x * T - cam_x + 5, thing.y * T - cam_y - 8
        bob = int(math.sin(self.app.time * 6.0) * 1.5)
        self.app.font.draw(surf, "▼", (x, y + bob), P.AMBER[3], shadow=P.BLACK)

    def _draw_hud(self, surf, gmap) -> None:
        world = self.world
        font = self.app.font
        p = world.player
        place = gmap.place_name(p.x, p.y)
        label = gmap.name if place == gmap.name else "%s  •  %s" % (gmap.name, place)
        if place != self._last_place:
            self._last_place = place
            self._place_flash = 3.0
        font.draw(surf, label, (5, 4), P.WHITE if self._place_flash > 0 else P.STEEL[4], shadow=P.BLACK)
        clock = world.clock
        font.draw(surf, "Day %d  %s" % (clock.day, clock.time_str()), (config.VIEW_W - 5, 4), P.WHITE,
                  shadow=P.BLACK, align="right")
        systems = world.systems
        alerts = []
        if systems.brownout:
            alerts.append("<y>POWER</y>")
        if "air" in systems.resources and systems.resource("air") < 60:
            alerts.append("<r>AIR</r>")
        if "water" in systems.resources and systems.resource_frac("water") < 0.2:
            alerts.append("<c>WATER</c>")
        if alerts and int(self.app.time * 2) % 2 == 0:
            font.draw(surf, "⚠ " + " ".join(alerts), (config.VIEW_W - 5, 15), P.AMBER[3],
                      shadow=P.BLACK, align="right")
        self.toasts.draw(surf, config.VIEW_W - 4, 27, font)

        # vitals
        y = config.VIEW_H - 27
        backing = self.app.pygame.Surface((78, 28), self.app.pygame.SRCALPHA)
        backing.fill((P.NAVY[0][0], P.NAVY[0][1], P.NAVY[0][2], 170))
        surf.blit(backing, (2, y - 3))
        hp_frac = p.hp / max(1, p.max_hp)
        for i, (name, frac, color) in enumerate((
                ("HP", hp_frac, ui.level_color(hp_frac)),
                ("FOOD", 1.0 - p.needs["hunger"] / 100.0, P.AMBER[2]),
                ("REST", 1.0 - p.needs["fatigue"] / 100.0, P.CYAN[2]))):
            font.draw(surf, name, (5, y + i * 8 - 1), P.STEEL[4], shadow=P.BLACK)
            ui.bar(surf, (30, y + i * 8, 44, 7), frac, color)
        if self.target is not None and self.app.scene is self:
            text = "<y>E</y> %s" % self.target[2]
            w = font.width(text)
            x = (config.VIEW_W - w) // 2
            surf.fill(P.BLACK, (x - 5, config.VIEW_H - 16, w + 10, 13))
            surf.fill(P.NAVY[1], (x - 4, config.VIEW_H - 15, w + 8, 11))
            font.draw(surf, text, (x, config.VIEW_H - 14), P.WHITE)

    def _draw_debug(self, surf, gmap, chars) -> None:
        world = self.world
        font = self.app.font
        cam_x, cam_y = self.cam
        for ch in chars:
            px, py = ch.pixel_pos()
            x, y = int(px) - cam_x, int(py) - cam_y
            for cx, cy in ch.path[:40]:
                surf.fill(P.AMBER[3], (cx * T - cam_x + 7, cy * T - cam_y + 7, 2, 2))
            if not ch.is_player:
                font.draw(surf, "%s:%s" % (ch.id, ch.ai.get("goal", "")), (x - 8, y - 24), P.GREEN[4], shadow=P.BLACK)
        for obj in gmap.objects:
            col = P.PURPLE[3] if obj.type != "container" else P.AMBER[3]
            surf.fill(col, (obj.x * T - cam_x, obj.y * T - cam_y, 3, 3))
        p = world.player
        s = world.systems
        lines = [
            "fps %d  %s (%d,%d)  seed-time %.0f" % (self._fps, p.map_id, p.x, p.y, world.clock.minutes),
            "power %.0f/%.0f  air %.0f  water %.0f" % (s.power_supply, s.power_demand, s.resource("air"),
                                                         s.resource("water")),
            "F6 +1h  F7 +6h   script errors: %d" % len(world.script.errors),
        ]
        if world.script.errors:
            lines.append(world.script.errors[-1][:90])
        for i, line in enumerate(lines):
            font.draw(surf, line, (5, 40 + i * 10), P.GREEN[4], shadow=P.BLACK)
