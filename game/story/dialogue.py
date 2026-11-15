"""Branching dialogue runtime.

A dialogue file (``data/dialogue/<id>.json``) is a graph of nodes::

    {
      "start": [{"if": "not met()", "goto": "intro"}, {"goto": "hub"}],
      "nodes": {
        "intro": {
          "text": ["First page.", "Second page, with a {skill('tech')} in it."],
          "do": ["set_flag('met_lin')"],
          "next": "hub"
        },
        "hub": {
          "variants": [{"if": "cond('lab') < 40", "text": "The lab is dying."},
                       {"text": "What is it?"}],
          "choices": [
            {"text": "What are you working on?", "goto": "work"},
            {"text": "That waveform isn't natural.", "skill": ["science", 4], "goto": "insight"},
            {"text": "Let me help you.", "check": ["persuasion", 12], "pass": "yes", "fail": "no"},
            {"text": "Here, take these.", "item": ["spare_parts", 2], "consume": true, "goto": "thanks"},
            {"text": "Goodbye.", "goto": "end"}
          ]
        }
      }
    }

Node keys: ``speaker`` (default: the NPC; "player", "narrator" or any
character id), ``text`` (string or list of pages), ``variants`` (first
matching ``if`` wins), ``do`` (effects run on entering), ``branch`` (list of
``{"if", "goto"}`` redirects checked before the text is shown), ``choices``
and ``next``.

Choice keys: ``text``, ``if`` (hidden when false), ``skill`` / ``attr``
(``[name, minimum]``: shown but disabled when unmet), ``check``
(``[skill, difficulty]``: a d20 roll, continue at ``pass`` or ``fail``),
``item`` (``[item, count]`` required; removed if ``consume`` is true),
``cost`` (credits), ``once``, ``do``, ``pass_do`` / ``fail_do`` and ``goto``.

This module is pure logic; ``scenes/dialogue.py`` draws it.
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, List, Optional

from ..world.character import ATTR_LABELS, SKILL_LABELS


@dataclass
class Choice:
    index: int              # position in the node's raw choice list
    text: str
    enabled: bool = True
    tag: str = ""           # e.g. "Tech 4" or "Persuasion 65%"
    tag_ok: bool = True     # colour hint for the tag


class DialogueRunner:
    def __init__(self, world, dialogue_id: str, npc_id: str = "", obj_id: str = "",
                 node: Optional[str] = None):
        self.world = world
        self.id = dialogue_id
        self.data: Dict[str, Any] = world.content.dialogues.get(dialogue_id) or {"nodes": {}}
        self.nodes: Dict[str, dict] = self.data.get("nodes") or {}
        self.npc_id = npc_id
        self.ctx = {"npc": npc_id, "self": npc_id, "obj": obj_id}
        self.node_id = ""
        self.node: Dict[str, Any] = {}
        self.pages: List[str] = []
        self.page = 0
        self.finished = False
        self.last_check: Optional[Dict[str, Any]] = None   # result of the latest skill check
        self._choices: Optional[List[Choice]] = None
        self._start_node = node

    # -- flow ----------------------------------------------------------------------
    def start(self) -> "DialogueRunner":
        world = self.world
        npc = world.characters.get(self.npc_id) if self.npc_id else None
        if npc is not None:
            world.events.emit("talk:" + npc.id, npc=npc.id)
        target = self._start_node or self._entry()
        self._goto(target)
        if npc is not None:
            npc.met = True
            key = "talked:" + npc.id
            world.flags[key] = int(world.flags.get(key, 0)) + 1
        return self

    def _entry(self) -> str:
        start = self.data.get("start", "start")
        if isinstance(start, str):
            return start
        for option in start:
            if self.world.script.eval(option.get("if"), self.ctx):
                return str(option.get("goto", "end"))
        return "end"

    def _finish(self) -> None:
        if not self.finished:
            self.finished = True
            if self.npc_id:
                self.world.events.emit("talk_end:" + self.npc_id, npc=self.npc_id)
            self.world.events.pump()

    def _goto(self, target: Optional[str]) -> None:
        script = self.world.script
        for _ in range(64):                      # guard against redirect loops
            if not target or target == "end":
                self._finish()
                return
            node = self.nodes.get(target)
            if node is None:
                script.errors.append("dialogue %s: missing node %r" % (self.id, target))
                self._finish()
                return
            self.node_id, self.node = target, node
            redirect = None
            for option in node.get("branch") or ():
                if script.eval(option.get("if"), self.ctx):
                    redirect = option.get("goto", "end")
                    break
            if redirect is not None:
                target = redirect
                continue
            script.run(node.get("do"), self.ctx)
            if self.world.ended:
                self._finish()
                return
            pages = self._pages(node)
            if not pages:
                if node.get("choices") and self._visible_choices(node):
                    pages = [""]
                else:
                    target = node.get("next", "end")
                    continue
            self.pages = pages
            self.page = 0
            self._choices = None
            return
        script.errors.append("dialogue %s: redirect loop at %r" % (self.id, target))
        self._finish()

    def _pages(self, node: dict) -> List[str]:
        text = node.get("text")
        for variant in node.get("variants") or ():
            if self.world.script.eval(variant.get("if"), self.ctx):
                text = variant.get("text")
                break
        if text is None or text == "" or text == []:
            return []
        if isinstance(text, str):
            text = [text]
        return [self.world.text(str(page), self.ctx) for page in text]

    # -- choices -------------------------------------------------------------------
    def _once_key(self, index: int) -> str:
        return "_dlg:%s:%s:%d" % (self.id, self.node_id, index)

    def _visible_choices(self, node: dict) -> List[Choice]:
        world = self.world
        player = world.player
        out: List[Choice] = []
        for i, raw in enumerate(node.get("choices") or ()):
            if raw.get("once") and world.flags.get("_dlg:%s:%s:%d" % (self.id, self.node_id, i)):
                continue
            if not world.script.eval(raw.get("if"), self.ctx):
                continue
            enabled, tag, ok = True, "", True
            if "skill" in raw:
                name, need = raw["skill"][0], int(raw["skill"][1])
                ok = player.skill(name) >= need
                enabled = enabled and ok
                tag = "%s %d" % (SKILL_LABELS.get(name, name), need)
            elif "attr" in raw:
                name, need = raw["attr"][0], int(raw["attr"][1])
                ok = player.attr(name) >= need
                enabled = enabled and ok
                tag = "%s %d" % (ATTR_LABELS.get(name, name), need)
            elif "check" in raw:
                name, dc = raw["check"][0], int(raw["check"][1])
                tag = "%s %d%%" % (SKILL_LABELS.get(name, name), world.check_chance(player, name, dc))
            if "item" in raw:
                item_id, count = raw["item"][0], int(raw["item"][1]) if len(raw["item"]) > 1 else 1
                have = player.inventory.has(item_id, count)
                enabled = enabled and have
                ok = ok and have
                label = world.item_name(item_id) + (" x%d" % count if count > 1 else "")
                tag = (tag + ", " if tag else "") + label
            if "cost" in raw:
                cost = int(raw["cost"])
                afford = player.credits >= cost
                enabled = enabled and afford
                ok = ok and afford
                tag = (tag + ", " if tag else "") + "%d cr" % cost
            out.append(Choice(i, world.text(str(raw.get("text", "...")), self.ctx), enabled, tag, ok))
        return out

    @property
    def on_last_page(self) -> bool:
        return self.page >= len(self.pages) - 1

    def choices(self) -> List[Choice]:
        """Choices offered right now (empty until the last page is shown)."""
        if self.finished or not self.on_last_page:
            return []
        if self._choices is None:
            self._choices = self._visible_choices(self.node)
        return self._choices

    def advance(self) -> None:
        """Next page, or follow ``next`` when the node has no choices."""
        if self.finished:
            return
        if not self.on_last_page:
            self.page += 1
            return
        if self.choices():
            return
        self._goto(self.node.get("next", "end"))

    def choose(self, position: int) -> bool:
        """Pick the choice at ``position`` in :meth:`choices`."""
        options = self.choices()
        if not (0 <= position < len(options)) or not options[position].enabled:
            return False
        world = self.world
        raw = self.node["choices"][options[position].index]
        if raw.get("once"):
            world.flags[self._once_key(options[position].index)] = True
        self.last_check = None
        target = raw.get("goto", "end")
        if "item" in raw and raw.get("consume"):
            count = int(raw["item"][1]) if len(raw["item"]) > 1 else 1
            world.player.inventory.remove(raw["item"][0], count)
            world.player.check_equipment()
        if "cost" in raw:
            world.player.credits -= int(raw["cost"])
        if "check" in raw:
            name, dc = raw["check"][0], int(raw["check"][1])
            success = world.skill_check(world.player, name, dc)
            self.last_check = {"skill": name, "dc": dc, "success": success}
            world.script.run(raw.get("pass_do" if success else "fail_do"), self.ctx)
            target = raw.get("pass" if success else "fail", target)
        world.script.run(raw.get("do"), self.ctx)
        if world.ended:
            self._finish()
            return True
        self._goto(target)
        return True

    # -- presentation ----------------------------------------------------------------
    def speaker(self) -> Dict[str, Any]:
        """Who is talking: {"id", "name", "appearance", "blip"}."""
        who = self.node.get("speaker", "npc")
        world = self.world
        if who == "narrator":
            return {"id": "", "name": "", "appearance": None, "blip": "blip_low"}
        if who == "npc":
            who = self.npc_id
        if who == "player":
            who = world.player.id
        ch = world.characters.get(who)
        if ch is None:
            name = self.node.get("speaker_name") or world.object_name(self.ctx.get("obj", "")) or ""
            return {"id": "", "name": name, "appearance": None, "blip": "blip"}
        name = self.node.get("speaker_name") or (ch.name if (ch.met or ch.is_player) else (ch.title or "Stranger"))
        return {"id": ch.id, "name": name, "appearance": ch.appearance, "blip": ch.blip}

    @property
    def text(self) -> str:
        if self.finished or not self.pages:
            return ""
        return self.pages[self.page]
