"""Drive the real scenes with synthetic input (headless)."""
import pygame
import pytest

from game import save
from game.tools.bot import Bot


@pytest.fixture
def bot(content):
    b = Bot.new_game({"name": "Tester", "background": "engineer"}, seed=3, content=content)
    w = b.world
    w.characters["halloran"].goals = [g for g in w.characters["halloran"].goals if g["id"] != "greet_auditor"]
    b.frames(3)
    return b


def test_title_new_game_flow(content):
    from game.engine.app import App
    from game.scenes.title import TitleScene
    app = App(headless=True, audio=False, content=content, persist_options=False)
    app.push_scene(TitleScene(app))
    bot = Bot(app)
    bot.frames(3)
    assert [i.data for i in bot.scene.menu.items] == ["new", "load", "quit"]
    bot.tap("confirm")
    assert bot.scene.name == "newgame"
    for _ in range(20):
        bot.key(pygame.K_BACKSPACE)
    bot.type("Ada")
    bot.key(pygame.K_RETURN)
    bot.tap("down")
    bot.tap("down")
    bot.tap("confirm")                         # third background: negotiator
    bot.tap("tab_next")                        # randomise the look
    for _ in range(8):
        bot.tap("down", 1)
    bot.tap("confirm", 6)
    assert bot.in_dialogue, "the intro plays"
    player = bot.world.player
    assert player.name == "Ada" and player.background == "negotiator" and player.skills["persuasion"] == 4
    bot.end_dialogue()
    assert bot.scene.name == "play" and bot.world.quests.status("q_audit") == "active"


def test_halloran_walks_up_and_greets_the_player(content):
    bot = Bot.new_game(seed=1, content=content)
    for _ in range(300):
        bot.frames(1)
        if bot.in_dialogue:
            break
    assert bot.in_dialogue and bot.scene.runner.npc_id == "halloran"
    assert bot.choose("Where do I find the Administrator")
    bot.end_dialogue()
    assert bot.world.characters["halloran"].met


def test_walking_portals_and_collision(bot):
    start = bot.player.pos
    assert not bot.step("left") or bot.player.pos != start
    assert bot.walk_to(15, 15)
    assert bot.use_portal("hangar_to_hub") and bot.player.map_id == "hub"
    assert save.read_header("autosave") is not None, "changing map autosaves"
    assert bot.use_portal("hub_to_habitat") and bot.player.map_id == "habitat"
    wall_x, wall_y = 0, 0
    assert bot.world.maps["habitat"].solid_at(wall_x, wall_y)
    before = bot.player.pos
    bot.face("up")
    for _ in range(6):
        bot.step("up")
    assert bot.player.y >= 1 and bot.player.pos != before or bot.player.y == before[1]


def test_talking_to_someone_opens_their_dialogue(bot):
    assert bot.use_portal("hangar_to_hub") and bot.use_portal("hub_to_eng")
    assert bot.go_to_char("pip")
    bot.frames(2)
    assert bot.play.target and bot.play.target[0] == "npc"
    bot.interact()
    assert bot.in_dialogue and bot.scene.runner.npc_id == "pip"
    bot.frames(30)                              # let the typewriter run and draw
    assert bot.choose("Where is the amplifier core")
    bot.end_dialogue()
    assert bot.scene.name == "play" and bot.world.flags.get("knows_amp")


def test_datapad_tabs_draw_and_close(bot):
    bot.world.quests.start("q_audit")
    bot.world.characters["bram"].met = True
    bot.tap("menu")
    assert bot.scene.name == "datapad"
    for _ in range(7):
        bot.tap("right", 2)
        bot.tap("down", 1)
    bot.tap("cancel")
    assert bot.scene.name == "play"
    for action in ("inventory", "journal", "crew", "station"):
        bot.tap(action)
        assert bot.scene.name == "datapad"
        bot.tap("cancel")


def test_using_and_equipping_items_from_the_menu(bot):
    p = bot.player
    p.needs["hunger"] = 60
    p.unequip("tool")
    bot.tap("inventory")
    tab = bot.scene.tab
    names = [i.data for i in tab.menu.items]
    tab.menu.index = names.index("ration_pack")
    bot.tap("confirm")                          # opens the action pop-up
    assert bot.scene.name == "choice"
    bot.tap("confirm")                          # "Use"
    assert p.needs["hunger"] < 20 and not p.inventory.has("ration_pack")
    tab.menu.index = [i.data for i in tab.menu.items].index("multitool")
    bot.tap("confirm")
    bot.tap("confirm")                          # "Equip"
    assert p.is_equipped("multitool")
    bot.close_overlays()


def test_spending_skill_points(bot):
    p = bot.player
    p.skill_points = 1
    bot.tap("menu")
    tab = bot.scene.tab
    tab.menu.index = [i.data for i in tab.menu.items].index(("skill", "medicine"))
    bot.tap("confirm")
    assert p.skills["medicine"] == 1 and p.skill_points == 0
    bot.close_overlays()


def test_container_window_and_theft(bot):
    w = bot.world
    bot.walk_to(22, 6)
    bot.walk_to(22, 4)
    dex = w.characters["dex"]
    w.transfer(dex, "hangar", 24, 3)
    assert bot.go_to_object("stores_food")
    bot.interact()
    assert bot.scene.name == "container"
    before = w.player.inventory.count("coffee_bulb")
    bot.tap("confirm")
    assert w.player.inventory.count("coffee_bulb") == before + 1
    assert w.flags.get("thefts_seen") == 1 and dex.opinion_of("player") < 5
    bot.tap("right")
    bot.tap("tab_next")                         # put a whole stack back
    bot.tap("cancel")
    assert bot.scene.name == "play"


def test_taking_from_stores_you_were_cleared_for_is_not_theft(bot):
    w = bot.world
    bot.walk_to(22, 6)
    bot.walk_to(22, 4)
    w.transfer(w.characters["dex"], "hangar", 24, 3)
    # what the commander's "open the stores" order does
    w.script.run(["set_obj('stores_parts', 'access', 'engineering,stores,auditor')", "give_access('auditor')"])
    assert w.find_object("stores_parts").get("restricted"), "still off limits to everybody else"
    assert bot.go_to_object("stores_parts")
    bot.interact()
    assert bot.scene.name == "container"
    bot.tap("confirm")
    assert w.player.inventory.count("spare_parts") == 1
    assert not w.flags.get("thefts_seen")
    bot.tap("cancel")


def test_locked_container_offers_a_bypass(bot):
    bot.walk_to(22, 6)
    bot.walk_to(22, 4)
    assert bot.go_to_object("stores_fuel")
    bot.frames(2)
    assert bot.play.target[2].startswith("Locked")
    bot.interact()
    assert bot.scene.name == "choice" and "Tech" in bot.scene.options[0][0]
    bot.world.rng.seed(1)
    clock = bot.world.clock.minutes
    bot.tap("confirm")
    assert bot.world.clock.minutes >= clock + 10, "an attempt costs time"
    bot.close_overlays()


def test_trading(bot):
    w = bot.world
    dex = w.characters["dex"]
    w.transfer(dex, "hangar", 24, 3)
    dex.schedule = [{"from": 0, "to": 24, "do": "idle", "region": "stores"}]     # keep him at his desk
    bot.walk_to(22, 6)
    bot.walk_to(22, 4)
    assert bot.go_to_char("dex")
    bot.interact()
    assert bot.choose("Show me what you have")
    bot.frames(3)
    assert bot.scene.name == "trade"
    credits, dex_credits = w.player.credits, dex.credits
    item = bot.scene.menus[0].selected.data
    have = w.player.inventory.count(item)
    bot.tap("confirm")
    assert w.player.inventory.count(item) == have + 1
    assert w.player.credits < credits and dex.credits > dex_credits
    bot.tap("right")
    sell = bot.scene.menus[1]
    sell.index = [i.data for i in sell.items].index("spare_parts")
    bot.tap("confirm")
    assert not w.player.inventory.has("spare_parts")
    bot.tap("cancel")
    assert bot.in_dialogue, "back in the conversation"
    bot.end_dialogue()


def test_system_console_repair_and_power_queue(bot):
    w = bot.world
    w.flags["audit_authority"] = True
    assert bot.use_portal("hangar_to_hub") and bot.use_portal("hub_to_medlab")
    bot.walk_to(14, 5)
    lin = w.characters["lin"]
    w.transfer(lin, "habitat", 3, 3)             # nobody guarding the board
    assert bot.go_to_object("lab_console")
    bot.interact()
    assert bot.scene.name == "console"
    menu = bot.scene.menu
    menu.index = [i.data for i in menu.items].index("prio_down")
    for _ in range(5):
        bot.tap("confirm")
    assert w.systems.get("lab").priority == 7 and w.flags.get("lab_demoted")
    menu.index = [i.data for i in menu.items].index("toggle")
    bot.tap("confirm")
    assert not w.systems.get("lab").enabled
    menu.index = [i.data for i in menu.items].index("repair")
    cond = w.systems.get("lab").condition
    bot.tap("confirm")
    assert w.systems.get("lab").condition > cond and not w.player.inventory.has("spare_parts")
    menu.index = [i.data for i in menu.items].index("diag")
    bot.tap("confirm")
    assert bot.scene.name == "message"
    bot.close_overlays()


def test_console_refuses_without_authority(bot):
    w = bot.world
    w.player.skills["tech"] = 0
    assert bot.use_portal("hangar_to_hub") and bot.use_portal("hub_to_eng")
    assert bot.go_to_object("grid_console")
    bot.interact()
    menu = bot.scene.menu
    menu.index = [i.data for i in menu.items].index("toggle")
    bot.tap("confirm")
    assert w.systems.get("grid").enabled and "authorisation" in bot.scene.message
    bot.close_overlays()


def test_waiting_and_sleeping(bot):
    w = bot.world
    start = w.clock.minutes
    bot.tap("wait")
    assert bot.scene.name == "wait"
    bot.tap("confirm")                          # one hour
    for _ in range(60):
        if bot.scene.name == "play":
            break
        bot.frames(1)
    assert 55 <= w.clock.minutes - start <= 75
    assert bot.use_portal("hangar_to_hub") and bot.use_portal("hub_to_habitat")
    bot.walk_to(4, 7)
    bot.walk_to(4, 5)
    w.player.needs["fatigue"] = 85
    assert bot.go_to_object("bed_player")
    bot.frames(2)
    assert bot.play.target[2] == "Sleep"
    bot.interact()
    bot.tap("confirm")                          # sleep until rested
    for _ in range(400):
        if bot.scene.name == "play":
            break
        bot.frames(1)
    assert w.player.needs["fatigue"] < 10 and not w.player.asleep
    assert w.clock.minutes - start > 300


def test_save_and_load_through_the_menus(bot):
    w = bot.world
    w.flags["marker"] = "before"
    bot.tap("menu")
    bot.tap("left")                             # SYSTEM tab
    bot.tap("confirm")                          # Save game
    assert bot.scene.name == "saveload"
    bot.tap("confirm")                          # slot 1
    assert save.read_header("slot1") is not None
    bot.close_overlays()
    w.flags["marker"] = "after"
    bot.tap("quicksave")
    assert save.read_header("quicksave") is not None
    w.flags["marker"] = "later"
    bot.tap("quickload", 6)
    assert bot.world is not w and bot.world.flags["marker"] == "after"
    assert bot.play.load_slot("slot1")
    bot.frames(4)
    assert bot.world.flags["marker"] == "before"


def test_pushing_past_a_bystander(bot):
    w = bot.world
    p = bot.player
    mote = w.characters["mote"]
    w.transfer(p, "hub", 20, 2)
    w.transfer(mote, "hub", 21, 2)
    mote.goals.clear()
    mote.schedule = [{"from": 0, "to": 24, "do": "idle", "label": "Standing still"}]
    w.requests.clear()
    bot.frames(2)
    inp = bot.app.input
    for _ in range(40):
        inp.begin_frame()
        inp.hold("right", True)
        bot.app.frame_step(1 / 30.0, None)
    inp.hold("right", False)
    bot.frames(20)
    assert p.x >= 21, "the player squeezed past"


def test_ending_scene_plays_through(bot):
    from game.scenes.ending import EndingScene
    w = bot.world
    w.end_game("blackout")
    bot.frames(3)
    assert bot.scene.name == "ending"
    for _ in range(40):
        bot.tap("confirm", 1)
        if getattr(bot.scene, "done", False) or bot.scene.name == "title":
            break
    assert bot.scene.name in ("ending", "title")
    assert isinstance(bot.scene, EndingScene) is (bot.scene.name == "ending")


def test_collapse_wakes_the_player_in_the_medbay(bot):
    w = bot.world
    w.player.hp = 0.0
    w.collapse(w.player)
    start = w.clock.minutes
    bot.frames(8)
    assert w.player.map_id == "medlab" and w.clock.minutes >= start + 350
    assert w.player.hp >= w.player.max_hp * 0.39


def test_opening_hours_played_through_the_interface(content):
    """Intro, the sergeant's welcome, the briefing, the reactor quest and the
    wait screen - walked and clicked the way a player would."""
    bot = Bot.new_game({"name": "Rin Okabe", "background": "negotiator"}, seed=2, content=content, intro=True)
    w = bot.world
    bot.frames(5)
    assert bot.in_dialogue, "the game opens with the intro"
    bot.end_dialogue()
    assert w.quests.stage("q_audit") == "report_in"

    for _ in range(40 * 30):                    # the sergeant comes over on his own
        if bot.in_dialogue:
            break
        bot.frames(1)
    assert bot.in_dialogue and bot.scene.runner.npc_id == "halloran"
    assert bot.choose("Nothing is decided")
    bot.end_dialogue()

    def travel(*portals):
        for portal in portals:
            assert bot.use_portal(portal), portal

    def talk_to(char_id, pick):
        assert bot.go_to_char(char_id), "cannot reach " + char_id
        bot.interact()
        assert bot.in_dialogue
        for _ in range(6):                      # click through introductions until the option appears
            choices = bot.dialogue_choices()
            if any(pick.lower() in c.text.lower() for c in choices):
                break
            bot.scene.index = [i for i, c in enumerate(choices) if c.enabled][0]
            bot.tap("confirm", 1)
        assert bot.choose(pick), "%s never offered %r" % (char_id, pick)
        bot.end_dialogue()

    travel("hangar_to_hub", "hub_to_command")
    talk_to("osei", "Show me")
    assert w.flags.get("audit_authority") and w.quests.stage("q_audit") == "inspect"
    travel("command_to_hub", "hub_to_eng")
    talk_to("bram", "Who is giving you forms")
    assert w.quests.stage("q_reactor") == "parts" and w.systems.get("reactor").condition < 65
    travel("eng_to_hub", "hub_to_command")
    talk_to("osei", "I need the stores opened")
    assert w.flags.get("stores_open")

    bot.tap("wait")                             # sit out the afternoon while the chief works
    assert bot.scene.name == "wait"
    bot.scene.menu.index = 2                    # six hours
    bot.tap("confirm", 2)
    for _ in range(4000):
        if bot.scene.name == "play":
            break
        bot.frames(1)
    assert bot.scene.name == "play" and w.clock.hour >= 14
    assert w.quests.status("q_reactor") == "done" and w.systems.get("reactor").condition > 85
    assert w.player.inventory.has("tool_belt"), "Bram's thanks"
    assert w.script.errors == []


def test_verdict_at_the_airlock_ends_the_game_through_the_interface(bot):
    """Day six: the airlock, the captain, a verdict, the ending slides, the title."""
    w = bot.world
    w.fast_forward(w.clock.to_minutes(6, 8) - w.clock.minutes + 30)
    w.player.needs["hunger"] = w.player.needs["fatigue"] = 0.0
    w.player.hp = float(w.player.max_hp)
    w.requests.clear()                           # five neglected days: forget the fainting fits
    w.transfer(w.player, "hangar", 15, 20)
    bot.frames(5)
    bot.close_overlays()
    assert w.quests.stage("q_audit") == "final"
    assert bot.go_to_object("airlock")
    bot.interact()
    assert bot.in_dialogue and bot.choose("Go through")
    for _ in range(200):                         # the scene changes hands to the captain
        if bot.in_dialogue and bot.scene.runner.id == "verdict":
            break
        bot.frames(1)
    assert bot.in_dialogue and bot.scene.runner.id == "verdict"
    assert bot.choose("It cannot be saved")
    for _ in range(600):
        if bot.scene.name == "ending":
            break
        if bot.in_dialogue:
            bot.end_dialogue()
        bot.frames(1)
    assert bot.scene.name == "ending" and w.ended == "decommission"
    for _ in range(200):
        if bot.scene.name != "ending":
            break
        bot.tap("confirm", 3)
    assert bot.scene.name == "title"


def test_real_keyboard_events_drive_the_game(content):
    """The other tests inject actions; this one sends the events a real
    keyboard produces, through the same door the window uses."""
    from game.engine.app import App
    from game.scenes.title import TitleScene
    app = App(headless=True, audio=False, content=content, persist_options=False)
    app.push_scene(TitleScene(app))

    def frame(*events):
        app.frame_step(1 / 30.0, list(events))

    def down(k, text=""):
        events = [pygame.event.Event(pygame.KEYDOWN, key=k, mod=0, unicode=text)]
        if text:
            events.append(pygame.event.Event(pygame.TEXTINPUT, text=text))
        frame(*events)

    def up(k):
        frame(pygame.event.Event(pygame.KEYUP, key=k, mod=0))

    def key(k, text=""):
        down(k, text)
        up(k)
        frame()

    frame()
    key(pygame.K_RETURN)                                        # "New game"
    assert app.scene.name == "newgame"
    for _ in range(20):
        key(pygame.K_BACKSPACE)
    for ch in "Rex Quaid":          # r, e, x, q, a, i, d and space are all bound to actions as well
        key(pygame.K_SPACE if ch == " " else pygame.key.key_code(ch.lower()), ch)
    scene = app.scene
    assert scene.name == "newgame" and scene.step == 0 and scene.entry.value == "Rex Quaid"
    key(pygame.K_RETURN)
    assert scene.step == 1
    key(pygame.K_DOWN)
    key(pygame.K_RETURN)                                        # the second background
    assert scene.step == 2
    for _ in range(8):
        key(pygame.K_DOWN)
    key(pygame.K_RETURN)                                        # "Begin"
    for _ in range(90):
        if app.scene.name == "dialogue":
            break
        frame()
    assert app.scene.name == "dialogue", "the intro plays"
    world = app.scenes[0].world
    player = world.player
    assert player.name == "Rex Quaid" and player.background == "medic"
    world.characters["halloran"].goals = [g for g in world.characters["halloran"].goals if g["id"] != "greet_auditor"]
    for _ in range(120):                                        # space bar through the intro
        if app.scene.name == "play":
            break
        key(pygame.K_SPACE)
    assert app.scene.name == "play"

    start = (player.x, player.y)
    down(pygame.K_UP)                                           # hold an arrow key: walk
    for _ in range(25):
        frame()
    up(pygame.K_UP)
    for _ in range(10):
        frame()
    assert player.y < start[1] and player.x == start[0]
    here = (player.x, player.y)
    down(pygame.K_d)                                            # WASD works too
    for _ in range(25):
        frame()
    up(pygame.K_d)
    for _ in range(10):
        frame()
    assert player.x > here[0] and not player.moving

    key(pygame.K_ESCAPE)
    assert app.scene.name == "datapad"
    key(pygame.K_ESCAPE)                                        # the same key closes it, and it stays closed
    frame()
    assert app.scene.name == "play"
    key(pygame.K_TAB)
    assert app.scene.name == "datapad"
    key(pygame.K_x)
    assert app.scene.name == "play"
    key(pygame.K_i)
    assert app.scene.name == "datapad" and type(app.scene.tab).__name__ == "ItemsTab"
    key(pygame.K_r)                                             # next tab
    assert type(app.scene.tab).__name__ == "JournalTab"
    key(pygame.K_BACKSPACE)
    assert app.scene.name == "play"
    key(pygame.K_t)
    assert app.scene.name == "wait"
    key(pygame.K_ESCAPE)
    assert app.scene.name == "play"
    key(pygame.K_F5)
    assert save.read_header("quicksave")["name"] == "Rex Quaid"
    frame(pygame.event.Event(pygame.QUIT))
    assert not app.running
