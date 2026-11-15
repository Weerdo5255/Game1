"""Global constants and paths.

Nothing in here depends on pygame so that the simulation and the tests can
import it freely.  Paths can be redirected with environment variables, which
is how the test-suite points the engine at temporary directories:

``GAME_DATA_DIR``  content directory (maps, characters, dialogue ...)
``GAME_SAVE_DIR``  where save slots and options are written
"""
from __future__ import annotations

import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def data_dir() -> str:
    return os.environ.get("GAME_DATA_DIR") or os.path.join(ROOT, "data")


def save_dir() -> str:
    return os.environ.get("GAME_SAVE_DIR") or os.path.join(ROOT, "saves")


def data_path(*parts: str) -> str:
    return os.path.join(data_dir(), *parts)


# --- presentation -----------------------------------------------------------
# The game renders to a small virtual framebuffer that is scaled up with
# nearest-neighbour filtering, which is what gives it the chunky 16-bit look.
VIEW_W = 384
VIEW_H = 216
TILE = 16
FPS = 60
DEFAULT_SCALE = 3

# Character sprites are 16 wide and 24 tall; the feet sit on the tile.
CHAR_W = 16
CHAR_H = 24

# --- simulation -------------------------------------------------------------
# One real second of play advances the station clock by this many game
# minutes.  ``data/game.json`` can override it ("time_scale").
TIME_SCALE = 1.0
MINUTES_PER_HOUR = 60
HOURS_PER_DAY = 24
MINUTES_PER_DAY = MINUTES_PER_HOUR * HOURS_PER_DAY

# Movement speeds in tiles per real second.
WALK_SPEED = 4.5
RUN_SPEED = 7.5
NPC_SPEED = 3.2

# Largest slice of time the simulation will swallow in one step.  Longer
# stretches (sleeping, waiting, loading screens) are cut into slices this size
# so off-screen NPCs and station systems behave exactly as they would in
# real time.
MAX_SIM_STEP = 0.25

SAVE_VERSION = 1

# Tile layers, bottom to top.  Characters are drawn between "objects" and
# "overhead".
LAYERS = ("ground", "decor", "objects", "overhead")

DIRS = {
    "up": (0, -1),
    "down": (0, 1),
    "left": (-1, 0),
    "right": (1, 0),
}
OPPOSITE = {"up": "down", "down": "up", "left": "right", "right": "left"}


def dir_from_delta(dx: int, dy: int, default: str = "down") -> str:
    """Facing name for a movement delta (dominant axis wins)."""
    if dx == 0 and dy == 0:
        return default
    if abs(dx) > abs(dy):
        return "right" if dx > 0 else "left"
    return "down" if dy > 0 else "up"
