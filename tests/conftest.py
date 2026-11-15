"""Shared fixtures.  Everything runs headless: no window, no audio."""
import os
import shutil
import sys

os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
os.environ.setdefault("PYGAME_HIDE_SUPPORT_PROMPT", "1")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

import pytest  # noqa: E402


@pytest.fixture(autouse=True)
def _isolated_saves(tmp_path, monkeypatch):
    """Never touch the real saves directory."""
    monkeypatch.setenv("GAME_SAVE_DIR", str(tmp_path / "saves"))


@pytest.fixture(scope="session")
def content():
    """The shipped campaign, loaded once (treat it as read-only)."""
    from game.content import Content
    return Content.load(os.path.join(ROOT, "data"))


@pytest.fixture
def world(content):
    """A fresh game with a fixed seed and no pending UI requests."""
    from game.world.world import World
    w = World.new_game(content, {"name": "Tester", "background": "engineer"}, seed=7)
    w.requests.clear()
    w.toasts.clear()
    w.script.strict = True
    return w


@pytest.fixture
def data_copy(tmp_path, monkeypatch):
    """A private, writable copy of the data directory (for editor tests)."""
    dest = tmp_path / "data"
    shutil.copytree(os.path.join(ROOT, "data"), dest)
    monkeypatch.setenv("GAME_DATA_DIR", str(dest))
    return str(dest)
