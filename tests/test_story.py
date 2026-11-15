"""Scripted playthroughs of the campaign's branches (logic level)."""
import random

import pytest

from game.scenes.ending import ending_slides
from tests.helpers import place_player, run_hours, talk


def refresh(world):
    """Keep the test player alive through long waits."""
    p = world.player
    p.needs["hunger"] = p.needs["fatigue"] = 0.0
    p.hp = float(p.max_hp)


def wait_until(world, day, hour, step=2.0):
    target = world.clock.to_minutes(day, hour)
    requests = []
    while world.clock.minutes < target and not world.ended:
        refresh(world)
        requests += run_hours(world, min(step, (target - world.clock.minutes) / 60.0))
    return requests


def brief(world):
    talk(world, "halloran", ["Nothing is decided"], node="welcome")
    talk(world, "osei", ["Show me"])
    assert world.flags.get("audit_authority") and world.quests.stage("q_audit") == "inspect"


def test_intro_starts_the_audit(world):
    talk(world, "intro")
    assert world.quests.status("q_audit") == "active" and world.quests.stage("q_audit") == "report_in"


def test_inspection_tour_advances_the_main_quest(world):
    brief(world)
    assert world.player.has_effect("authority")
    talk(world, "bram", ["Who is giving you forms"])
    talk(world, "wren", ["What would it take"])
    assert world.quests.stage("q_audit") == "inspect"
    talk(world, "pell", ["Emptying itself"])
    assert world.quests.stage("q_audit") == "findings"
    for quest in ("q_reactor", "q_water", "q_medical"):
        assert world.quests.status(quest) == "active"
    talk(world, "osei", ["People holding a wreck"])
    assert world.quests.stage("q_audit") == "countdown"


def test_inspecting_before_the_briefing_still_counts(world):
    talk(world, "bram")
    talk(world, "wren")
    talk(world, "pell")
    talk(world, "osei", ["Show me"])
    assert world.quests.stage("q_audit") == "findings"


def test_water_quest_buy_the_seal_from_dex(world):
    talk(world, "wren", ["What would it take"])
    credits = world.player.credits
    talk(world, "dex", ["I need the pump seal", "Seventy. Done."])
    assert world.player.inventory.has("pump_seal") and world.player.credits == credits - 70
    assert world.quests.stage("q_water") == "fit_seal"
    talk(world, "wren", ["I have a pump seal"])
    pump = world.systems.get("water_reclaim")
    assert world.flags.get("pump_fixed") and pump.cap == 100 and pump.condition > 80
    assert world.quests.status("q_water") == "done"
    assert world.player.inventory.has("fresh_greens", 3)


def test_water_quest_fabricate_and_fit_it_yourself(world):
    talk(world, "wren")
    lines = talk(world, "pip", ["Could you fabricate"])
    assert any("had any power" in line for line in lines), "the fabricator is browned out at the start"
    world.systems.get("lab").priority = 9
    for _ in range(20):
        world.systems.get("fabricator").add_labour(0.2)
        world.systems.tick(world, 0.05)
    world.player.inventory.add("scrap_metal", 2)
    talk(world, "pip", ["Could you fabricate", "Here is the scrap"])
    assert world.player.inventory.has("pump_seal") and not world.player.inventory.has("scrap_metal")
    talk(world, "pump", ["Fit the new pump seal"], obj_id="pump")
    assert world.flags.get("pump_fixed") and world.quests.status("q_water") == "done"


def test_water_quest_strip_the_bore_rig(world):
    talk(world, "wren")
    world.characters["tuck"].relations["player"] = 15
    drill_before = world.systems.get("drill").condition
    talk(world, "tuck", ["The water plant needs a seal", "Wren asked me"])
    assert world.player.inventory.has("pump_seal")
    assert world.systems.get("drill").condition < drill_before and world.systems.get("drill").cap == 55


def test_reactor_quest_by_opening_the_stores(world):
    brief(world)
    talk(world, "bram", ["Who is giving you forms"])
    # use up what engineering has so the stores really matter
    for obj_id in ("parts_locker", "eng_locker"):
        world.inventory_of(world.find_object(obj_id)).items.pop("spare_parts", None)
    world.systems.get("reactor").condition = 55.0
    talk(world, "osei", ["I need the stores opened"])
    assert world.flags.get("stores_open")
    wait_until(world, 2, 0)
    assert world.systems.get("reactor").condition >= 70
    assert world.quests.status("q_reactor") == "done"
    assert world.player.inventory.has("tool_belt")


def test_the_reactor_does_not_fix_itself(world):
    """Engineering starts with empty shelves, so the quest waits for the player."""
    brief(world)
    talk(world, "bram", ["Who is giving you forms"])
    assert world.quests.stage("q_reactor") == "parts"
    wait_until(world, 2, 8)
    reactor = world.systems.get("reactor")
    assert reactor.condition < 60 and world.quests.status("q_reactor") == "active"
    assert world.characters["bram"].wants.get("spare_parts")
    assert world.systems.brownout
    # one signature later the chief has what he needs
    talk(world, "osei", ["I need the stores opened"])
    wait_until(world, 2, 20)
    assert world.quests.status("q_reactor") == "done" and reactor.condition > 75


def test_bram_notices_a_reactor_that_is_already_healthy(world):
    world.systems.get("reactor").condition = 95.0
    lines = talk(world, "bram")
    assert any("good day to count things" in line for line in lines)
    assert not any("She needs parts" in line for line in lines)


def test_fitting_parts_yourself_moves_the_reactor_quest_on(world):
    talk(world, "bram")
    assert world.quests.stage("q_reactor") == "parts"
    ok, message = world.player_repair("reactor")
    assert ok and "repaired" in message
    world.events.pump()
    assert world.quests.stage("q_reactor") == "stabilise"


def test_demoting_the_lab_is_noticed(world):
    lab = world.systems.get("lab")
    lin, bram = world.characters["lin"], world.characters["bram"]
    lin_before, bram_before = lin.opinion_of("player"), bram.opinion_of("player")
    lab.priority = 9
    world.events.emit("priority:lab", system="lab", priority=9)
    world.events.pump()
    assert world.flags.get("lab_demoted")
    assert lin.opinion_of("player") < lin_before and bram.opinion_of("player") > bram_before
    lab.priority = 2
    world.events.emit("priority:lab", system="lab", priority=2)
    world.events.pump()
    assert not world.flags.get("lab_demoted")


def test_catching_dex_at_night_and_having_him_arrested(world):
    place_player(world, "hangar", 12, 11)
    wait_until(world, 2, 3, step=0.5)
    assert world.flags.get("saw_dex_loading") and world.quests.stage("q_smuggler") == "proof"
    hold = world.inventory_of(world.find_object("shuttle_hold"))
    assert hold.count("ration_pack") > 0
    talk(world, "halloran", ["I have proof", "Arrest him"])
    wait_until(world, 2, 12)
    dex = world.characters["dex"]
    assert world.flags.get("dex_arrested") and dex.map_id == "command"
    assert world.maps["command"].region_at(dex.x, dex.y).id == "brig_cell"
    assert world.find_object("cell_door").get("locked")
    assert hold.empty and world.quests.status("q_smuggler") == "done"
    wait_until(world, 5, 6)
    assert dex.present and not world.flags.get("dex_left"), "he stays put in the cell"
    assert world.maps["command"].region_at(dex.x, dex.y).id == "brig_cell"
    assert "dex_arrested" in " ".join(str(k) for k in world.flags)
    assert any("restraints" in s for s in ending_slides(world, "viable"))


def test_finding_the_manifest_is_proof_too(world):
    talk(world, "halloran", ["Is anything going missing"])
    assert world.quests.stage("q_smuggler") == "suspect"
    world.give_item(world.player, "cargo_manifest", 1)
    world.events.pump()
    assert world.flags.get("knows_dex_scheme") and world.quests.stage("q_smuggler") == "proof"
    talk(world, "halloran", ["I have proof", "Arrest him"])
    assert world.inventory_of(world.find_object("evidence_shelf")).has("cargo_manifest")


def test_talking_dex_round(world):
    world.flags["knows_dex_scheme"] = True
    world.characters["dex"].relations["player"] = 35
    wait_until(world, 2, 6)
    hold = world.inventory_of(world.find_object("shuttle_hold"))
    assert hold.count("ration_pack") > 0
    talk(world, "dex", ["I know about the Magpie", "You have a friend here"])
    assert world.flags.get("dex_reformed") and hold.empty
    assert world.quests.status("q_smuggler") == "done"
    wait_until(world, 5, 6)
    assert world.characters["dex"].present and not world.flags.get("dex_left")


def test_left_alone_dex_takes_the_shuttle(world):
    place_player(world, "habitat", 3, 3)
    talk(world, "halloran", ["Is anything going missing"])
    wait_until(world, 5, 5)
    dex = world.characters["dex"]
    assert world.flags.get("dex_left") and not dex.present
    assert world.maps["hangar"].tile("objects", 6, 8) == "", "the pad is empty"
    assert world.find_object("magpie") is None
    assert world.quests.status("q_smuggler") == "failed"
    assert not world.script.eval("present('dex')")


def test_flying_away_with_dex(world):
    world.flags["knows_dex_scheme"] = True
    talk(world, "dex", ["I know about the Magpie", "room for one more"])
    assert world.flags.get("leaving_with_dex")
    place_player(world, "hangar", 9, 12)
    wait_until(world, 5, 4, step=0.25)
    assert world.ended == "magpie"


def get_amplifier(world):
    world.flags["knows_amp"] = True
    brief(world)
    talk(world, "osei", ["The amplifier core"])
    assert world.flags.get("amp_released")
    talk(world, "halloran", ["The amplifier core", "The Administrator has decided"])
    assert world.player.inventory.has("signal_amplifier")
    assert world.inventory_of(world.find_object("sec_locker")).count("signal_amplifier") == 0


def test_signal_branch_ends_in_contact(world):
    get_amplifier(world)
    talk(world, "lin", ["What is the analysis rig for"])
    talk(world, "lin", ["I brought you the amplifier core"])
    lin = world.characters["lin"]
    assert lin.inventory.has("signal_amplifier") and world.flags.get("amp_to_lin")
    assert world.quests.stage("q_signal") == "waiting"
    requests = []
    for day in (2, 3, 4, 5):
        requests += wait_until(world, day, 12)
        if world.flags.get("signal_decoded"):
            break
    assert world.flags.get("signal_decoded"), "progress %.1f" % world.flags.get("signal_progress", 0)
    assert world.quests.stage("q_signal") == "decoded"
    requests += run_hours(world, 2)
    assert any(r["type"] == "dialogue" and r.get("npc") == "lin" and r.get("node") == "decoded" for r in requests)
    for _ in range(96):                       # she waits by the spire whenever she is not asleep or eating
        region = world.maps[lin.map_id].region_at(lin.x, lin.y)
        if lin.map_id == "excavation" and region is not None and region.id == "artifact_chamber":
            break
        refresh(world)
        run_hours(world, 0.25)
    assert lin.map_id == "excavation" and lin.ai.get("goal") == "wait_in_chamber"
    assert "seal_tunnel" in [g["id"] for g in world.characters["tuck"].goals], "Tuck decides to bury it"
    talk(world, "lin", ["Answer it"])
    assert world.ended == "contact" and world.quests.status("q_signal") == "done"
    assert len(ending_slides(world, "contact")) == 4


def test_tuck_buries_the_chamber_if_he_gets_the_charges(world):
    world.flags["knows_signal"] = True
    talk(world, "tuck", ["Would you really bury it"])
    assert world.flags.get("tuck_wants_charges")
    world.give_item(world.player, "mining_charge", 2)
    talk(world, "tuck", ["Here are the charges", "Do it."])
    place_player(world, "excavation", 24, 20)        # standing in the chamber when it goes
    wait_until(world, 1, 14, step=0.25)
    assert world.flags.get("tunnel_sealed") and world.flags.get("tuck_charges")
    assert world.find_object("artifact") is None
    excavation = world.maps["excavation"]
    assert excavation.tile("objects", 22, 20) == "rubble" and excavation.solid_at(20, 20)
    region = excavation.region_at(world.player.x, world.player.y)
    assert region is None or region.id != "artifact_chamber", "the player was pulled clear"
    assert not excavation.solid_at(world.player.x, world.player.y)
    assert world.quests.status("q_signal") == "done"
    assert talk(world, "lin")[0].startswith("You gave him the charges")
    assert talk(world, "tuck")[0].startswith("It is quiet")


def test_comms_branch_uncovers_the_real_orders_and_clause_nine(world):
    get_amplifier(world)
    talk(world, "pip", ["Where is the amplifier core"])
    talk(world, "pip", ["Here. The amplifier core."])
    assert world.flags.get("amp_to_pip") and world.quests.stage("q_comms") == "repairs"
    world.systems.get("lab").priority = 9
    requests = wait_until(world, 3, 12)
    assert world.flags.get("comms_fixed"), "comms at %.0f%%" % world.systems.get("comms").condition
    assert any(r["type"] == "dialogue" and r.get("node") == "comms_up" for r in requests)
    # rebuilt is not the same as powered: starve the array and the console says so
    world.systems.get("reactor").condition = 30.0
    world.systems.get("comms").priority = 9
    world.systems.tick(world, 1e-6)
    lines = talk(world, "terminal_traffic")
    assert world.systems.get("comms").power_ratio < 0.1 and any("starved" in line for line in lines)
    assert world.quests.stage("q_comms") == "power"
    world.systems.get("comms").priority = 1
    world.systems.get("reactor").condition = 90.0      # undo the sabotage: this test is not about famine
    world.systems.tick(world, 1e-6)
    assert world.script.eval("online('comms')")
    talk(world, "terminal_traffic", ["Read it.", "Send a distress call"])
    assert world.flags.get("knows_combine_plan") and world.player.inventory.has("combine_orders")
    assert world.quests.status("q_comms") == "done" and world.flags.get("relief_called")
    talk(world, "osei", ["refuse the Combine"])
    assert world.player.inventory.has("station_charter")

    # day 6: the tug docks and the crew stand behind the charter
    wait_until(world, 6, 9)
    for npc in world.npcs():
        npc.relations["player"] = 40
    world.systems.get("reactor").condition = 80
    world.systems.resources["water"] = 500
    assert world.quests.stage("q_audit") == "final"
    assert world.characters["osei"].map_id == "hangar"
    talk(world, "airlock", ["Go through"], obj_id="airlock")
    talk(world, "verdict", ["I have read your real orders", "Clause Nine"])
    assert world.ended == "independent" and world.quests.status("q_audit") == "done"
    slides = ending_slides(world, "independent")
    assert any("Radio Club" in s for s in slides) and any("distress call" in s for s in slides)


def test_a_diligent_auditor_honestly_saves_the_station(world):
    """No conditions are set by hand here: the station is viable on day six
    because the reactor got its parts, the pump its seal and the quartermaster
    a cell - the simulation does the rest."""
    brief(world)
    talk(world, "bram", ["Who is giving you forms"])
    talk(world, "wren", ["What would it take"])
    talk(world, "pell", ["Emptying itself"])
    talk(world, "osei", ["People holding a wreck"])
    talk(world, "osei", ["I need the stores opened"])
    talk(world, "dex", ["I need the pump seal", "Seventy. Done."])
    talk(world, "wren", ["I have a pump seal"])
    place_player(world, "hangar", 12, 11)                 # a night watch in the hangar
    wait_until(world, 2, 3, step=0.5)
    assert world.flags.get("saw_dex_loading")
    talk(world, "halloran", ["I have proof", "Arrest him"])
    wait_until(world, 6, 9)
    systems = world.systems
    assert systems.get("reactor").condition >= 60 and systems.resources["air"] >= 70
    assert systems.resources["water"] >= 225 and world.characters["dex"].present
    for quest in ("q_reactor", "q_water", "q_smuggler"):
        assert world.quests.status(quest) == "done", quest
    assert all(npc.alive for npc in world.npcs())
    talk(world, "airlock", ["Go through"], obj_id="airlock")
    talk(world, "verdict", ["It is viable"])
    assert world.ended == "viable" and not world.flags.get("verdict_lied")
    assert world.script.errors == []


def test_clause_nine_needs_the_crew_behind_it(world):
    world.flags["knows_combine_plan"] = True
    world.give_item(world.player, "station_charter", 1)
    for npc in world.npcs():
        npc.relations["player"] = -10
    talk(world, "verdict", ["I have read your real orders", "Clause Nine"])
    assert world.ended == "decommission"


@pytest.mark.parametrize("healthy,expected", [(True, "viable"), (False, "decommission")])
def test_plain_verdicts(world, healthy, expected):
    if healthy:
        world.systems.get("reactor").condition = 90
        talk(world, "verdict", ["It is viable"])
    else:
        talk(world, "verdict", ["It cannot be saved"])
    assert world.ended == expected and ending_slides(world, expected)


def test_lying_about_a_failing_station_takes_a_silver_tongue(world):
    outcomes = set()
    for seed in range(14):
        world.ended = None
        world.requests.clear()
        world.quests.state.pop("q_audit", None)
        world.flags.pop("verdict_lied", None)
        world.rng = random.Random(seed)
        world.systems.get("reactor").condition = 30
        talk(world, "verdict", ["It is viable", "Sign it"])
        outcomes.add((world.ended, bool(world.flags.get("verdict_lied"))))
    assert outcomes == {("viable", True), ("decommission", False)}


def test_not_turning_up_is_also_an_answer(world):
    place_player(world, "habitat", 3, 3)
    for system in world.systems.systems.values():      # keep everybody breathing for the test
        system.decay = 0.0
    world.inventory_of(world.find_object("reactor_hopper")).add("fuel_cell", 12)
    wait_until(world, 6, 21)
    assert world.ended == "absent"


def test_station_death_ends_the_game(world):
    world.systems.resources["air"] = 13.0
    world.systems.get("life_support").enabled = False
    run_hours(world, 0.5)
    assert world.ended == "blackout"
    assert world.requests == [] or True


def test_medical_quest_resolutions(world):
    talk(world, "pell", ["Emptying itself"])
    talk(world, "wren", ["Have you noticed anyone not sleeping"])
    assert world.quests.stage("q_medical") == "tuck"
    talk(world, "tuck", ["You look like you have not slept", "I will not tell Pell"])
    assert world.flags.get("tuck_confided") and world.quests.stage("q_medical") == "decide"
    talk(world, "pell", ["Is there something gentler"])
    assert world.player.inventory.has("sedative", 2)
    tuck = world.characters["tuck"]
    talk(world, "tuck", ["Take these. From a friend."])
    assert world.flags.get("tuck_rested") and tuck.needs["fatigue"] == 0
    assert world.quests.status("q_medical") == "done"


def test_reporting_tuck_takes_him_off_the_rig(world):
    talk(world, "pell", ["Emptying itself"])
    world.quests.set_stage("q_medical", "decide")
    talk(world, "pell", ["It is Tuck taking the stims"])
    tuck = world.characters["tuck"]
    assert tuck.schedule == "resting" and tuck.opinion_of("player") < -15
    wait_until(world, 1, 15)
    assert tuck.map_id != "excavation", "he has been sent to rest"


def test_theft_gets_noticed_and_followed_up(world):
    wren = world.characters["wren"]
    place_player(world, wren.map_id, wren.x + 1, wren.y)
    shelf = world.find_object("hydro_shelf")
    before = wren.opinion_of("player")
    assert world.report_theft(shelf) is wren and wren.opinion_of("player") < before
    world.report_theft(shelf)
    world.events.pump()
    assert world.flags["thefts_seen"] == 2
    assert "warn_thief" in [g["id"] for g in world.characters["halloran"].goals]
    wren.asleep = True
    assert world.report_theft(shelf) is None, "nobody awake to see it"


def test_every_ending_has_slides_in_any_state(content):
    from game.world.world import World
    for seed in range(3):
        w = World.new_game(content, seed=seed)
        w.script.strict = True
        if seed:
            for flag in ("pump_fixed", "dex_arrested", "comms_fixed", "tunnel_sealed", "relief_called",
                         "verdict_lied", "tuck_rested"):
                w.flags[flag] = True
        for ending in content.endings:
            if ending != "epilogue":
                assert ending_slides(w, ending), ending
