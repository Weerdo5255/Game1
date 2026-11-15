"""Tethys Deep - a branching, simulation-driven sci-fi story game framework.

Package layout
--------------
game.engine   window, scenes, input, bitmap font, UI widgets, audio
game.art      procedural 16-bit style pixel art (tiles, characters, icons)
game.world    maps, pathfinding, clock, characters, items, the World container
game.sim      station systems, needs, NPC goal AI, events
game.story    safe script language, dialogue, quests, story director
game.scenes   title screen, overworld, menus, dialogue UI, endings
game.tools    map editor, character creator, validator, headless simulator

Everything under ``game.world``, ``game.sim`` and ``game.story`` is pure
logic and never imports pygame, so the whole simulation can run headless
(tests, the ``simulate`` tool, fast-forwarding while the player sleeps).
"""

__version__ = "0.1.0"
