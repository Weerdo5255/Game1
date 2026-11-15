"""Character / portrait / icon contact sheets for ``artdump``."""
from __future__ import annotations

import os


def dump(out: str, scale: int = 4) -> None:
    import pygame
    from game.art import chars
    from game.engine import palette
    art = chars.CharArt()
    samples = [
        {"hair": "short", "hair_color": "black", "skin": "tan", "outfit": "uniform", "primary": "navy", "secondary": "amber"},
        {"hair": "long", "hair_color": "auburn", "skin": "fair", "outfit": "labcoat", "primary": "teal", "secondary": "cyan", "extras": ["glasses"]},
        {"hair": "buzz", "hair_color": "grey", "skin": "brown", "outfit": "jumpsuit", "primary": "orange", "secondary": "black", "extras": ["beard"]},
        {"hair": "bob", "hair_color": "blue", "skin": "porcelain", "outfit": "uniform", "primary": "green", "secondary": "tan", "legs": "brown"},
        {"hair": "ponytail", "hair_color": "blonde", "skin": "olive", "outfit": "vest", "primary": "slate", "secondary": "red", "legs": "black"},
        {"hair": "spiky", "hair_color": "magenta", "skin": "umber", "outfit": "jumpsuit", "primary": "purple", "secondary": "cyan"},
        {"hair": "cap", "hair_color": "brown", "skin": "ebony", "outfit": "uniform", "primary": "amber", "secondary": "steel", "legs": "navy", "extras": ["beard"]},
        {"hair": "helmet", "hair_color": "black", "skin": "tan", "outfit": "suit", "primary": "white", "secondary": "orange"},
        {"hair": "bald", "hair_color": "black", "skin": "ashen", "outfit": "labcoat", "primary": "red", "secondary": "black", "extras": ["glasses", "beard"]},
        {"body": "drone", "primary": "steel", "secondary": "cyan"},
    ]
    cols = len(samples)
    sheet = pygame.Surface((cols * 70 + 4, 4 * 26 + 44))
    sheet.fill(palette.STEEL[2])
    for i, a in enumerate(samples):
        x0 = 4 + i * 70
        for r, d in enumerate(("down", "left", "right", "up")):
            for f in range(4):
                sheet.blit(art.sprite(a, d, f), (x0 + f * 17, 2 + r * 26))
        sheet.blit(art.portrait(a), (x0, 4 * 26 + 6))
        sheet.blit(art.sleeping(a), (x0 + 40, 4 * 26 + 10))
    pygame.image.save(pygame.transform.scale(sheet, (sheet.get_width() * scale, sheet.get_height() * scale)),
                      os.path.join(out, "characters.png"))
    try:
        from game.art import icons
    except ImportError:
        return
    iart = icons.IconArt()
    names = list(icons.ICONS)
    sheet = pygame.Surface((len(names) * 18 + 4, 22))
    sheet.fill(palette.NAVY[1])
    for i, name in enumerate(names):
        sheet.blit(iart.get(name), (4 + i * 18, 3))
    pygame.image.save(pygame.transform.scale(sheet, (sheet.get_width() * scale, sheet.get_height() * scale)),
                      os.path.join(out, "icons.png"))
