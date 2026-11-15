# Script function reference

<!-- Generated file: python -m game.tools.validate --api-md > docs/SCRIPT_API.md -->

Every function that content files can call (146 in all). This page is generated from
`game/story/api.py`; regenerate it after adding a function:

    python -m game.tools.validate --api-md > docs/SCRIPT_API.md

Scripts are small Python-looking expressions and statements stored as strings in the JSON
content: `if` fields take one expression, `do` fields take a list of statements, and text
may embed expressions in `{braces}`. See [AUTHORING.md](AUTHORING.md#the-script-language)
for the language itself.

`who` is always a character id such as `'bram'` or `'player'`. When it is left out:

* health, need and status-effect functions (`hp`, `need`, `heal`, `add_effect` ...) mean
  *whoever is using the item*, otherwise the player;
* inventory, money, skill, attribute and access functions (`has_item`, `credits`, `skill`,
  `roll`, `give_access` ...) mean *the player*;
* relationship, morale, name, goal and activity functions (`rel`, `add_rel`, `name`, `wants`,
  `activity`, `say` ...) mean *the NPC the script belongs to* - the person being talked to or
  the owner of the goal.

## Conditions and getters

Use these in `if` fields and inside other expressions.

| Function | What it does |
|---|---|
| `allies(threshold=20)` | How many people still aboard like the player at least this much. |
| `asleep(who=None)` | True if the character is asleep. |
| `attr(name, who=None)` | Effective attribute. |
| `background()` | The player's background id. |
| `brownout()` | True while demand exceeds supply. |
| `can_trade(who=None)` | True if the NPC's character file gives them a ``trade`` section. |
| `cap(system)` | The integrity ceiling of a system (100 unless something is missing). |
| `chance(probability)` | True with the given probability (0-1). |
| `cond(system)` | Integrity (0-100) of a station system. |
| `container_count(obj, item)` | Count of an item in one container. |
| `credits(who=None)` | Credits held. |
| `crew_morale()` | Average morale of everyone on the station. |
| `day()` | Current day number (starts at 1). |
| `eff(system)` | Current efficiency (0-1) of a station system. |
| `enabled(system)` | True if the system is switched on. |
| `equipped(item, who=None)` | True if ``item`` is equipped. |
| `flag(name, default=False)` | Value of a story flag (False if never set). |
| `goal_active(who, goal)` | True if the NPC has this goal and it is not finished. |
| `goal_done(who, goal)` | True if the NPC completed this goal. |
| `goal_failed(who, goal)` | True if the NPC failed this goal. |
| `goal_progress(who, goal)` | Progress 0..1 of an NPC goal. |
| `has_effect(effect, who=None)` | True if the character currently has a status effect. |
| `has_item(item, count=1, who=None)` | True if the character (default player) carries ``count`` of ``item``. |
| `has_tag(tag, who=None)` | True if the character carries any item with this tag. |
| `has_trait(trait, who=None)` | True if the character has a trait. |
| `hour()` | Hour of the day, 0-23. |
| `hours_until(day, hour=0)` | Game hours remaining until a given day and hour. |
| `hp(who=None)` | Current health. |
| `hp_pct(who=None)` | Health as a percentage of maximum. |
| `in_map(map_id)` | True if the player is on this map. |
| `in_region(region)` | True if the player is standing in this region. |
| `item_count(item, who=None)` | How many of ``item`` the character carries. |
| `level(who=None)` | Character level. |
| `locked(obj=None)` | True if a door or container is locked. |
| `met(who=None)` | True once the player has spoken to this NPC. |
| `minute()` | Minute of the hour. |
| `morale(who=None)` | Morale 0-100 of a character (default: the NPC being talked to). |
| `near(who, distance=4)` | True if the character is within ``distance`` tiles of the player. |
| `need(name, who=None)` | A need value 0-100 ("hunger" or "fatigue"); higher is worse. |
| `npc_goal(who=None)` | Id of the goal an NPC is pursuing right now. |
| `npc_has(who, item, count=1)` | True if an NPC carries the item. |
| `npc_in(who, map_id)` | True if the character is on the given map. |
| `npc_region(who=None)` | Id of the region the character is standing in ('' if none). |
| `obj(key, obj=None, default=False)` | Read a property of a map object. |
| `obj_exists(obj)` | True if a map object with this id currently exists. |
| `online(system)` | True if the system is switched on and producing anything. |
| `power_demand()` | Power being asked for (kW). |
| `power_supply()` | Power being generated (kW). |
| `present(who=None)` | True while the character is on the station. |
| `priority(system)` | A system's place in the power queue (1 = served first). |
| `quest_active(quest)` | True while a quest is in progress. |
| `quest_done(quest)` | True once a quest was completed. |
| `quest_failed(quest)` | True once a quest was failed. |
| `quest_started(quest)` | True if a quest was ever started. |
| `rand(low, high)` | Random whole number between low and high inclusive. |
| `rel(who=None)` | How much an NPC likes the player, -100..100. |
| `rel_between(a, b)` | How much character ``a`` likes character ``b``. |
| `res(resource)` | Amount of a station resource (e.g. 'air', 'water'). |
| `res_pct(resource)` | A station resource as a percentage of its capacity. |
| `roll(skill, difficulty, who=None)` | Skill check: d20 + skill >= difficulty. |
| `skill(name, who=None)` | Effective skill (with attribute bonus and modifiers). |
| `stage(quest, stage=None)` | Current stage id of a quest, or True/False if ``stage`` is given. |
| `stage_seen(quest, stage)` | True if the quest ever reached ``stage``. |
| `stock(item)` | Total count of an item in every container on the station. |
| `sys_status(system)` | 'ok', 'degraded', 'critical' or 'offline'. |
| `talked(who=None)` | How many conversations the player has had with this NPC. |
| `time()` | Game minutes elapsed since 00:00 on day 1. |
| `time_between(start_hour, end_hour)` | True if the time of day is within the range (may wrap midnight). |
| `var(name, default=0)` | Value of a numeric story variable. |
| `wants(who=None)` | Id of an item the NPC is looking for ('' if nothing). |
| `wants_count(who=None)` | How many of the wanted item the NPC still needs. |

## Text helpers

Return text; meant for `{braces}` inside dialogue and messages.

| Function | What it does |
|---|---|
| `activity(who=None)` | What an NPC is doing this moment ("Repairing Life Support"). |
| `clock()` | The time as text, e.g. 'Day 2 14:05'. |
| `first_name(who=None)` | Short name of a character. |
| `goal_text(who=None)` | Description of what an NPC is currently trying to do. |
| `item_name(item)` | Display name of an item. |
| `name(who=None)` | Display name of a character. |
| `player_name()` | The player's name. |
| `title(who=None)` | Job title of a character. |

## Effects

Change the world; use them in `do` lists.

| Function | What it does |
|---|---|
| `add_attr(name, amount=1, who=None)` | Permanently raise an attribute. |
| `add_effect(effect, hours=None, who=None)` | Apply a status effect, optionally for a number of game hours. |
| `add_goal(who, goal, priority=None, **args)` | Give an NPC a goal from goals.json (extra keyword args fill ``$name`` placeholders in the template). |
| `add_morale(amount, who=None)` | Change morale of one character, or everybody with ``who='all'``. |
| `add_need(name, amount, who=None)` | Change a need: ``add_need('hunger', -45)`` feeds the character. |
| `add_rel(amount, who=None)` | Change how much an NPC likes the player. |
| `add_rel_between(a, b, amount)` | Change how much character ``a`` likes character ``b``. |
| `add_res(resource, amount)` | Add to (or subtract from) a station resource. |
| `add_skill(name, amount=1, who=None)` | Permanently raise a skill. |
| `add_var(name, amount=1)` | Add to a numeric story variable. |
| `add_xp(amount)` | Give the player experience. |
| `announce(text)` | Station-wide announcement (also written to the log). |
| `cancel_goal(who, goal)` | Remove a goal from an NPC. |
| `clear_flag(name)` | Remove a story flag. |
| `complete_quest(quest)` | Finish a quest successfully (awards its XP). |
| `damage(system, amount)` | Damage a system. |
| `emit(event)` | Raise a custom event for director rules. |
| `end_game(ending)` | End the game with the given ending id. |
| `fail_quest(quest)` | Fail a quest. |
| `fill_tiles(map_id, layer, x, y, w, h, tile)` | Change a rectangle of map tiles ('' clears them). |
| `give_access(group, who=None)` | Grant a door access group (e.g. 'engineering'). |
| `give_credits(amount, who=None)` | Add credits. |
| `give_item(item, count=1, who=None)` | Create ``count`` of ``item`` in the character's inventory. |
| `give_to(who, item, count=1)` | The player hands ``count`` of ``item`` to an NPC. |
| `heal(amount, who=None)` | Restore health. |
| `hurt(amount, who=None)` | Deal damage. |
| `journal(text)` | Write a free-form note into the journal. |
| `lock(obj=None)` | Lock a door, portal or container. |
| `log(text)` | Write a line into the activity log. |
| `move_items(source, dest, item=None)` | Move everything (or one kind of item) between two inventories; each is 'player', a character id or a container object id. |
| `move_npc(who, map_id, x, y, facing=None)` | Teleport a character. |
| `notify(text, kind='info')` | Show a small notification. |
| `open_container(obj=None)` | Open a container's contents. |
| `open_trade(who=None)` | Open the trade screen with an NPC. |
| `pass_time(minutes)` | Let game time pass (the whole simulation keeps running). |
| `put_item(obj, item, count=1)` | Add items to a container. |
| `remove_effect(effect, who=None)` | Remove a status effect. |
| `remove_from(obj, item, count=1)` | Remove items from a container. |
| `remove_npc(who, reason='left')` | The character leaves the station permanently. |
| `remove_obj(obj=None)` | Delete a map object. |
| `repair(system, amount)` | Restore integrity to a system. |
| `say(text, who=None, seconds=4)` | Show a speech bubble over a character. |
| `set_cap(system, value=100)` | Raise or lower the integrity ceiling repairs can reach on a system. |
| `set_condition(system, value)` | Set a system's integrity outright. |
| `set_dialogue(who, dialogue)` | Switch the dialogue file an NPC uses. |
| `set_enabled(system, on=True)` | Switch a system on or off. |
| `set_flag(name, value=True)` | Set a story flag; raises the event ``flag:<name>``. |
| `set_goal_priority(who, goal, priority)` | Change how badly an NPC wants one of their goals. |
| `set_home(who, bed='')` | Give an NPC a different bed to sleep in ('' = none). |
| `set_job(who, system='')` | Reassign an NPC to operate a different system ('' = none). |
| `set_maintain(who, *systems)` | Make an NPC responsible for repairing the listed systems. |
| `set_met(who=None)` | Mark an NPC as known to the player. |
| `set_obj(obj, key, value)` | Change a property of a map object. |
| `set_priority(system, priority)` | Change a system's place in the power queue (1 = first). |
| `set_schedule(who, schedule)` | Change an NPC's daily schedule (a name from schedules.json). |
| `set_stage(quest, stage)` | Move a quest to another stage. |
| `set_tile(map_id, layer, x, y, tile)` | Change one map tile (layer: ground, decor, objects or overhead). |
| `set_var(name, value)` | Set a numeric story variable. |
| `sound(name)` | Play a sound effect. |
| `spawn_item(map_id, x, y, item, count=1)` | Drop an item pickup onto a map. |
| `start_dialogue(dialogue, who=None, node=None)` | Start another dialogue (optionally with an NPC, at a node). |
| `start_quest(quest, stage=None)` | Add a quest to the journal. |
| `take_credits(amount, who=None)` | Remove credits (never below zero). |
| `take_from(who, item, count=1)` | An NPC hands the player up to ``count`` of ``item`` (if they have it). |
| `take_item(item, count=1, who=None)` | Remove items from the character's inventory. |
| `teleport(map_id, x, y, facing=None)` | Move the player. |
| `unlock(obj=None)` | Unlock a door, portal or container. |

## Also available

`min`, `max`, `abs`, `round`, `int`, `float`, `str`, `len`, `bool`, `and` / `or` / `not`,
comparisons (including chained ones and `in`), arithmetic, and `x if cond else y`.

Context names: `npc` / `self` (the NPC the script belongs to), `obj` (the map object),
`item`, `map` and `player` - each is an id string that can be passed to the functions above.

