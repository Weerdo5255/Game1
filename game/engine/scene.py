"""Scene base class.

The application keeps a stack of scenes.  Only the top scene receives input
and updates; scenes flagged ``transparent`` let the one underneath keep
drawing (dialogue boxes and menus over the game world).
"""
from __future__ import annotations


class Scene:
    transparent = False   # draw the scene below first
    name = "scene"

    def __init__(self, app):
        self.app = app

    # lifecycle ---------------------------------------------------------------
    def enter(self) -> None:
        """Called when the scene is pushed onto the stack."""

    def exit(self) -> None:
        """Called when the scene is removed from the stack."""

    def resume(self) -> None:
        """Called when the scene above this one is popped."""

    # per frame ---------------------------------------------------------------
    def update(self, dt: float) -> None:
        pass

    def draw(self, surf) -> None:
        pass

    # helpers -----------------------------------------------------------------
    @property
    def input(self):
        return self.app.input

    def close(self) -> None:
        """Pop this scene (no-op if it is not on the stack any more)."""
        self.app.pop_scene(self)
