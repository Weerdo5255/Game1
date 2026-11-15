"""A small mouse-driven widget toolkit for the editors.

The game itself is keyboard driven and draws at 384x216; the editors need
buttons, lists and text fields at desktop resolution, so they use this
instead.  Everything is drawn with the same bitmap font and palette as the
game.  Widgets are plain objects with a rectangle; a :class:`ToolApp` owns
the window, dispatches events and runs modal dialogs.

``ToolApp(headless=True)`` plus :meth:`ToolApp.click` / :meth:`ToolApp.press`
let the tests drive an editor exactly like a user would.
"""
from __future__ import annotations

import json
import os
from typing import Any, Callable, List, Optional, Sequence, Tuple

import pygame

from game.engine import palette as P
from game.engine.font import get_font

BG = P.NAVY[0]
PANEL = P.NAVY[1]
PANEL_HI = P.NAVY[2]
BORDER = P.CYAN[1]
TEXT = P.WHITE
DIM = P.STEEL[3]
ACCENT = P.CYAN[3]
SELECT = P.AMBER[3]
ROW = 22                     # standard row height


def font(scale: int = 2):
    return get_font(scale)


class Widget:
    def __init__(self, rect):
        self.rect = pygame.Rect(rect)
        self.visible = True
        self.enabled = True
        self.tooltip = ""

    def handle(self, event, app) -> bool:
        return False

    def update(self, dt: float) -> None:
        pass

    def draw(self, surf, app) -> None:
        pass


class Label(Widget):
    def __init__(self, rect, text: str = "", color=TEXT, align: str = "left", scale: int = 2):
        super().__init__(rect)
        self.text = text
        self.color = color
        self.align = align
        self.scale = scale

    def draw(self, surf, app) -> None:
        f = font(self.scale)
        x = self.rect.x if self.align == "left" else self.rect.centerx if self.align == "center" else self.rect.right
        f.draw(surf, self.text, (x, self.rect.y + (self.rect.h - f.line_height) // 2 + 1), self.color, align=self.align)


class Button(Widget):
    def __init__(self, rect, text: str, on_click: Optional[Callable[[], None]] = None, toggle: bool = False,
                 active: bool = False, icon=None, scale: int = 2, tooltip: str = ""):
        super().__init__(rect)
        self.text = text
        self.on_click = on_click
        self.toggle = toggle
        self.active = active
        self.icon = icon
        self.scale = scale
        self.tooltip = tooltip
        self._down = False

    def handle(self, event, app) -> bool:
        if event.type == pygame.MOUSEBUTTONDOWN and event.button == 1 and self.rect.collidepoint(event.pos):
            if self.enabled:
                self._down = True
            return True
        if event.type == pygame.MOUSEBUTTONUP and event.button == 1 and self._down:
            self._down = False
            if self.rect.collidepoint(event.pos) and self.enabled:
                if self.toggle:
                    self.active = not self.active
                if self.on_click:
                    self.on_click()
            return True
        return False

    def draw(self, surf, app) -> None:
        hover = self.rect.collidepoint(app.mouse) and self.enabled
        fill = P.CYAN[0] if self.active else PANEL_HI if hover else PANEL
        if self._down:
            fill = P.NAVY[3]
        pygame.draw.rect(surf, P.BLACK, self.rect)
        pygame.draw.rect(surf, SELECT if self.active else BORDER, self.rect.inflate(-2, -2))
        pygame.draw.rect(surf, fill, self.rect.inflate(-4, -4))
        f = font(self.scale)
        color = TEXT if self.enabled else P.STEEL[2]
        x = self.rect.centerx
        if self.icon is not None:
            surf.blit(self.icon, (self.rect.x + 4, self.rect.y + (self.rect.h - self.icon.get_height()) // 2))
            x += self.icon.get_width() // 2 + 2
        f.draw(surf, self.text, (x, self.rect.y + (self.rect.h - f.line_height) // 2 + 1),
               SELECT if self.active else color, align="center")


class TextInput(Widget):
    """One line of editable text.  ``on_commit`` fires on Enter or focus loss."""

    def __init__(self, rect, value: str = "", on_commit: Optional[Callable[[str], None]] = None,
                 numeric: bool = False, placeholder: str = "", scale: int = 2):
        super().__init__(rect)
        self.value = str(value)
        self.on_commit = on_commit
        self.numeric = numeric
        self.placeholder = placeholder
        self.scale = scale
        self.focused = False
        self._orig = self.value

    def set(self, value: Any) -> None:
        self.value = str(value)
        self._orig = self.value

    def _commit(self) -> None:
        if self.value != self._orig:
            self._orig = self.value
            if self.on_commit:
                self.on_commit(self.value)

    def handle(self, event, app) -> bool:
        if event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:
            inside = self.rect.collidepoint(event.pos) and self.enabled
            if inside and not self.focused:
                app.focus(self)
            elif not inside and self.focused:
                app.focus(None)
            return inside
        if not self.focused:
            return False
        if event.type == pygame.TEXTINPUT:
            for ch in event.text:
                if self.numeric and not (ch.isdigit() or ch in "-."):
                    continue
                if ch.isprintable():
                    self.value += ch
            return True
        if event.type == pygame.KEYDOWN:
            if event.key == pygame.K_BACKSPACE:
                self.value = self.value[:-1]
            elif event.key in (pygame.K_RETURN, pygame.K_KP_ENTER, pygame.K_TAB):
                app.focus(None)
            elif event.key == pygame.K_ESCAPE:
                self.value = self._orig
                app.focus(None)
            elif event.key == pygame.K_v and (event.mod & pygame.KMOD_CTRL):
                try:
                    clip = pygame.scrap.get_text() if pygame.scrap.get_init() else ""
                    self.value += "".join(c for c in (clip or "") if c.isprintable())
                except pygame.error:
                    pass
            return True
        return False

    def on_blur(self) -> None:
        self.focused = False
        self._commit()

    def draw(self, surf, app) -> None:
        f = font(self.scale)
        pygame.draw.rect(surf, P.BLACK, self.rect)
        pygame.draw.rect(surf, SELECT if self.focused else P.CYAN[0], self.rect.inflate(-2, -2))
        pygame.draw.rect(surf, P.NAVY[0], self.rect.inflate(-4, -4))
        y = self.rect.y + (self.rect.h - f.line_height) // 2 + 1
        text = self.value
        max_w = self.rect.w - 12
        while text and f.width(text.replace("<", "<<")) > max_w:
            text = text[1:]
        if text or self.focused:
            end = f.draw(surf, text.replace("<", "<<"), (self.rect.x + 6, y), TEXT)
            if self.focused and int(app.time * 2.5) % 2 == 0:
                surf.fill(SELECT, (end.right + 2 if text else self.rect.x + 6, y, 2, f.line_height - 4))
        elif self.placeholder:
            f.draw(surf, self.placeholder, (self.rect.x + 6, y), P.STEEL[2])


class ListBox(Widget):
    """Scrollable list of ``(label, data)`` rows."""

    def __init__(self, rect, items: Sequence[Tuple[str, Any]] = (), on_select: Optional[Callable[[Any], None]] = None,
                 scale: int = 2, row_h: int = ROW):
        super().__init__(rect)
        self.items: List[Tuple[str, Any]] = list(items)
        self.on_select = on_select
        self.index = -1
        self.scroll = 0
        self.scale = scale
        self.row_h = row_h

    @property
    def rows(self) -> int:
        return max(1, (self.rect.h - 4) // self.row_h)

    @property
    def selected(self) -> Any:
        return self.items[self.index][1] if 0 <= self.index < len(self.items) else None

    def set_items(self, items: Sequence[Tuple[str, Any]], keep: bool = True) -> None:
        old = self.selected if keep else None
        self.items = list(items)
        self.index = -1
        for i, (_label, data) in enumerate(self.items):
            if old is not None and data == old:
                self.index = i
        self.scroll = max(0, min(self.scroll, max(0, len(self.items) - self.rows)))

    def select(self, data: Any) -> None:
        for i, (_label, d) in enumerate(self.items):
            if d == data:
                self.index = i
                if i < self.scroll:
                    self.scroll = i
                elif i >= self.scroll + self.rows:
                    self.scroll = i - self.rows + 1
                return
        self.index = -1

    def handle(self, event, app) -> bool:
        if event.type == pygame.MOUSEWHEEL and self.rect.collidepoint(app.mouse):
            self.scroll = max(0, min(max(0, len(self.items) - self.rows), self.scroll - event.y * 2))
            return True
        if event.type == pygame.MOUSEBUTTONDOWN and event.button == 1 and self.rect.collidepoint(event.pos):
            row = (event.pos[1] - self.rect.y - 2) // self.row_h + self.scroll
            if 0 <= row < len(self.items):
                self.index = row
                if self.on_select:
                    self.on_select(self.items[row][1])
            return True
        return False

    def draw(self, surf, app) -> None:
        f = font(self.scale)
        pygame.draw.rect(surf, P.BLACK, self.rect)
        pygame.draw.rect(surf, P.CYAN[0], self.rect.inflate(-2, -2))
        pygame.draw.rect(surf, P.NAVY[0], self.rect.inflate(-4, -4))
        clip = surf.get_clip()
        surf.set_clip(self.rect.inflate(-4, -4))
        y = self.rect.y + 2
        for i in range(self.scroll, min(len(self.items), self.scroll + self.rows + 1)):
            label = self.items[i][0]
            row = pygame.Rect(self.rect.x + 2, y, self.rect.w - 4, self.row_h)
            if i == self.index:
                pygame.draw.rect(surf, P.NAVY[3], row)
            elif row.collidepoint(app.mouse):
                pygame.draw.rect(surf, PANEL, row)
            f.draw(surf, label, (row.x + 4, y + (self.row_h - f.line_height) // 2 + 1),
                   SELECT if i == self.index else TEXT)
            y += self.row_h
        surf.set_clip(clip)
        if len(self.items) > self.rows:
            track = self.rect.h - 8
            h = max(12, track * self.rows // len(self.items))
            top = self.rect.y + 4 + (track - h) * self.scroll // max(1, len(self.items) - self.rows)
            pygame.draw.rect(surf, P.CYAN[1], (self.rect.right - 7, top, 4, h))


class Cycler(Widget):
    """``< value >`` selector for a short list of options."""

    def __init__(self, rect, options: Sequence[Any], value: Any = None,
                 on_change: Optional[Callable[[Any], None]] = None, scale: int = 2,
                 fmt: Optional[Callable[[Any], str]] = None):
        super().__init__(rect)
        self.options = list(options)
        self.value = value if value in self.options else (self.options[0] if self.options else None)
        self.on_change = on_change
        self.scale = scale
        self.fmt = fmt or (lambda v: str(v).replace("_", " "))

    def set_options(self, options: Sequence[Any], value: Any = None) -> None:
        self.options = list(options)
        if value in self.options:
            self.value = value
        elif self.value not in self.options:
            self.value = self.options[0] if self.options else None

    def step(self, delta: int) -> None:
        if not self.options:
            return
        i = self.options.index(self.value) if self.value in self.options else 0
        self.value = self.options[(i + delta) % len(self.options)]
        if self.on_change:
            self.on_change(self.value)

    def handle(self, event, app) -> bool:
        if event.type == pygame.MOUSEBUTTONDOWN and self.rect.collidepoint(event.pos) and self.enabled:
            if event.button == 1:
                if len(self.options) > 6 and self.rect.x + 22 < event.pos[0] < self.rect.right - 22:
                    app.pick_list("Choose", [(self.fmt(o), o) for o in self.options], self._picked)
                else:
                    self.step(-1 if event.pos[0] < self.rect.centerx else 1)
            elif event.button == 3:
                self.step(-1)
            return True
        if event.type == pygame.MOUSEWHEEL and self.rect.collidepoint(app.mouse) and self.enabled:
            self.step(-event.y)
            return True
        return False

    def _picked(self, value: Any) -> None:
        if value is not None:
            self.value = value
            if self.on_change:
                self.on_change(value)

    def draw(self, surf, app) -> None:
        f = font(self.scale)
        hover = self.rect.collidepoint(app.mouse)
        pygame.draw.rect(surf, P.BLACK, self.rect)
        pygame.draw.rect(surf, BORDER if hover else P.CYAN[0], self.rect.inflate(-2, -2))
        pygame.draw.rect(surf, PANEL, self.rect.inflate(-4, -4))
        y = self.rect.y + (self.rect.h - f.line_height) // 2 + 1
        f.draw(surf, "◀", (self.rect.x + 6, y), ACCENT)
        f.draw(surf, "▶", (self.rect.right - 6, y), ACCENT, align="right")
        f.draw(surf, self.fmt(self.value) if self.value is not None else "-", (self.rect.centerx, y), TEXT,
               align="center")


class Spinner(Widget):
    """Number with - / + buttons."""

    def __init__(self, rect, value: int = 0, lo: int = 0, hi: int = 10,
                 on_change: Optional[Callable[[int], None]] = None, scale: int = 2):
        super().__init__(rect)
        self.value = int(value)
        self.lo, self.hi = lo, hi
        self.on_change = on_change
        self.scale = scale

    def handle(self, event, app) -> bool:
        delta = 0
        if event.type == pygame.MOUSEBUTTONDOWN and self.rect.collidepoint(event.pos):
            if event.button == 1:
                delta = -1 if event.pos[0] < self.rect.centerx else 1
            elif event.button == 3:
                delta = -1
        elif event.type == pygame.MOUSEWHEEL and self.rect.collidepoint(app.mouse):
            delta = event.y
        if delta:
            new = max(self.lo, min(self.hi, self.value + delta))
            if new != self.value:
                self.value = new
                if self.on_change:
                    self.on_change(new)
            return True
        return False

    def draw(self, surf, app) -> None:
        f = font(self.scale)
        pygame.draw.rect(surf, P.BLACK, self.rect)
        pygame.draw.rect(surf, P.CYAN[0], self.rect.inflate(-2, -2))
        pygame.draw.rect(surf, PANEL, self.rect.inflate(-4, -4))
        y = self.rect.y + (self.rect.h - f.line_height) // 2 + 1
        f.draw(surf, "-", (self.rect.x + 7, y), ACCENT)
        f.draw(surf, "+", (self.rect.right - 7, y), ACCENT, align="right")
        f.draw(surf, str(self.value), (self.rect.centerx, y), TEXT, align="center")


# ---------------------------------------------------------------------------
# Modal dialogs
# ---------------------------------------------------------------------------
class Modal:
    """Base class: a centred window that swallows all input until closed."""

    def __init__(self, app, title: str, w: int, h: int):
        self.app = app
        self.title = title
        sw, sh = app.screen.get_size()
        self.rect = pygame.Rect((sw - w) // 2, (sh - h) // 2, w, h)
        self.widgets: List[Widget] = []

    def close(self) -> None:
        self.app.close_modal(self)

    def handle(self, event) -> None:
        if event.type == pygame.KEYDOWN and event.key == pygame.K_ESCAPE and self.app.focused is None:
            self.cancel()
            return
        for w in reversed(self.widgets):
            if w.visible and w.handle(event, self.app):
                return

    def cancel(self) -> None:
        self.close()

    def draw(self, surf) -> None:
        veil = pygame.Surface(surf.get_size(), pygame.SRCALPHA)
        veil.fill((0, 0, 0, 150))
        surf.blit(veil, (0, 0))
        pygame.draw.rect(surf, P.BLACK, self.rect.inflate(4, 4))
        pygame.draw.rect(surf, P.CYAN[2], self.rect.inflate(2, 2))
        pygame.draw.rect(surf, PANEL, self.rect)
        pygame.draw.rect(surf, P.NAVY[0], (self.rect.x, self.rect.y, self.rect.w, 26))
        font().draw(surf, self.title, (self.rect.x + 10, self.rect.y + 4), ACCENT)
        for w in self.widgets:
            if w.visible:
                w.draw(surf, self.app)


class MessageBox(Modal):
    def __init__(self, app, title: str, text: str, buttons: Sequence[Tuple[str, Optional[Callable[[], None]]]] = (),
                 width: int = 520):
        f = font()
        self.lines = []
        for para in text.split("\n"):
            self.lines += f.wrap(para.replace("<", "<<"), width - 30)
        h = 44 + len(self.lines) * 20 + 46
        super().__init__(app, title, width, h)
        buttons = list(buttons) or [("OK", None)]
        bw = 110
        x = self.rect.right - (bw + 8) * len(buttons) - 4
        for label, fn in buttons:
            self.widgets.append(Button((x, self.rect.bottom - 38, bw, 28), label, self._wrap(fn)))
            x += bw + 8
        self._default = buttons[0][1]

    def _wrap(self, fn):
        def run():
            self.close()
            if fn:
                fn()
        return run

    def handle(self, event) -> None:
        if event.type == pygame.KEYDOWN and event.key in (pygame.K_RETURN, pygame.K_KP_ENTER):
            self._wrap(self._default)()
            return
        super().handle(event)

    def draw(self, surf) -> None:
        super().draw(surf)
        y = self.rect.y + 36
        for line in self.lines:
            font().draw(surf, line, (self.rect.x + 14, y), TEXT)
            y += 20


class PromptBox(Modal):
    """Ask for one or more text values.  ``fields`` is ``[(label, initial)]``;
    the callback receives the list of entered strings."""

    def __init__(self, app, title: str, fields: Sequence[Tuple[str, str]], callback: Callable[[List[str]], None],
                 width: int = 560):
        super().__init__(app, title, width, 50 + len(fields) * 34 + 48)
        self.callback = callback
        self.inputs: List[TextInput] = []
        y = self.rect.y + 38
        label_w = max(font().width(label) for label, _ in fields) + 16 if fields else 0
        for label, initial in fields:
            self.widgets.append(Label((self.rect.x + 12, y, label_w, 28), label, DIM))
            ti = TextInput((self.rect.x + 12 + label_w, y, width - 24 - label_w, 28), initial)
            self.widgets.append(ti)
            self.inputs.append(ti)
            y += 34
        self.widgets.append(Button((self.rect.right - 236, self.rect.bottom - 38, 110, 28), "OK", self.accept))
        self.widgets.append(Button((self.rect.right - 118, self.rect.bottom - 38, 110, 28), "Cancel", self.cancel))
        self.initial_focus = self.inputs[0] if self.inputs else None

    def accept(self) -> None:
        self.app.focus(None)
        values = [ti.value for ti in self.inputs]
        self.close()
        self.callback(values)

    def cancel(self) -> None:
        self.app.focus(None)
        self.close()

    def handle(self, event) -> None:
        if event.type == pygame.KEYDOWN:
            if event.key in (pygame.K_RETURN, pygame.K_KP_ENTER):
                self.accept()
                return
            if event.key == pygame.K_ESCAPE:
                self.cancel()
                return
            if event.key == pygame.K_TAB and self.inputs:
                cur = self.inputs.index(self.app.focused) if self.app.focused in self.inputs else -1
                self.app.focus(self.inputs[(cur + 1) % len(self.inputs)])
                return
        super().handle(event)


class ListPicker(Modal):
    def __init__(self, app, title: str, items: Sequence[Tuple[str, Any]], callback: Callable[[Any], None],
                 width: int = 460, height: int = 440):
        super().__init__(app, title, width, height)
        self.callback = callback
        self.list = ListBox((self.rect.x + 10, self.rect.y + 34, width - 20, height - 84), items, self._picked)
        self.widgets.append(self.list)
        self.widgets.append(Button((self.rect.right - 118, self.rect.bottom - 38, 110, 28), "Cancel", self.cancel))

    def _picked(self, data: Any) -> None:
        self.close()
        self.callback(data)

    def cancel(self) -> None:
        self.close()
        self.callback(None)


# ---------------------------------------------------------------------------
# Application shell
# ---------------------------------------------------------------------------
class ToolApp:
    title = "Tool"
    size = (1280, 760)
    min_size = (1000, 640)

    def __init__(self, headless: bool = False):
        if headless:
            os.environ["SDL_VIDEODRIVER"] = "dummy"
            os.environ["SDL_AUDIODRIVER"] = "dummy"
        os.environ.setdefault("PYGAME_HIDE_SUPPORT_PROMPT", "1")
        pygame.init()
        pygame.key.set_repeat(350, 35)
        self.headless = headless
        self.screen = pygame.display.set_mode(self.size, pygame.RESIZABLE)
        pygame.display.set_caption(self.title)
        self.clock = pygame.time.Clock()
        self.widgets: List[Widget] = []
        self.modals: List[Modal] = []
        self.focused: Optional[TextInput] = None
        self.mouse = (0, 0)
        self.time = 0.0
        self.running = True
        self.status = ""
        self._status_time = 0.0

    # -- helpers -------------------------------------------------------------------
    def set_status(self, text: str, seconds: float = 4.0) -> None:
        self.status = text
        self._status_time = seconds

    def focus(self, widget: Optional[TextInput]) -> None:
        if self.focused is widget:
            return
        if self.focused is not None:
            self.focused.on_blur()
        self.focused = widget
        if widget is not None:
            widget.focused = True
            widget._orig = widget.value

    def open_modal(self, modal: Modal) -> None:
        self.modals.append(modal)
        self.focus(getattr(modal, "initial_focus", None))

    def close_modal(self, modal: Modal) -> None:
        if modal in self.modals:
            self.modals.remove(modal)

    def message(self, title: str, text: str, buttons=()) -> None:
        self.open_modal(MessageBox(self, title, text, buttons))

    def confirm(self, title: str, text: str, on_yes: Callable[[], None]) -> None:
        self.open_modal(MessageBox(self, title, text, [("Yes", on_yes), ("No", None)]))

    def prompt(self, title: str, fields: Sequence[Tuple[str, str]], callback: Callable[[List[str]], None]) -> None:
        self.open_modal(PromptBox(self, title, fields, callback))

    def pick_list(self, title: str, items: Sequence[Tuple[str, Any]], callback: Callable[[Any], None]) -> None:
        self.open_modal(ListPicker(self, title, items, callback))

    # -- to override ---------------------------------------------------------------
    def layout(self) -> None:
        """Position widgets for the current window size."""

    def on_event(self, event) -> bool:
        """Handle an event no widget wanted.  Return True if consumed."""
        return False

    def update(self, dt: float) -> None:
        pass

    def draw(self) -> None:
        self.screen.fill(BG)

    def can_quit(self) -> bool:
        return True

    # -- loop ----------------------------------------------------------------------
    def process(self, events) -> None:
        for event in events:
            if event.type == pygame.QUIT:
                if self.can_quit():
                    self.running = False
                continue
            if event.type == pygame.VIDEORESIZE:
                w, h = max(self.min_size[0], event.w), max(self.min_size[1], event.h)
                self.screen = pygame.display.set_mode((w, h), pygame.RESIZABLE)
                self.layout()
                continue
            if event.type in (pygame.MOUSEMOTION, pygame.MOUSEBUTTONDOWN, pygame.MOUSEBUTTONUP):
                self.mouse = event.pos
            if self.modals:
                self.modals[-1].handle(event)
                continue
            if self.focused is not None and event.type in (pygame.KEYDOWN, pygame.TEXTINPUT):
                self.focused.handle(event, self)
                continue
            consumed = False
            for w in reversed(self.widgets):
                if w.visible and w.handle(event, self):
                    consumed = True
                    break
            if not consumed:
                if event.type == pygame.MOUSEBUTTONDOWN and self.focused is not None:
                    self.focus(None)
                self.on_event(event)

    def frame(self, dt: float, events=()) -> None:
        self.time += dt
        if self._status_time > 0:
            self._status_time -= dt
            if self._status_time <= 0:
                self.status = ""
        self.process(events)
        for w in self.widgets:
            w.update(dt)
        self.update(dt)
        self.draw()
        for w in self.widgets:
            if w.visible:
                w.draw(self.screen, self)
        for modal in self.modals:
            modal.draw(self.screen)
        pygame.display.flip()

    def run(self) -> None:
        self.layout()
        while self.running:
            dt = min(0.1, self.clock.tick(60) / 1000.0)
            if not self.headless:
                self.mouse = pygame.mouse.get_pos()
            self.frame(dt, pygame.event.get())
        pygame.quit()

    # -- synthetic input for tests ----------------------------------------------------
    def click(self, pos, button: int = 1) -> None:
        self.mouse = tuple(pos)
        self.frame(0.016, [pygame.event.Event(pygame.MOUSEBUTTONDOWN, pos=tuple(pos), button=button)])
        self.frame(0.016, [pygame.event.Event(pygame.MOUSEBUTTONUP, pos=tuple(pos), button=button)])

    def drag(self, start, end, button: int = 1, steps: int = 4) -> None:
        self.mouse = tuple(start)
        self.frame(0.016, [pygame.event.Event(pygame.MOUSEBUTTONDOWN, pos=tuple(start), button=button)])
        for i in range(1, steps + 1):
            pos = (start[0] + (end[0] - start[0]) * i // steps, start[1] + (end[1] - start[1]) * i // steps)
            self.mouse = pos
            self.frame(0.016, [pygame.event.Event(pygame.MOUSEMOTION, pos=pos, rel=(0, 0),
                                                  buttons=(1 if button == 1 else 0, 0, 1 if button == 3 else 0))])
        self.frame(0.016, [pygame.event.Event(pygame.MOUSEBUTTONUP, pos=tuple(end), button=button)])

    def press(self, key: int, mod: int = 0, text: str = "") -> None:
        events = [pygame.event.Event(pygame.KEYDOWN, key=key, mod=mod, unicode=text)]
        if text:
            events.append(pygame.event.Event(pygame.TEXTINPUT, text=text))
        self.frame(0.016, events)
        self.frame(0.016, [pygame.event.Event(pygame.KEYUP, key=key, mod=mod)])

    def type_text(self, text: str) -> None:
        self.frame(0.016, [pygame.event.Event(pygame.TEXTINPUT, text=text)])


def parse_value(text: str) -> Any:
    """Turn what the user typed into a JSON value where that makes sense:
    ``12`` -> 12, ``true`` -> True, ``{"a": 1}`` -> dict, anything else stays text."""
    stripped = text.strip()
    if stripped == "":
        return ""
    try:
        return json.loads(stripped)
    except ValueError:
        pass
    low = stripped.lower()
    if low in ("true", "yes"):
        return True
    if low in ("false", "no"):
        return False
    return text


def format_value(value: Any) -> str:
    if isinstance(value, str):
        return value
    return json.dumps(value)
