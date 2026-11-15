"""Item definitions and inventories.

Items are defined in ``data/items.json``::

    "multitool": {
      "name": "Multitool", "desc": "...", "kind": "tool",
      "weight": 0.8, "value": 60, "icon": "tool", "color": "amber",
      "slot": "tool", "mods": {"skill.engineering": 1},
      "tags": ["tool"]
    },
    "ration_pack": {
      "name": "Ration Pack", "kind": "consumable", "weight": 0.4, "value": 8,
      "use": ["add_need('hunger', -45)"], "use_text": "You eat the ration.",
      "tags": ["food"]
    }

``mods`` are stat modifiers applied while the item is equipped (see
``character.Character.mods``).  ``use`` is a list of script statements run
when the item is consumed (see ``story/script.py``).  ``tags`` are free-form
labels that the simulation looks for: ``food`` (NPCs eat it), ``part``
(consumed by repairs), ``fuel``, ``access:<group>`` (opens doors) and so on.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, Iterator, List, Optional, Tuple

KINDS = ["consumable", "tool", "equipment", "key", "material", "data", "quest", "misc"]
SLOTS = ["tool", "suit", "accessory"]
SLOT_LABELS = {"tool": "Tool", "suit": "Suit", "accessory": "Accessory"}


@dataclass
class ItemDef:
    id: str
    name: str
    desc: str = ""
    kind: str = "misc"
    weight: float = 0.1
    value: int = 0
    icon: str = "generic"
    color: str = "steel"
    slot: str = ""
    mods: Dict[str, float] = field(default_factory=dict)
    use: List[str] = field(default_factory=list)
    use_text: str = ""
    tags: List[str] = field(default_factory=list)
    quest: bool = False          # cannot be dropped or sold

    @property
    def usable(self) -> bool:
        return bool(self.use)

    @property
    def equippable(self) -> bool:
        return bool(self.slot)

    def has_tag(self, tag: str) -> bool:
        return tag in self.tags

    def access_groups(self) -> List[str]:
        return [t.split(":", 1)[1] for t in self.tags if t.startswith("access:")]

    @classmethod
    def from_dict(cls, item_id: str, data: dict) -> "ItemDef":
        return cls(
            id=item_id,
            name=str(data.get("name", item_id)),
            desc=str(data.get("desc", "")),
            kind=str(data.get("kind", "misc")),
            weight=float(data.get("weight", 0.1)),
            value=int(data.get("value", 0)),
            icon=str(data.get("icon", "generic")),
            color=str(data.get("color", "steel")),
            slot=str(data.get("slot", "")),
            mods=dict(data.get("mods") or {}),
            use=list(data.get("use") or []),
            use_text=str(data.get("use_text", "")),
            tags=list(data.get("tags") or []),
            quest=bool(data.get("quest", False)),
        )


class Inventory:
    """A bag of stackable items: ``{item_id: quantity}``."""

    def __init__(self, items: Optional[Dict[str, int]] = None):
        self.items: Dict[str, int] = {}
        if items:
            for item_id, qty in items.items():
                if int(qty) > 0:
                    self.items[str(item_id)] = int(qty)

    def count(self, item_id: str) -> int:
        return self.items.get(item_id, 0)

    def has(self, item_id: str, qty: int = 1) -> bool:
        return self.items.get(item_id, 0) >= qty

    def add(self, item_id: str, qty: int = 1) -> None:
        if qty > 0:
            self.items[item_id] = self.items.get(item_id, 0) + int(qty)

    def remove(self, item_id: str, qty: int = 1) -> int:
        """Remove up to ``qty``; returns how many were actually removed."""
        have = self.items.get(item_id, 0)
        taken = min(have, max(0, int(qty)))
        if taken:
            if have - taken <= 0:
                del self.items[item_id]
            else:
                self.items[item_id] = have - taken
        return taken

    def transfer(self, other: "Inventory", item_id: str, qty: int = 1) -> int:
        moved = self.remove(item_id, qty)
        other.add(item_id, moved)
        return moved

    def entries(self) -> List[Tuple[str, int]]:
        return sorted(self.items.items())

    def __iter__(self) -> Iterator[str]:
        return iter(sorted(self.items))

    def __len__(self) -> int:
        return len(self.items)

    def __bool__(self) -> bool:
        return True

    @property
    def empty(self) -> bool:
        return not self.items

    def total(self) -> int:
        return sum(self.items.values())

    # -- queries that need the item database -----------------------------------
    def weight(self, db: Dict[str, ItemDef]) -> float:
        return sum(db[i].weight * q for i, q in self.items.items() if i in db)

    def find_tag(self, db: Dict[str, ItemDef], tag: str) -> Optional[str]:
        """First item id carrying ``tag`` (alphabetical for determinism)."""
        for item_id in sorted(self.items):
            idef = db.get(item_id)
            if idef is not None and tag in idef.tags:
                return item_id
        return None

    def count_tag(self, db: Dict[str, ItemDef], tag: str) -> int:
        return sum(q for i, q in self.items.items() if i in db and tag in db[i].tags)

    def to_dict(self) -> Dict[str, int]:
        return dict(self.items)

    @classmethod
    def from_dict(cls, data: Optional[dict]) -> "Inventory":
        return cls(data or {})

    def __repr__(self) -> str:
        return "Inventory(%r)" % (self.items,)


def parse_item_list(text: str) -> Dict[str, int]:
    """Parse ``"ration_pack:3, multitool"`` into ``{"ration_pack": 3, "multitool": 1}``."""
    out: Dict[str, int] = {}
    for part in str(text or "").replace(";", ",").split(","):
        part = part.strip()
        if not part:
            continue
        if ":" in part:
            name, qty = part.split(":", 1)
            try:
                n = int(qty.strip())
            except ValueError:
                n = 1
        else:
            name, n = part, 1
        name = name.strip()
        if name and n > 0:
            out[name] = out.get(name, 0) + n
    return out


def format_item_list(items: Dict[str, int]) -> str:
    return ", ".join("%s:%d" % (k, v) if v != 1 else k for k, v in sorted(items.items()))
