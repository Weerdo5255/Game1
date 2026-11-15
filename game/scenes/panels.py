"""Pop-up windows: messages, choices, containers, trading, system consoles,
resting and the save/load list."""
from __future__ import annotations

from typing import Any, Callable, List, Optional, Sequence, Tuple

import pygame

from .. import config, save
from ..engine import palette as P
from ..engine import ui
from ..engine.scene import Scene
from ..sim import ai
from ..world.items import ItemDef, SLOT_LABELS

W, H = config.VIEW_W, config.VIEW_H


def mods_text(mods: dict) -> str:
    """Readable summary of stat modifiers: "Engineering +1, Carry +6"."""
    parts = []
    for key, value in sorted(mods.items()):
        name = key.split(".", 1)[-1].replace("_", " ")
        if key == "skill.all":
            name = "all skills"
        if key in ("cold_resist", "air_supply"):
            parts.append({"cold_resist": "cold protection", "air_supply": "own air supply"}[key])
            continue
        if key in ("speed", "work"):
            parts.append("%s %+d%%" % (name, round(value * 100)))
        else:
            parts.append("%s %+d" % (name.capitalize(), int(value)))
    return ", ".join(parts)


def item_lines(idef: ItemDef) -> List[str]:
    lines = []
    kind = idef.kind.capitalize()
    if idef.slot:
        kind += " (%s)" % SLOT_LABELS.get(idef.slot, idef.slot)
    lines.append("<d>%s  •  %.1f kg  •  %d cr</d>" % (kind, idef.weight, idef.value))
    if idef.mods:
        lines.append("<c>%s</c>" % mods_text(idef.mods))
    return lines


# ---------------------------------------------------------------------------
class MessageScene(Scene):
    """A block of text to read; any key closes it."""
    transparent = True
    name = "message"

    def __init__(self, app, text: str, title: str = "", on_close: Optional[Callable[[], None]] = None):
        super().__init__(app)
        self.rect = pygame.Rect(4, 150, W - 8, 62)
        self.title = title
        self.writer = ui.Typewriter(text, self.rect.w - 16, 4, app.text_cps, app.font)
        self.on_close = on_close

    def update(self, dt: float) -> None:
        self.writer.update(dt)
        if self.input.pressed("confirm") or self.input.pressed("cancel"):
            if not self.writer.page_done:
                self.writer.skip()
            elif not self.writer.next_page():
                ui.sound("move")
                self.close()
                if self.on_close:
                    self.on_close()

    def draw(self, surf) -> None:
        inner = ui.panel(surf, self.rect, title=self.title or None, border=P.STEEL[3])
        self.writer.draw(surf, inner.x + 2, self.rect.y + 12, P.STEEL[5])
        if self.writer.page_done and int(self.app.time * 3) % 2 == 0:
            self.app.font.draw(surf, "▼", (self.rect.right - 12, self.rect.bottom - 12), P.UI["select"])


class ChoiceScene(Scene):
    """A question with a short list of answers."""
    transparent = True
    name = "choice"

    def __init__(self, app, title: str, text: str, options: Sequence[Tuple[str, Optional[Callable[[], None]]]],
                 width: int = 240):
        super().__init__(app)
        self.title = title
        self.options = list(options)
        self.lines = app.font.wrap(text, width - 16) if text else []
        self.menu = ui.Menu([ui.MenuItem(label) for label, _ in self.options], visible=8)
        h = 14 + len(self.lines) * 11 + (4 if self.lines else 0) + len(self.options) * 11 + 6
        self.rect = pygame.Rect((W - width) // 2, (H - h) // 2, width, h)

    def update(self, dt: float) -> None:
        result = self.menu.update(self.input)
        if result == "cancel":
            self.close()
        elif result == "select":
            fn = self.options[self.menu.index][1]
            self.close()
            if fn:
                fn()

    def draw(self, surf) -> None:
        ui.shade(surf, 90)
        inner = ui.panel(surf, self.rect, title=self.title or None)
        y = self.rect.y + 12
        for line in self.lines:
            self.app.font.draw(surf, line, (inner.x + 2, y), P.UI["text"])
            y += 11
        if self.lines:
            y += 4
        self.menu.draw(surf, inner.x, y, inner.w, self.app.time)


# ---------------------------------------------------------------------------
class TwoListScene(Scene):
    """Shared layout for the container and trade windows."""
    transparent = True
    left_title = ""
    right_title = ""

    def __init__(self, app, world):
        super().__init__(app)
        self.world = world
        self.side = 0
        self.menus = [ui.Menu(visible=9, wrap=False), ui.Menu(visible=9, wrap=False)]
        self.message = ""

    def enter(self) -> None:
        self.world.busy = True
        self.refresh()

    def refresh(self) -> None:
        raise NotImplementedError

    def act(self, item_id: str, everything: bool) -> None:
        raise NotImplementedError

    def footer(self) -> str:
        return ""

    def hints(self):
        return [("E", "Move"), ("R", "Move all"), ("←→", "Switch"), ("X", "Close")]

    def update(self, dt: float) -> None:
        inp = self.input
        if inp.pressed("left") or inp.pressed("right"):
            other = 1 - self.side
            if self.menus[other].items:
                self.side = other
                ui.sound("move")
        menu = self.menus[self.side]
        if not menu.items and self.menus[1 - self.side].items:
            self.side = 1 - self.side
            menu = self.menus[self.side]
        if inp.pressed("tab_next") and menu.selected is not None and menu.selected.enabled:
            ui.sound("confirm")
            self.act(menu.selected.data, True)
            self.refresh()
            return
        result = menu.update(inp)
        if result == "cancel":
            self.close()
        elif result == "select" and menu.selected is not None:
            self.act(menu.selected.data, False)
            self.refresh()

    def draw(self, surf) -> None:
        ui.shade(surf, 110)
        font = self.app.font
        items = self.world.content.items
        lw = 182
        for i, title in enumerate((self.left_title, self.right_title)):
            rect = pygame.Rect(6 + i * (lw + 8), 16, lw, 124)
            inner = ui.panel(surf, rect, title=title,
                             border=P.UI["select"] if i == self.side else P.UI["border_dim"])
            menu = self.menus[i]
            if menu.items:
                menu.draw(surf, inner.x, inner.y + 4, inner.w, self.app.time, focused=i == self.side)
            else:
                font.draw(surf, "(empty)", (inner.x + 8, inner.y + 4), P.UI["disabled"])
        detail = pygame.Rect(6, 144, W - 12, 56)
        inner = ui.panel(surf, detail)
        sel = self.menus[self.side].selected
        if sel is not None and sel.data in items:
            idef = items[sel.data]
            surf.blit(self.app.icons.get(idef.icon, idef.color), (inner.x + 1, inner.y + 1))
            font.draw(surf, idef.name, (inner.x + 22, inner.y), P.UI["select"])
            y = inner.y + 11
            for line in item_lines(idef):
                font.draw(surf, line, (inner.x + 22, y), P.UI["text"])
                y += 10
            ui.draw_text_block(surf, idef.desc, (inner.x + 22, y, inner.w - 26, 22), P.STEEL[4], max_lines=2)
        foot = self.message or self.footer()
        if foot:
            font.draw(surf, foot, (10, 4), P.UI["text"], shadow=P.BLACK)
        ui.key_hints(surf, self.hints())


class ContainerScene(TwoListScene):
    name = "container"

    def __init__(self, app, world, obj):
        super().__init__(app, world)
        self.obj = obj
        self.inv = world.inventory_of(obj)
        self.left_title = obj.name
        self.right_title = "Your pack"
        self._reported = False

    def refresh(self) -> None:
        w = self.world
        left = [ui.MenuItem(w.item_name(i), "x%d" % q, data=i) for i, q in self.inv.entries()]
        right = [ui.MenuItem(w.item_name(i) + (" <c>(E)</c>" if w.player.is_equipped(i) else ""), "x%d" % q, data=i)
                 for i, q in w.player.inventory.entries()]
        self.menus[0].set_items(left)
        self.menus[1].set_items(right)

    def footer(self) -> str:
        p = self.world.player
        over = p.carried_weight() > p.carry_capacity
        return "Carrying %s%.1f / %d kg%s" % ("<r>" if over else "", p.carried_weight(), p.carry_capacity,
                                              "</r>" if over else "")

    def act(self, item_id: str, everything: bool) -> None:
        w = self.world
        p = w.player
        if self.side == 0:
            qty = self.inv.count(item_id) if everything else 1
            moved = self.inv.transfer(p.inventory, item_id, qty)
            if moved:
                w.events.emit("item:" + item_id, item=item_id, count=moved)
                w.events.emit("took:%s:%s" % (self.obj.id, item_id), item=item_id, container=self.obj.id)
                ui.sound("pickup")
                cleared = any(g in p.access_groups() for g in self.obj.access_groups()) \
                    or self.obj.get("owner") == "player"
                if self.obj.get("restricted") and not cleared and not self._reported:
                    self._reported = True
                    seen = w.report_theft(self.obj)
                    if seen is not None:
                        self.message = "<r>%s is watching you.</r>" % seen.short_name
        else:
            idef = w.content.items.get(item_id)
            if idef is not None and idef.quest:
                self.message = "<y>You should hold on to that.</y>"
                ui.sound("error")
                return
            qty = p.inventory.count(item_id) if everything else 1
            moved = p.inventory.transfer(self.inv, item_id, qty)
            p.check_equipment()
            if moved:
                w.events.emit("stored:%s:%s" % (self.obj.id, item_id), item=item_id, container=self.obj.id)
                ui.sound("move")
        w.events.pump()


def trade_info(world, npc) -> dict:
    return dict((world.content.characters.get(npc.id) or {}).get("trade") or {})


def buy_price(world, npc, idef: ItemDef) -> int:
    info = trade_info(world, npc)
    price = idef.value * float(info.get("markup", 1.5))
    rel = max(-50.0, min(50.0, npc.opinion_of("player")))
    price *= 1.0 - rel / 200.0
    price *= 1.0 - 0.02 * (world.player.attr("presence") - 4)
    stock = world.stock(idef.id)
    if stock < 3:
        price *= 1.5
    elif stock > 12:
        price *= 0.85
    return max(1, int(round(price)))


def sell_price(world, npc, idef: ItemDef) -> int:
    info = trade_info(world, npc)
    rel = max(-50.0, min(50.0, npc.opinion_of("player")))
    price = idef.value * float(info.get("buys", 0.5)) * (1.0 + rel / 400.0)
    return max(0, int(price))


class TradeScene(TwoListScene):
    name = "trade"

    def __init__(self, app, world, npc):
        super().__init__(app, world)
        self.npc = npc
        self.left_title = "%s's goods" % npc.short_name
        self.right_title = "Your pack"
        info = trade_info(world, npc)
        self.stock_objs = [o for o in (world.find_object(s) for s in info.get("stock") or []) if o is not None]

    def hints(self):
        return [("E", "Buy / sell"), ("←→", "Switch"), ("X", "Close")]

    def _stock(self):
        totals = {}
        for obj in self.stock_objs:
            for item_id, qty in self.world.inventory_of(obj).entries():
                totals[item_id] = totals.get(item_id, 0) + qty
        for item_id, qty in self.npc.inventory.entries():
            idef = self.world.content.items.get(item_id)
            if idef is not None and not idef.quest and idef.kind != "key":
                totals[item_id] = totals.get(item_id, 0) + qty
        return totals

    def refresh(self) -> None:
        w = self.world
        items = w.content.items
        left = []
        for item_id, qty in sorted(self._stock().items()):
            idef = items.get(item_id)
            if idef is None or idef.quest or idef.value <= 0:
                continue
            price = buy_price(w, self.npc, idef)
            left.append(ui.MenuItem("%s x%d" % (idef.name, qty), "%d cr" % price, data=item_id,
                                    enabled=w.player.credits >= price))
        right = []
        for item_id, qty in w.player.inventory.entries():
            idef = items.get(item_id)
            if idef is None:
                continue
            price = sell_price(w, self.npc, idef)
            ok = not idef.quest and price > 0 and self.npc.credits >= price and not w.player.is_equipped(item_id)
            right.append(ui.MenuItem("%s x%d" % (idef.name, qty), "%d cr" % price if price else "-",
                                     data=item_id, enabled=ok))
        self.menus[0].set_items(left)
        self.menus[1].set_items(right)

    def footer(self) -> str:
        return "Your credits: <y>%d</y>     %s has: <y>%d</y>" % (
            self.world.player.credits, self.npc.short_name, self.npc.credits)

    def act(self, item_id: str, everything: bool) -> None:
        w = self.world
        p = w.player
        idef = w.content.items.get(item_id)
        if idef is None:
            return
        if self.side == 0:
            price = buy_price(w, self.npc, idef)
            if p.credits < price:
                ui.sound("error")
                return
            taken = self.npc.inventory.remove(item_id, 1)
            if not taken:
                for obj in self.stock_objs:
                    taken = w.inventory_of(obj).remove(item_id, 1)
                    if taken:
                        break
            if taken:
                p.credits -= price
                self.npc.credits += price
                p.inventory.add(item_id, 1)
                w.events.emit("item:" + item_id, item=item_id, count=1)
                w.events.emit("bought:" + item_id, item=item_id, npc=self.npc.id)
                ui.sound("pickup")
        else:
            price = sell_price(w, self.npc, idef)
            if idef.quest or price <= 0 or self.npc.credits < price or p.is_equipped(item_id):
                ui.sound("error")
                return
            if p.inventory.remove(item_id, 1):
                p.credits += price
                self.npc.credits -= price
                target = w.inventory_of(self.stock_objs[0]) if self.stock_objs else self.npc.inventory
                target.add(item_id, 1)
                w.events.emit("sold:" + item_id, item=item_id, npc=self.npc.id)
                ui.sound("pickup")
        w.events.pump()


# ---------------------------------------------------------------------------
STATUS_COLORS = {"ok": P.UI["good"], "degraded": P.UI["warn"], "critical": P.UI["bad"], "offline": P.RED[1]}


def system_detail_lines(world, system) -> List[str]:
    """Shared by the console window and the datapad's station page."""
    nets = world.systems
    lines = []
    if system.kind == "generator":
        lines.append("Output <c>%.0f kW</c> of %.0f" % (system.output, system.power_out))
        if system.fuel:
            hopper = world.find_object(str(system.fuel.get("hopper", "")))
            cells = world.inventory_of(hopper).count(str(system.fuel.get("item", ""))) if hopper else 0
            lines.append("Fuel: <c>%.0f h</c> burning, %d in hopper" % (max(0.0, system.fuel_left), cells))
    else:
        got = system.power * system.power_ratio if system.enabled else 0.0
        col = "g" if system.power_ratio >= 0.99 else "y" if system.power_ratio > 0.01 else "r"
        lines.append("Power <%s>%.0f / %.0f kW</%s>   queue position %d" % (col, got, system.power, col,
                                                                            system.priority))
    lines.append("Efficiency <c>%d%%</c>%s" % (round(system.efficiency * 100),
                                               "" if system.enabled else "  <r>(switched off)</r>"))
    if system.operator > 0:
        who = world.characters.get(system.operator_id)
        lines.append("Crewing <c>%d%%</c>%s" % (round(min(1.0, system.labour) * 100),
                                                "  (%s on shift)" % who.short_name if who else ""))
    flows = []
    for name, rate in system.consumes.items():
        flows.append("-%g %s/h" % (rate, nets.resource_defs.get(name, {}).get("name", name).lower()))
    for name, rate in system.produces.items():
        flows.append("+%g %s/h" % (rate, nets.resource_defs.get(name, {}).get("name", name).lower()))
    if flows:
        lines.append("<d>" + ", ".join(flows) + "</d>")
    for i, recipe in enumerate(system.makes):
        lines.append("Making %s: <c>%d%%</c>" % (world.item_name(str(recipe.get("item"))),
                                                 round(max(0.0, min(1.0, system.progress[i])) * 100)))
    if system.blocked:
        lines.append("<y>Stalled: %s</y>" % system.blocked)
    return lines


class ConsoleScene(Scene):
    """Control panel of one station system."""
    transparent = True
    name = "console"

    def __init__(self, app, world, system, obj=None):
        super().__init__(app)
        self.world = world
        self.system = system
        self.obj = obj
        self.menu = ui.Menu(visible=7)
        self.message = ""

    def enter(self) -> None:
        self.world.busy = True
        self._build()

    def _guardian(self):
        """An NPC who works this system, is standing nearby and will object."""
        w = self.world
        p = w.player
        for npc in w.chars_in(p.map_id):
            if npc.is_player or npc.asleep:
                continue
            if (npc.job or {}).get("system") == self.system.id and abs(npc.x - p.x) + abs(npc.y - p.y) <= 5:
                return npc
        return None

    def _build(self) -> None:
        w, s = self.world, self.system
        p = w.player
        parts = p.inventory.count_tag(w.content.items, "part")
        items = []
        gain = int(round(ai.repair_amount(p)))
        items.append(ui.MenuItem("Fit a spare part  <d>(+%d%%, 30 min)</d>" % gain, "have %d" % parts,
                                 enabled=parts > 0 and s.condition < 99.5, data="repair"))
        if s.condition < 45:
            items.append(ui.MenuItem("Jury-rig it  <d>(no parts, 1 hour)</d>", data="jury"))
        if s.kind != "generator":
            items.append(ui.MenuItem("Move up the power queue", data="prio_up", enabled=s.priority > 1))
            items.append(ui.MenuItem("Move down the power queue", data="prio_down", enabled=s.priority < 9))
        if not s.essential:
            items.append(ui.MenuItem("Switch %s" % ("OFF" if s.enabled else "ON"), data="toggle"))
        items.append(ui.MenuItem("Run diagnostics", data="diag"))
        items.append(ui.MenuItem("Leave", data="leave"))
        self.menu.set_items(items)

    def _may_control(self, harmful: bool) -> bool:
        w = self.world
        if not (w.flags.get("audit_authority") or w.player.skill("tech") >= 4):
            self.message = "<y>The board wants an authorisation you do not have.</y> <d>(Tech 4)</d>"
            ui.sound("error")
            return False
        guard = self._guardian()
        if guard is not None and harmful:
            if guard.opinion_of("player") < 20 and not w.flags.get("audit_authority"):
                self.message = "<r>%s puts a hand on the board. \"Not on my watch.\"</r>" % guard.short_name
                ui.sound("error")
                return False
            guard.adjust_relation("player", -8)
            w.notify("%s disapproves" % guard.short_name, "bad")
        return True

    def update(self, dt: float) -> None:
        w, s = self.world, self.system
        result = self.menu.update(self.input)
        if result == "cancel":
            self.close()
            return
        if result != "select":
            return
        action = self.menu.selected.data
        self.message = ""
        if action == "leave":
            self.close()
            return
        if action == "repair":
            ok, msg = w.player_repair(s.id)
            self.message = ("<g>%s</g>" if ok else "<y>%s</y>") % msg
            if ok:
                self.app.audio.play("repair")
                w.fast_forward(30)
                w.api.add_xp(10)
        elif action == "jury":
            p = w.player
            gained = s.repair((1.5 + 0.35 * p.skill("engineering")) * p.condition_factor())
            w.fast_forward(60)
            self.app.audio.play("repair")
            self.message = "<g>An hour of improvising: +%d%% integrity.</g>" % round(gained)
        elif action in ("prio_up", "prio_down"):
            if self._may_control(action == "prio_down"):
                s.priority = max(1, min(9, s.priority + (-1 if action == "prio_up" else 1)))
                w.systems.tick(w, 1e-6)
                w.events.emit("priority:" + s.id, system=s.id, priority=s.priority)
                w.add_log("You moved %s to position %d in the power queue." % (s.name, s.priority), "player")
                self.message = "<g>%s is now number %d in the power queue.</g>" % (s.name, s.priority)
        elif action == "toggle":
            if self._may_control(s.enabled):
                s.enabled = not s.enabled
                w.systems.tick(w, 1e-6)
                w.events.emit("power:%s:%s" % (s.id, "on" if s.enabled else "off"), system=s.id)
                w.add_log("You switched %s %s." % (s.name, "on" if s.enabled else "off"), "player")
                self.message = "<g>%s switched %s.</g>" % (s.name, "on" if s.enabled else "off")
                self.app.audio.play("power_down" if not s.enabled else "confirm")
        elif action == "diag":
            from .panels import MessageScene  # noqa: F401  (same module; kept for clarity)
            text = s.desc or "No notes on file."
            self.app.push_scene(MessageScene(self.app, text, s.name))
        w.events.pump()
        self._build()

    def draw(self, surf) -> None:
        ui.shade(surf, 110)
        w, s = self.world, self.system
        font = self.app.font
        rect = pygame.Rect(40, 14, W - 80, 186)
        inner = ui.panel(surf, rect, title=s.name)
        y = inner.y + 2
        font.draw(surf, "Integrity", (inner.x + 2, y), P.UI["dim"])
        ui.bar(surf, (inner.x + 52, y, 150, 8), s.condition / 100.0, ui.level_color(s.condition / 100.0))
        font.draw(surf, "%d%%" % round(s.condition), (inner.x + 208, y), P.UI["text"])
        font.draw(surf, s.status.upper(), (inner.right - 2, y), STATUS_COLORS.get(s.status, P.WHITE), align="right")
        y += 13
        for line in system_detail_lines(w, s):
            font.draw(surf, line, (inner.x + 2, y), P.UI["text"])
            y += 10
        y += 3
        surf.fill(P.UI["border_dim"], (inner.x, y, inner.w, 1))
        y += 5
        self.menu.draw(surf, inner.x, y, inner.w, self.app.time)
        if self.message:
            ui.draw_text_block(surf, self.message, (inner.x + 2, rect.bottom - 26, inner.w - 4, 22), max_lines=2)
        ui.key_hints(surf, [("E", "Select"), ("X", "Close")])


# ---------------------------------------------------------------------------
class WaitScene(Scene):
    """Rest / sleep: the simulation runs on fast-forward while you watch."""
    transparent = True
    name = "wait"

    def __init__(self, app, world, play, bed=None):
        super().__init__(app)
        self.world = world
        self.play = play
        self.bed = bed
        self.running = False
        self.remaining = 0.0
        self.until_rested = False
        self.elapsed = 0.0
        self._log_mark = len(world.log)
        if bed is not None:
            options = [("Sleep until rested", ("rest", 600)), ("Nap for two hours", ("time", 120)),
                       ("Stay up", None)]
        else:
            options = [("Wait one hour", ("time", 60)), ("Wait three hours", ("time", 180)),
                       ("Wait six hours", ("time", 360)), ("Never mind", None)]
        self.options = options
        self.menu = ui.Menu([ui.MenuItem(label) for label, _ in options], visible=5)

    def enter(self) -> None:
        self.world.busy = True

    def _start(self, spec) -> None:
        kind, minutes = spec
        self.running = True
        self.remaining = float(minutes)
        self.until_rested = kind == "rest"
        self.elapsed = 0.0
        self._log_mark = len(self.world.log)
        if self.bed is not None:
            self.world.player.asleep = True
            self.world.player.ai["bed"] = self.bed.id
            self.world.events.emit("sleep")
            self.app.audio.play("sleep")

    def _finish(self) -> None:
        w = self.world
        p = w.player
        if self.bed is not None:
            p.asleep = False
            p.ai.pop("bed", None)
            w.events.emit("wake")
        hours = self.elapsed / 60.0
        w.notify(("You slept %.1f hours." if self.bed is not None else "%.1f hours pass.") % hours)
        w.events.pump()
        self.close()
        self.play.autosave()

    def update(self, dt: float) -> None:
        w = self.world
        if not self.running:
            result = self.menu.update(self.input)
            if result == "cancel":
                self.close()
            elif result == "select":
                spec = self.options[self.menu.index][1]
                if spec is None:
                    self.close()
                else:
                    self._start(spec)
            return
        chunk = min(self.remaining, 10.0)
        w.fast_forward(chunk)
        self.remaining -= chunk
        self.elapsed += chunk
        stop = self.remaining <= 0 or w.ended is not None or bool(w.requests)
        if self.until_rested and w.player.needs["fatigue"] <= 4 and self.elapsed >= 60:
            stop = True
        for text, kind in w.toasts:
            if kind == "warn" and ("CRITICAL" in text or "OFFLINE" in text or "collapsed" in text):
                stop = True
        if self.input.pressed("cancel") and self.elapsed >= 20:
            stop = True
        if stop:
            self._finish()

    def draw(self, surf) -> None:
        font = self.app.font
        w = self.world
        if not self.running:
            ui.shade(surf, 90)
            rect = pygame.Rect(W // 2 - 80, H // 2 - 38, 160, 30 + len(self.options) * 11)
            inner = ui.panel(surf, rect, title="Rest" if self.bed is None else "Bunk")
            font.draw(surf, "The station carries on without you.", (inner.x + 2, inner.y + 1), P.UI["dim"])
            self.menu.draw(surf, inner.x, inner.y + 15, inner.w, self.app.time)
            return
        ui.shade(surf, 215, P.NAVY[0])
        self.app.font_big.draw(surf, w.clock.time_str(), (W // 2, 56), P.WHITE, shadow=P.BLACK, align="center")
        font.draw(surf, "Day %d" % w.clock.day, (W // 2, 44), P.UI["accent"], align="center")
        font.draw(surf, "Sleeping..." if self.bed is not None else "Time passes...", (W // 2, 82),
                  P.UI["dim"], align="center")
        y = 104
        for entry in w.log[max(self._log_mark, len(w.log) - 8):]:
            color = P.AMBER[3] if entry.get("kind") == "announce" else P.STEEL[4]
            font.draw(surf, "%s  %s" % (w.clock.format_stamp(entry["t"])[-5:], entry["text"][:74]), (30, y), color)
            y += 10
        ui.key_hints(surf, [("X", "Wake up" if self.bed is not None else "Stop waiting")])


# ---------------------------------------------------------------------------
class SaveLoadScene(Scene):
    transparent = True
    name = "saveload"

    def __init__(self, app, mode: str, world=None, play=None, on_load: Optional[Callable[[str], None]] = None):
        super().__init__(app)
        self.mode = mode                  # "save" or "load"
        self.world = world
        self.play = play
        self.on_load = on_load
        self.menu = ui.Menu(visible=8, wrap=True)
        self.entries: List[dict] = []
        self.message = ""

    def enter(self) -> None:
        self._refresh()

    def _refresh(self) -> None:
        self.entries = save.list_saves(include_special=self.mode == "load")
        items = []
        for e in self.entries:
            h = e["header"]
            if h:
                right = "%s  %s" % (h.get("stamp", ""), save.format_playtime(h.get("playtime", 0)))
                items.append(ui.MenuItem("%s  <d>%s, %s</d>" % (e["label"], h.get("name", "?"), h.get("place", "")),
                                         right, data=e["slot"]))
            else:
                items.append(ui.MenuItem("%s  <d>- empty -</d>" % e["label"], data=e["slot"],
                                         enabled=self.mode == "save"))
        self.menu.set_items(items)

    def _do_save(self, slot: str) -> None:
        try:
            save.save_game(self.world, slot)
            self.message = "<g>Saved to %s.</g>" % save.slot_label(slot)
            self.app.audio.play("save")
        except OSError as exc:
            self.message = "<r>Save failed: %s</r>" % exc
        self._refresh()

    def update(self, dt: float) -> None:
        result = self.menu.update(self.input)
        if result == "cancel":
            self.close()
            return
        if result != "select":
            return
        slot = self.menu.selected.data
        entry = next(e for e in self.entries if e["slot"] == slot)
        if self.mode == "save":
            if entry["header"]:
                self.app.push_scene(ChoiceScene(self.app, "Overwrite?", "Replace the save in %s?" % entry["label"],
                                                [("Overwrite", lambda: self._do_save(slot)), ("Cancel", None)]))
            else:
                self._do_save(slot)
        elif entry["header"]:
            self.close()
            if self.on_load:
                self.on_load(slot)
            elif self.play is not None:
                self.play.load_slot(slot)

    def draw(self, surf) -> None:
        ui.shade(surf, 150)
        rect = pygame.Rect(20, 30, W - 40, 130)
        inner = ui.panel(surf, rect, title="Save game" if self.mode == "save" else "Load game")
        self.menu.draw(surf, inner.x, inner.y + 4, inner.w, self.app.time)
        if self.message:
            self.app.font.draw(surf, self.message, (inner.x + 2, rect.bottom - 14), P.UI["text"])
        ui.key_hints(surf, [("E", "Select"), ("X", "Back")])
