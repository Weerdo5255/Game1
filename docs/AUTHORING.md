# Authoring guide

How to build a story with this framework: rooms, people, conversations,
quests, the machines that keep everyone alive, and the rules that tie them
together. Nothing here needs Python - until the last section, which shows how
to extend the engine when data is not enough.

* [Where things live](#where-things-live)
* [A ten-minute tour](#a-ten-minute-tour)
* [The script language](#the-script-language)
* [Maps](#maps)
* [Characters](#characters)
* [Dialogue](#dialogue)
* [Quests](#quests)
* [Story rules and events](#story-rules-and-events)
* [Endings](#endings)
* [Items and effects](#items-and-effects)
* [Station systems](#station-systems)
* [Schedules and goals: what NPCs do all day](#schedules-and-goals-what-npcs-do-all-day)
* [Game settings](#game-settings)
* [Checking your work](#checking-your-work)
* [Extending the engine](#extending-the-engine)

## Where things live

Everything about the story is in `data/`. The engine reads the whole folder
when it starts; nothing is compiled.

| File | Holds |
|---|---|
| `game.json` | title, where and when the game starts, the player, backgrounds |
| `maps/<id>.json` | one area each: tiles, objects, regions (made with the map editor) |
| `characters/<id>.json` | one person each (made with the character creator) |
| `dialogue/<id>.json` | one conversation tree each |
| `quests.json` | quests and their stages |
| `story/rules.json` | "when this happens, do that" rules |
| `story/endings.json` | ending slides |
| `items.json`, `effects.json` | items and status effects |
| `systems.json` | station machines and the resources they share |
| `schedules.json` | named daily routines |
| `goals.json` | reusable goal templates for NPCs |
| `archetypes.json` | role presets used by the character creator |

Ids are lower-case words with underscores (`reactor_console`, `q_water`).
In the dictionary-style files (items, effects, systems, quests, goals,
schedules, archetypes, endings) a top-level key starting with an underscore,
such as `_comment`, is ignored, so you can leave notes.

Run `python -m game.tools.validate` after any change. It reads every file and
reports mistakes with the file and field they are in.

## A ten-minute tour

This adds a room, somebody who works there, a conversation, a project of her
own, a quest and a rule. The test suite performs exactly these steps
(`test_authoring_tour`), so they are known to work.

**1. A room.** Start the editor on a new map:

```
python map_editor.py --new cargo_bay 14 10
```

Paint a wall around the edge (pick *wall* from the palette; walls join up on
their own). Choose the **Object** tool, pick *Container* on the right and click
a tile; in the property list change its `id` to `salvage_crate` and set `items`
to `{"scrap_metal": 6}`. Add a *Portal*, press **Pick target**, choose the hangar
and click where the player should arrive. Open the hangar map and add a portal
back the same way. Save with Ctrl+S; **F5** drops you into the room to try it.

**2. A person.**

```
python character_creator.py --new "Kira Vance" --role technician --map cargo_bay --dialogue
```

That writes `data/characters/kira.json` (stats, a look, a bunk, a job at the
fabricator, a day schedule) and a starter conversation in
`data/dialogue/kira.json`. Start the game and she is there, working her shift,
eating at midday and going to bed at night.

**3. Something to say.** Open `data/dialogue/kira.json` and add a choice to the
`hub` node, plus the node it leads to:

```json
{
  "hub_choice": {"text": "That is a lot of scrap for one person.",
                 "if": "not quest_started('q_salvage')",
                 "do": ["start_quest('q_salvage')"], "goto": "salvage"},
  "salvage": {"text": "Forty years of broken things. The fabricator will eat all of it, two crates a trip.",
              "next": "hub"}
}
```

(The first object goes into the `choices` list of `hub`; the second is a new
entry under `nodes`.)

**4. A project of her own.** In `data/characters/kira.json`:

```json
{
  "goals": [
    {"id": "haul_salvage", "desc": "Haul the salvage to the fabricator", "priority": 46,
     "repeat": true, "cooldown": 60,
     "when": "container_count('salvage_crate', 'scrap_metal') > 0",
     "steps": [
       {"do": "acquire", "item": "scrap_metal", "count": 2, "from": ["salvage_crate"]},
       {"do": "deliver", "item": "scrap_metal", "to": "fab_feed"}
     ],
     "on_complete": ["add_var('salvage_runs', 1)"]}
  ]
}
```

Priority 46 beats her shift (40) but not sleep (60), so she hauls scrap between
other things, two pieces a trip, across three maps - whether or not you are
there to see it. The scrap is real: the fabricator turns it into spare parts,
and the engineer fits those to whatever is failing.

**5. A quest and a rule.** In `data/quests.json`:

```json
{
  "q_salvage": {
    "name": "Waste Not", "xp": 40,
    "desc": "Kira Vance is feeding forty years of scrap to the fabricator.",
    "stages": {"haul": "Three trips should empty the salvage crate."},
    "outcomes": {"done": "The salvage is in the fabricator's feed bin."}
  }
}
```

and in `data/story/rules.json`:

```json
{"id": "salvage_done", "on": "goal_done:kira:haul_salvage", "once": false,
 "if": "quest_active('q_salvage') and var('salvage_runs') >= 3",
 "do": ["complete_quest('q_salvage')", "add_rel(10, 'kira')"]}
```

Run `python -m game.tools.validate`, then play - or watch her do it without you:
`python -m game.tools.simulate --days 1 --log`.

## The script language

Conditions and effects are short Python-looking strings:

```json
{
  "if": "skill('tech') >= 4 and not flag('lab_hacked')",
  "do": ["set_flag('lab_hacked')", "add_xp(40)", "add_rel(-10, 'lin')"]
}
```

* An `if` (or `when`, `while`, `until`, `cond`) field is **one expression**.
  A missing or empty condition counts as true.
* A `do` (or `on_complete`, `on_enter`, `on_interact`, `use`, `script`) field
  is a **list of statements**, run in order. A statement is a function call, or
  a one-line `if`: `"if not quest_started('q_water'): start_quest('q_water')"`.
* Text can embed expressions in braces: `"She is running at {round(cond('reactor'))} percent."`
  `$PLAYER`, `$NPC`, `$TIME` and `$DAY` are shortcuts for the obvious things.
* Text colours: `<y>yellow</y>`, `<r>red</r>`, `<g>green</g>`, `<c>cyan</c>`,
  `<p>purple</p>`, `<b>blue</b>`, `<w>white</w>`, `<d>dim</d>`.

Available: numbers, strings, `True` / `False` / `None`, lists, `and` / `or` /
`not`, comparisons (`==`, `<`, `in` ...), arithmetic, `x if cond else y`,
`min max abs round int float str len bool`, and the functions in
**[SCRIPT_API.md](SCRIPT_API.md)**. Nothing else: no attribute access, no
indexing, no imports. Scripts are parsed and interpreted by
`game/story/script.py`, never passed to `eval`, so a content folder cannot run
arbitrary code.

The names `npc` (the person being talked to, or the owner of a goal), `self`
(the same, inside goals), `obj` (the map object being used), `item`, `map` and
`player` hold ids and can be passed to functions: `give_to(npc, 'ration_pack', 1)`.

**State** lives in three places: *flags and variables* (`set_flag`, `flag`,
`add_var`, `var` - one shared dictionary; setting a flag also raises the event
`flag:<name>`), *quests* (`start_quest`, `stage`, ...), and the world itself
(inventories, relationships, system conditions, map objects). All of it is
saved.

A script error never crashes the game: the statement is skipped and the
message is kept (press F3 in game to see the latest one). The validator finds
most of them before you ever run the game.

## Maps

Maps are JSON written by the map editor. You should not need to edit them by
hand, but the format is simple:

```json
{
  "id": "habitat", "name": "Habitat Ring", "width": 30, "height": 20,
  "props": {"ambient": [210, 215, 235], "backdrop": "space"},
  "tileset": ["", "floor_plate", "wall"],
  "layers": {"ground": [[2, 2, 2], [2, 1, 2], [2, 2, 2]], "decor": [], "objects": [], "overhead": []},
  "objects": [{"id": "hab_locker", "type": "container", "x": 1, "y": 1,
               "name": "Locker", "items": {"ration_pack": 2}}],
  "regions": [{"id": "mess", "name": "Mess Hall", "x": 1, "y": 1, "w": 8, "h": 6}]
}
```

**Layers.** `ground` (floors and walls), `decor` (floor markings, wall signs),
`objects` (furniture, machines) and `overhead` (drawn above characters). A cell
with no ground tile is solid void. Whether a tile blocks movement is part of
the tile, not the map.

**Map properties** (`props`, the **Map** button in the editor): `ambient` (RGB
light level; dark maps make lamps matter), `backdrop` (`space`, `saturn` or
`none`, visible wherever there is no ground), `cold` (true: anyone without
cold protection starts freezing) and `own_light` (true: lighting here ignores
the station's power grid).

**Regions** are named rectangles: "Mess Hall", "Reactor Hall". The HUD shows
the one you are standing in, schedules and goals send people to them
(`"region": "mess_hall"`), and entering one raises `enter_region:<id>`.

### Objects

Every object has an `id` (unique across all maps), a `type`, a position and
properties. Any property can be changed during play with `set_obj`, and the
change is saved.

| Type | What it is | Main properties |
|---|---|---|
| `spawn` | where the player starts | `name`, `facing` |
| `portal` | step on it to travel to another map | `target_map`, `target_x`, `target_y`, `facing`, `label` |
| `door` | sliding door on the same map | `sprite` (`door`, `door_secure`, `door_v`, `door_h`) |
| `container` | locker, crate, shelf, machine hopper | `items`, `owner`, `restricted`, `machine` |
| `console` | control panel of a station system | `system` |
| `terminal` | readable screen | `text`, or `dialogue` for menus |
| `bed` | a bunk | `owner` (a character id, `player`, or empty) |
| `sign` | something to look at | `text` |
| `item` | an item lying on the floor | `item`, `qty` |
| `light` | a light source | `radius`, `color` |
| `marker` | an invisible named spot for the AI | `name` |
| `trigger` | runs a script when the player steps on it | `on_enter`, `once`, `cond` |
| `interact` | anything else you can examine or use | `name`, `dialogue`, `on_interact`, `cond` |

Any object may also carry `name`, `cond` (it can only be used, or triggered,
while the condition holds), `dialogue` (start this conversation when used),
`on_interact` (run this script when used), `sprite` (draw this tile) and
`solid`.

**Locks.** Portals, doors and containers take `locked`, `key` (an item id),
`access` (comma-separated groups such as `engineering,security`) and
`lock_level`. A locked thing opens for anyone carrying the key or belonging to
one of the groups - that goes for NPCs too, which is how the engineer gets into
the fuel locker and the botanist does not. With a `lock_level` above 0 the
player may also try to bypass it: a Tech check against that number.

**Ownership.** A container with `restricted: true` belongs to its `owner` (a
character id, a faction or a role). NPCs leave other people's restricted
containers alone unless they share one of its `access` groups, a goal step
says `"steal": true`, or they are starving and it holds food. If the player
takes from one in sight of somebody awake, it is noticed.

### The map editor

```
python map_editor.py [map_id]
python map_editor.py --new <id> [width height]
```

| | |
|---|---|
| Paint `B` | left-drag paints the selected tile or prop, right-drag erases |
| Fill `F`, Rect `R` | flood-fill; drag out a filled rectangle |
| Pick `I` | pick up the tile under the cursor (or hold Alt with any tool) |
| Object `O` | click to select, drag to move; choose a type on the right and click the map to add one |
| Region `M` | drag to create, click to select |
| NPC `N` | choose a character on the right, click where they should start |
| `1`-`4` | choose the layer (or leave **Auto layer** on: each tile knows where it belongs) |
| `Tab`, `C`, `L` | grid, collision overlay, lighting preview |
| wheel, middle-drag, arrows | zoom and pan |
| `Ctrl+Z` / `Ctrl+Y`, `Del` | undo / redo, delete the selection |
| `Ctrl+S`, `F5` | save; save and play from the tile under the cursor |

Property values are typed as JSON: `12`, `true`, `"text"`, `{"ration_pack": 2}`,
`["set_flag('seen_it')"]`. **+ Field** adds a property that is not listed.
Saving runs the validator and shows what it found.

Multi-tile props (lockers, beds, the reactor) are placed and removed whole;
their upper rows go on the overhead layer so people can walk behind them.

## Characters

```json
{
  "id": "kira", "name": "Kira Vance", "title": "Technician",
  "role": "technician", "faction": "engineering",
  "bio": "Shown on the Crew tab.",
  "appearance": {"body": "human", "skin": "tan", "hair": "ponytail", "hair_color": "auburn",
                 "outfit": "jumpsuit", "primary": "orange", "secondary": "white", "legs": "slate",
                 "extras": ["glasses"]},
  "attrs": {"body": 5, "dexterity": 6, "intellect": 7, "presence": 4, "resolve": 5},
  "skills": {"tech": 5, "engineering": 3},
  "traits": [], "access": ["crew", "engineering"],
  "spawn": {"map": "engineering", "x": 12, "y": 8, "facing": "down"},
  "home": {"bed": "bed_spare_1"},
  "job": {"system": "fabricator", "restock": ["fabricator"]},
  "schedule": "day",
  "inventory": {"multitool": 1}, "equipped": {"tool": "multitool"}, "credits": 40,
  "relations": {"player": 0, "bram": 20},
  "goals": [],
  "dialogue": "kira"
}
```

**Stats.** Five attributes (1-10): body, dexterity, intellect, presence,
resolve. Nine skills (0-10): engineering, tech, science, medicine, persuasion,
intimidation, stealth, survival, athletics. Each skill gets a small bonus from
its attribute - *effective skill = skill + (attribute - 4) // 2 + modifiers* -
and modifiers come from equipment and status effects. A check is `d20 +
effective skill >= difficulty` (a 1 always fails, a 20 always succeeds). Health
is `20 + 4 x body`. The player and the NPCs use exactly the same rules: a
hungry engineer repairs less with every spare part.

**Appearance** is assembled from parts, so every combination has walking
animations in four directions and a portrait. `body`: human, drone. `hair`:
bald, buzz, short, bob, long, ponytail, spiky, cap, helmet. `outfit`: uniform,
jumpsuit, labcoat, vest, suit. `extras`: glasses, beard. The colour fields
take palette names (the creator lists them).

**Job.** `system` is where they work their shift (they walk to its console,
and their skill keeps it productive). `"duty": "maintain"` with a `maintain`
list makes them responsible for those systems: when one wears down they look
for a spare part and repair it, on their own initiative. `restock` lists
systems they keep supplied (reactor fuel, fabricator scrap). Without a system,
`region` (stand around there, with a `label` such as "Minding the stores") or
`markers` (walk a patrol) describe the job.

**Other fields.** `access` groups open locks. `traits` are free-form labels
scripts can test with `has_trait`; two mean something to the engine:
`synthetic` (no hunger, sleep or air) and `thief` (ignores ownership).
`relations` are opinions from -100 to 100; `player` is the one dialogue
usually tests (`rel() >= 30`). `dialogue` names the conversation file; left
empty, `generic` is used, which lets any character be asked what they are
doing and handed things they want. `trade` turns them into a shopkeeper:
`{"stock": ["stores_food"], "markup": 1.6, "buys": 0.5}` sells from those
containers. `goals` is described [below](#schedules-and-goals-what-npcs-do-all-day).

### The character creator

```
python character_creator.py                 # window
python character_creator.py lin             # open one character
python character_creator.py --new "Bolt" --role drone --map hub --pos 20,12
python character_creator.py --list | --roles | --delete kira
```

`--new` (or the **New** button) builds a complete character from a role in
`archetypes.json`: stats with a little random variation, a look that suits the
role, the first free bunk, a job, a schedule and a free tile next to the
workplace. `--dialogue` also writes a starter conversation. In the window,
everything is editable with a live preview; click the small map to move their
start, **+ Goal from template** to give them a goal, **Play here** to try them
out. A new character joins saved games too: they are added when the save is
loaded.

Add your own roles to `archetypes.json`; an archetype is the same shape as a
character with an extra `looks` section listing the appearance options to
choose from.

## Dialogue

```json
{
  "start": [{"if": "not met()", "goto": "intro"}, {"goto": "hub"}],
  "nodes": {
    "intro": {
      "text": ["First page.", "Second page, with your Tech skill of {skill('tech')} in it."],
      "do": ["set_met()"],
      "next": "hub"
    },
    "hub": {
      "variants": [{"if": "cond('lab') < 40", "text": "The lab is dying."},
                   {"text": "What is it?"}],
      "choices": [
        {"text": "What are you working on?", "goto": "work"},
        {"text": "That waveform is not natural.", "skill": ["science", 4], "once": true, "goto": "insight"},
        {"text": "Let me help you.", "check": ["persuasion", 12], "pass": "yes", "fail": "no"},
        {"text": "Here, take these.", "item": ["spare_parts", 2], "consume": true, "goto": "thanks"},
        {"text": "Fine. Fifty.", "cost": 50, "do": ["give_item('pump_seal')"], "goto": "deal"},
        {"text": "Goodbye.", "goto": "end"}
      ]
    }
  }
}
```

`start` is a node id or a list of redirects; the first whose `if` holds is
taken. The node id `end` closes the conversation.

**Node fields**

| Field | Meaning |
|---|---|
| `text` | a string, or a list of pages |
| `variants` | list of `{"if", "text"}`; the first match replaces `text` |
| `speaker` | who is talking: default the NPC; `player`, `narrator` or a character id |
| `speaker_name` | override the name shown |
| `do` | effects run when the node is entered |
| `branch` | list of `{"if", "goto"}` checked before anything is shown - a silent fork |
| `choices` | what the player can say |
| `next` | where to go when there are no choices |

**Choice fields**

| Field | Meaning |
|---|---|
| `text`, `goto` | what it says and where it leads |
| `if` | hidden unless true |
| `skill`, `attr` | `[name, minimum]`: shown but greyed out when not met |
| `check` | `[skill, difficulty]`: a roll; continue at `pass` or `fail` (optional `pass_do` / `fail_do`) |
| `item` | `[item, count]` the player must carry; taken away if `consume` is true |
| `cost` | credits, taken when chosen |
| `once` | disappears after being picked |
| `do` | effects run when picked |

The player sees a tag in front of anything special - `[Science 4]`,
`[Persuasion 55%]`, `[Spare Parts x2]`, `[50 cr]` - so the price or the risk
is known before choosing.

Branching comes from combining these: `start` and `branch` route by what has
already happened, `variants` change what is said, and `if` on choices opens or
closes paths. Use functions such as `met()`, `talked()`, `rel()`, `wants()`,
`activity()` and `goal_text()` to make people talk about what they are really
doing - the starter file written by the character creator shows the pattern.

Objects can have conversations too (`"dialogue": "airlock"` on a terminal or an
`interact` object); inside those, `obj` is the object and there is no `npc`.
NPCs can start conversations themselves through the `talk` goal step.

## Quests

```json
{
  "q_reactor": {
    "name": "Keep the Lights On", "xp": 90, "main": false,
    "desc": "The fusion reactor is forty years old and losing ground.",
    "stages": {
      "parts": "Bram needs spare parts.",
      "stabilise": "Bring the Fusion Reactor to 85% integrity or better."
    },
    "outcomes": {"done": "The reactor is holding steady.", "failed": "The reactor went dark."}
  }
}
```

A quest is a small state machine driven entirely by scripts: `start_quest`,
`set_stage`, `complete_quest`, `fail_quest`; read back with `quest_started`,
`quest_active`, `quest_done`, `quest_failed`, `stage('q', 'name')`. Stages can
be visited in any order and revisited, which is what lets a quest branch.
Each change raises an event (`quest:<id>:<stage>`, `quest_done:<id>`) that
rules can react to. `journal('text')` adds a free note.

## Story rules and events

`story/rules.json` is a list. Each rule waits for an event, checks a
condition and runs effects:

```json
{"id": "reactor_warning", "on": "hour", "if": "cond('reactor') < 40",
 "do": ["announce('Reactor integrity is falling.')"], "once": false, "cooldown": 720}
```

| Field | Meaning |
|---|---|
| `on` | one event name or a list |
| `if` | condition (optional) |
| `do` | effects |
| `once` | default **true**: the rule retires after firing. Set `false` to repeat |
| `cooldown` | for repeating rules: game minutes before it may fire again |
| `delay` | run the effects this many game minutes after the trigger |

**Events the simulation raises**

| Event | When |
|---|---|
| `start` | a new game begins |
| `tick`, `hour`, `day` | every 10 game minutes, every hour, every midnight |
| `flag:<name>` | `set_flag` was called |
| `quest:<id>:<stage>`, `quest_done:<id>`, `quest_failed:<id>` | quest changes |
| `enter_map:<map>`, `enter_region:<region>` | the player moves |
| `talk:<npc>`, `talk_end:<npc>` | a conversation starts or ends |
| `item:<item>` | the player picks something up |
| `system:<id>:<status>` | a system becomes `ok`, `degraded`, `critical` or `offline` |
| `repaired:<system>`, `priority:<system>` | somebody repaired it / the power queue changed |
| `goal_start:<npc>:<goal>`, `goal_done:...`, `goal_failed:...`, `goal_blocked:...` | NPC goals |
| `npc_took:<npc>`, `npc_delivered:<npc>`, `npc_raid:<container>` | NPCs moving things |
| `theft` | the player was seen taking from a restricted container |
| `collapse:<npc>`, `left:<npc>` | somebody's health ran out / somebody left the station |
| `sleep`, `wake` | the player slept |

Scripts raise their own with `emit('name')`. Because `set_flag` raises an
event, a dialogue can set a flag and a rule can turn it into consequences
somewhere else on the station - that is how most of the campaign is wired.

## Endings

```json
{
  "viable": {
    "title": "Ithaca Holds",
    "slides": [
      {"text": "Captain Voss logs a single word against Ithaca Station: VIABLE."},
      {"if": "flag('verdict_lied')", "text": "It is your name under that word."}
    ]
  },
  "epilogue": {
    "slides": [
      {"if": "quest_done('q_reactor')", "text": "Bram's reactor ran at better than eighty percent."}
    ]
  }
}
```

`end_game('viable')` ends the game and shows that ending's slides whose `if`
holds, followed by the matching slides of the special `epilogue` entry (skip
that with `"no_epilogue": true`). Slides are ordinary text, so they can quote
the state of the world: `{round(cond('reactor'))}`.

## Items and effects

```json
{
  "ration_pack": {"name": "Ration Pack", "kind": "consumable", "weight": 0.4, "value": 8,
                  "icon": "ration", "color": "amber", "desc": "Filling, at least.",
                  "use": ["add_need('hunger', -48)"], "use_text": "You eat the ration.",
                  "tags": ["food"]},
  "thermal_suit": {"name": "Thermal Suit", "kind": "equipment", "weight": 3.0, "value": 110,
                   "icon": "suit", "color": "orange", "slot": "suit",
                   "desc": "Without one the dig will kill you slowly.",
                   "mods": {"cold_resist": 1, "skill.survival": 1}, "tags": ["suit"]}
}
```

`kind`: consumable, tool, equipment, key, material, data, quest or misc.
`slot` (tool, suit or accessory) makes it wearable. `use` runs when the item is
used; consumables are used up. `quest: true` stops the player dropping it.

**Modifiers** (`mods`) apply while an item is equipped or an effect is active:
`skill.<name>`, `skill.all`, `attr.<name>`, `max_hp`, `carry` (kg), `speed`,
`work` (how productive they are at a console) and `cold_resist`.

**Tags** are how the simulation finds things: NPCs eat anything tagged `food`,
repair with anything tagged `part`, and an item tagged `access:engineering`
opens engineering locks for whoever carries it. Icons are drawn by
`game/art/icons.py`; `color` is a palette name.

`effects.json` defines status effects: `{"name", "desc", "mods", "morale",
"hp_per_hour", "bad"}`. The engine applies some by itself - hungry, starving,
tired, exhausted, thin air, suffocating, parched, freezing - from needs, the
station's air and water, and where the character is standing; entries in the
file override those or add new ones for scripts (`add_effect('caffeinated', 3)`).

## Station systems

`systems.json` describes shared **resources** and the **systems** that
produce and consume them. Nothing in the engine knows what a reactor is: add
an entry and it joins the network.

```json
{
  "resources": {
    "water": {"name": "Water", "unit": "L", "start": 560, "cap": 900,
              "per_crew": -1.2, "low": 0.3, "critical": 0.12}
  },
  "systems": {
    "reactor": {"name": "Fusion Reactor", "kind": "generator", "power_out": 120,
                "map": "engineering", "console": "reactor_console",
                "fuel": {"item": "fuel_cell", "hours": 26, "hopper": "reactor_hopper", "start_hours": 14},
                "decay": 0.55, "start_condition": 62, "skill": "engineering", "operator": 0.15,
                "essential": true},
    "hydroponics": {"name": "Hydroponics", "map": "hydroponics", "console": "hydro_console",
                    "power": 18, "priority": 4, "decay": 0.25, "start_condition": 80,
                    "consumes": {"water": 4.5}, "produces": {"air": 0.9},
                    "makes": [{"item": "ration_pack", "hours": 1.5, "into": "galley_dispenser", "max_stock": 14}],
                    "skill": "science", "operator": 0.6}
  }
}
```

Twice every game minute the network:

1. adds up **generation**: each generator's `power_out`, scaled by its
   condition, zero if it burns `fuel` and has none (it draws the fuel item
   from its `hopper` container);
2. hands the power out strictly by **`priority`** (1 first) until it runs
   out - later systems get a share or nothing;
3. works out each consumer's **efficiency** = condition x power x labour x
   inputs, and applies its `consumes` and `produces` (resource units per game
   hour at 100%) and its `makes` recipes (an item every `hours`, taking
   `needs` from one container and putting the result `into` another);
4. wears everything down by `decay` points per hour, and takes `per_crew` of
   each resource for every living crew member.

*Condition* runs from 0 to 100: full output above 75, falling away below it.
`condition_cap` limits how far repairs can go until a script lifts it with
`set_cap` - a missing part that only the story can supply. *Labour* is what
`operator` is about: the share of the system's output that needs a skilled
person at the console (0 runs itself; 1.0 does nothing unattended). Someone
working a shift there builds labour up, by more if their `skill` is high and
they are fed and rested; it fades when nobody comes. `lights: true` marks the
system that keeps the lamps on; `essential: true` cannot be switched off.

Resources at `low` and `critical` (fractions of `cap`) raise events and put
effects on everybody aboard. That closes the loop: less power, less food and
air, weaker crew, slower repairs, less power.

Players reach all of this through consoles (a `console` object with
`"system": "<id>"`): fit a spare part, jury-rig, switch off, or move the
system up and down the queue. Scripts reach it with `cond`, `eff`, `online`,
`res`, `res_pct`, `brownout`, `repair`, `damage`, `set_priority`, `set_cap`,
`set_enabled` and `add_res`.

## Schedules and goals: what NPCs do all day

Every NPC keeps a list of **goals** and, every couple of seconds, pursues the
eligible one with the highest priority. Goals are plain data, so they are
saved mid-step, and they run identically whether or not the player is on the
same map.

Goals come from four places:

| Source | Example | Priority |
|---|---|---|
| **Routine** - the schedule | off duty 20, shift 40, meal 44, sleep 60 | fixed |
| **Drives** - set by the NPC from their own situation | hungry (from 55% hunger) | 50 rising to 95 |
| | a system they maintain is below 65% | 58 rising to 90 |
| | a machine they supply is empty | 52 (fuel: 78) |
| | badly hurt: go to the medbay | 82 |
| | exhausted: sleep now | 86 |
| **Authored** - `goals` in the character file | Lin decoding the signal | yours (default 50) |
| **Story** - `add_goal(...)` from a script | the sergeant making an arrest | yours |

So an authored goal at 46 is done between other things but gives way to sleep
and hunger; one at 70 keeps them up at night; one at 90 beats almost
everything.

### Schedules

```json
{
  "day": [
    {"from": 6, "to": 7, "do": "meal"},
    {"from": 7, "to": 12, "do": "work"},
    {"from": 12, "to": 13, "do": "meal"},
    {"from": 13, "to": 18, "do": "work"},
    {"from": 18, "to": 22, "do": "leisure", "region": "rec_room"},
    {"from": 22, "to": 6, "do": "sleep"}
  ]
}
```

`do` is `work` (their job), `meal` (fetch food if hungry, then sit in the
mess), `leisure`, `sleep` (their own bed), `patrol` (the job's `markers`) or
`idle`. A block may name a `region`, a `label` for the Crew tab and its own
`priority`. A character's `schedule` is one of these names, or a list of
blocks written out in the character file.

### Goals

```json
{
  "id": "decode_signal", "desc": "Decode the signal from under the ice", "priority": 47,
  "while": "not flag('lin_paused')",
  "retry": 90,
  "steps": [
    {"do": "acquire", "item": "signal_amplifier", "count": 1},
    {"do": "work", "system": "lab", "var": "signal_progress", "target": 40, "label": "Listening to the ice at"},
    {"do": "run", "script": ["set_flag('signal_decoded')"]}
  ],
  "on_complete": ["announce('The lab reports a result.')"]
}
```

| Field | Meaning |
|---|---|
| `id`, `desc` | the id scripts use; the description shown on the Crew tab and in the log |
| `priority` | see the table above |
| `steps` | done in order; the goal is complete after the last |
| `when` | condition to **start**. Once begun it is not asked again, so the job is finished even if doing it made the condition false |
| `while` | condition that must hold **the whole time**. When false the goal is put down where it stands and resumed later |
| `repeat`, `cooldown` | start over after finishing, no sooner than `cooldown` game minutes later |
| `retry` | game minutes to wait after getting stuck (default 25) |
| `max_blocks` | give up (fail) after being stuck this many times |
| `on_complete`, `on_fail` | scripts |
| `log` | write start, finish and "is stuck" lines to the station log |

When a step cannot proceed - no such item anywhere they are allowed to take
it, the way is locked - the goal is *blocked*: the NPC notes what they are
missing (`wants()`), the log says so, and they try again later. That is the
hook for the player: bring Lin the amplifier, open the stores for Bram.

**Steps**

| `do` | Does | Fields |
|---|---|---|
| `goto` | walk somewhere | a place (see below) |
| `work` | operate a system's console | `system`; stop after `hours`, when `until` is true, or when the variable `var` reaches `target` (it grows with their skill and the system's efficiency) |
| `repair` | fit carried parts to a system, or jury-rig | `system`, `target` (default 88) |
| `acquire` | fetch an item from the nearest container they may use | `item` or `tag`, `count`, `from` / `exclude` (container ids), `steal`, `optional`, `strict` |
| `deliver` | carry it to a container | `item` or `tag`, `to`, `count` (default all) |
| `consume` | eat or use an item they carry | `item` or `tag` |
| `sleep` | go to bed | `until_fatigue` |
| `recover` | lie down until healed | `object` (a bed), `until_hp` |
| `idle` | hang around | `region`, `minutes`, `label` |
| `patrol` | walk between markers | `markers`, `laps` |
| `wait` | stay at a place | a place, and `minutes` or `until` |
| `talk` | walk up to somebody. To the player: opens a conversation at `node`. To an NPC: says `line`, then runs `effects` | `char`, `dialogue`, `node`, `line`, `minutes`, `optional` |
| `say` | a speech bubble | `text`, `seconds` |
| `run` | run a script | `script` |
| `leave` | leave the station for good | `reason` |

A *place* is one of `object` (id), `system` (its console), `marker`, `bed`
(their own), `char`, `region` (a free tile in it) or `x` / `y` (with `map`).
NPCs find their own way there, through portals and doors they are able to
open. Any step may carry an `if`; when false the step is skipped.

**Templates.** Goals used more than once go in `goals.json` with `$name`
placeholders, and are used from a character file -
`{"use": "stockpile", "args": {"item": "coffee_bulb", "count": 4, "stash": "tool_locker", "label": "coffee"}}` -
or handed out by a script:

```json
{"do": ["add_goal('halloran', 'arrest', target='dex', flag='dex_arrested', label='Dex Marlow')"]}
```

`cancel_goal`, `set_goal_priority`, `goal_active`, `goal_done`, `goal_progress`,
`npc_goal`, `goal_text`, `activity` and `wants` let dialogue and rules see and
steer what people are up to.

## Game settings

`game.json`:

| Field | Meaning |
|---|---|
| `title`, `subtitle`, `premise` | title screen and new-game text |
| `time_scale` | game minutes per real second (default 1) |
| `start` | `{"map", "spawn", "day", "hour", "minute"}`; `spawn` is a spawn object's id |
| `intro` | dialogue played when a new game starts |
| `player` | the player's starting character: same fields as any character |
| `backgrounds` | choices offered at the start: `{"id", "name", "desc", "attrs", "skills", "items", "equipped"}`, added on top of `player` |
| `mess_region` | where meal breaks are spent |
| `medbay_bed` | the bed the badly hurt are taken to |
| `part_item`, `food_item` | what NPCs ask for when they are short of parts or food |

## Checking your work

* `python -m game.tools.validate` - run it constantly. It checks every
  reference (items, maps, objects, characters, quests, stages, systems, nodes),
  parses every script and checks its function names and arguments, finds
  portals that lead into walls, maps that cannot be reached, flags that are
  read but never set, and more. The editors run it on save.
* `python -m game.tools.simulate --days 5 --log --crew` - let the station run
  with no player and read what happened. The quickest way to see whether a new
  goal or system behaves.
* `python main.py --map <id> --pos x,y` - start a throwaway game anywhere.
* In game, **F3** shows a debug overlay (positions, goals, the last script
  error); with it on, **F6** and **F7** skip one and six hours.
* `python -m pytest` - the suite includes a random walk through every
  conversation and scripted playthroughs of the campaign. `tests/helpers.py`
  has a `talk(world, "npc", ["text of a choice", ...])` helper that makes
  playthrough tests of your own story a few lines each.

## Extending the engine

When data is not enough, each of these is a small, local addition.

**A tile** - `game/art/tiles.py`. Art is drawn in code on a 16x16 canvas with
the palette ramps defined at the top of the file:

```python
@tile("vending", "Vending machine", "machine", solid=True, frames=2, fps=2, light=(28, CY[3]))
def _vending(c):
    c.rect(2, 1, 12, 14, BLACK)
    c.rect(3, 2, 10, 12, S[2])
    c.rect(4, 3, 8, 5, CY[1] if c.frame == 0 else CY[3])      # a blinking screen
    c.rect(4, 10, 8, 2, S[0])
```

It appears in the editor palette under its category at once. Options: `layer`
(default `objects`), `solid`, `frames` / `fps` (animation), `variants` (random
variation per cell), `group` (tiles that join up, like walls) and `light`
(radius and colour). `python -m game.tools.artdump` writes contact sheets so
you can see what you drew.

**A multi-tile prop** - `game/art/props.py`: `@prop("antenna", "Antenna mast",
"machine", 1, 3)` draws one 16x48 picture that the editor places as a unit;
the bottom row is solid, the rest is drawn over characters.

**An item icon** - `game/art/icons.py`: `@icon("bottle")` on a function taking
the canvas and a three-tone tint.

**A script function** - a method on `ScriptAPI` in `game/story/api.py`:

```python
    @script_fn("cond")
    def crew_asleep(self):
        """How many of the crew are asleep right now."""
        return sum(1 for c in self.world.npcs() if c.asleep)
```

Every content file can now use `crew_asleep()`. The docstring becomes the
reference entry; regenerate it with
`python -m game.tools.validate --api-md > docs/SCRIPT_API.md`. If an argument
is an id, add the function to `REFS` in `game/tools/validate.py` and the
validator will check that argument in every script.

**A goal step** - `game/sim/ai.py`:

```python
@step("cheer")
def _step_cheer(world, npc, goal, spec, state, dt):
    for other in world.chars_in(npc.map_id):
        other.morale = min(100.0, other.morale + float(spec.get("amount", 5)))
    npc.say("Good work, everyone.", 3.0)
    return DONE
```

Return `RUNNING` to be called again next tick (keep progress in `state`, which
is saved), `DONE`, `BLOCKED` (try again later) or `FAILED`. Use it from data
with `{"do": "cheer", "amount": 7}`.

**A map object type** - add an entry to `OBJECT_TYPES` in
`game/world/gamemap.py` (label, editor colour, default fields) and handle it
in `PlayScene._interact` in `game/scenes/play.py`.

**A screen** - subclass `Scene` (`game/engine/scene.py`) with `update(dt)` and
`draw(surface)`, and `app.push_scene(...)` it; `game/scenes/panels.py` has
small examples. Scenes draw on a 384x216 surface that the engine scales up.

The rule of thumb that keeps the project testable: `game/world`, `game/sim`
and `game/story` never import pygame. Put logic there and presentation in
`game/scenes`, and everything you add can be simulated and tested headless.
