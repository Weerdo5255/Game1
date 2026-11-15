import random

import pytest

from game.story.dialogue import DialogueRunner
from tests.helpers import read, talk

MINI = {
    "start": [{"if": "flag('seen')", "goto": "again"}, {"goto": "first"}],
    "nodes": {
        "first": {"text": ["Page one.", "Page two for $PLAYER, skill {skill('tech')}."],
                  "do": ["set_flag('seen')"],
                  "choices": [
                      {"text": "Smart option", "skill": ["tech", 99], "goto": "smart"},
                      {"text": "Roll for it", "check": ["persuasion", 10], "pass": "won", "fail": "lost",
                       "pass_do": ["set_flag('won')"], "fail_do": ["set_flag('lost')"]},
                      {"text": "Pay up", "cost": 50, "goto": "paid"},
                      {"text": "Hand over a ration", "item": ["ration_pack", 1], "consume": True, "goto": "gave"},
                      {"text": "Hidden", "if": "flag('never')", "goto": "end"},
                      {"text": "Only once", "once": True, "goto": "first"},
                      {"text": "Leave", "goto": "end"}]},
        "again": {"branch": [{"if": "flag('redirect')", "goto": "redirected"}],
                  "variants": [{"if": "credits() > 1000", "text": "Rich."}, {"text": "Welcome back."}],
                  "next": "end"},
        "redirected": {"speaker": "narrator", "text": "Elsewhere."},
        "smart": {"text": "Clever."}, "won": {"text": "You win."}, "lost": {"text": "You lose."},
        "paid": {"text": "Thanks."}, "gave": {"text": "Tasty."},
    },
}


@pytest.fixture
def mini(world):
    world.content.dialogues["_mini"] = MINI
    yield world
    del world.content.dialogues["_mini"]


def test_pages_text_substitution_and_effects(mini):
    w = mini
    runner = DialogueRunner(w, "_mini", "bram").start()
    assert runner.text == "Page one." and runner.choices() == []
    runner.advance()
    assert runner.text == "Page two for Tester, skill %d." % w.player.skill("tech")
    assert w.flags["seen"] is True
    assert runner.speaker()["name"] == "Bram Kowal"


def test_choice_gating(mini):
    w = mini
    w.player.credits = 10
    choices = {c.text: c for c in read(DialogueRunner(w, "_mini", "bram").start())}
    assert "Hidden" not in choices
    assert not choices["Smart option"].enabled and choices["Smart option"].tag == "Tech 99"
    assert not choices["Pay up"].enabled and "50 cr" in choices["Pay up"].tag
    assert choices["Hand over a ration"].enabled and "Ration Pack" in choices["Hand over a ration"].tag
    assert choices["Roll for it"].tag.startswith("Persuasion ") and choices["Roll for it"].tag.endswith("%")


def test_item_cost_and_once(mini):
    w = mini
    rations = w.player.inventory.count("ration_pack")
    talk(w, "_mini", ["Only once", "Hand over a ration"], npc_id="bram")
    assert w.player.inventory.count("ration_pack") == rations - 1
    w.flags.pop("seen")
    offered = [c.text for c in read(DialogueRunner(w, "_mini", "bram").start())]
    assert "Only once" not in offered
    w.player.credits = 80
    w.flags.pop("seen", None)
    talk(w, "_mini", ["Pay up"], npc_id="bram")
    assert w.player.credits == 30


def test_skill_check_outcomes_follow_the_dice(mini):
    w = mini
    outcomes = set()
    for seed in range(12):
        w.rng = random.Random(seed)
        w.flags.pop("seen", None)
        runner = DialogueRunner(w, "_mini", "bram").start()
        choices = read(runner)
        runner.choose(next(i for i, c in enumerate(choices) if c.text == "Roll for it"))
        assert runner.last_check["skill"] == "persuasion"
        outcomes.add((runner.last_check["success"], runner.node_id))
    assert outcomes == {(True, "won"), (False, "lost")}
    assert w.flags.get("won") and w.flags.get("lost")


def test_start_conditions_variants_and_branch(mini):
    w = mini
    w.flags["seen"] = True
    assert talk(w, "_mini", npc_id="bram") == ["Welcome back."]
    w.player.credits = 5000
    assert talk(w, "_mini", npc_id="bram") == ["Rich."]
    w.flags["redirect"] = True
    runner = DialogueRunner(w, "_mini", "bram").start()
    assert runner.text == "Elsewhere." and runner.speaker()["name"] == ""


def test_talking_marks_people_as_met(world):
    pip = world.characters["pip"]
    assert not pip.met and world.script.eval("talked('pip')") == 0
    talk(world, "pip")
    assert pip.met and world.script.eval("talked('pip')") == 1 and world.flags.get("met_pip")


def test_generic_dialogue_serves_any_character(world):
    mote = world.characters["mote"]
    lines = talk(world, "generic", ["What are you working on"], npc_id="mote")
    assert any(mote.name in line for line in lines)
    mote.wants["spare_parts"] = 1
    world.player.inventory.add("spare_parts", 1)
    before = mote.inventory.count("spare_parts")
    talk(world, "generic", ["Do you need anything", "Here, take this"], npc_id="mote")
    assert mote.inventory.count("spare_parts") == before + 1
    assert mote.opinion_of("player") > 0


@pytest.mark.parametrize("seed", range(6))
def test_random_walks_through_every_dialogue_raise_no_script_errors(content, seed):
    """Fuzz: wander through every conversation picking random options, in
    worlds at different stages, with script errors turned into exceptions."""
    from game.world.world import World
    rng = random.Random(seed)
    w = World.new_game(content, {"background": rng.choice(["engineer", "medic", "negotiator", "scout", "analyst"])},
                       seed=seed)
    w.requests.clear()
    w.script.strict = True
    if seed % 2:
        w.fast_forward(60 * rng.randint(4, 60))
        for flag in ("knows_signal", "knows_amp", "knows_dex_scheme", "briefed", "audit_authority",
                     "tuck_confided", "seen_artifact", "signal_decoded", "comms_fixed"):
            if rng.random() < 0.5:
                w.flags[flag] = True
        for quest in content.quests:
            if rng.random() < 0.6:
                w.quests.start(quest, w.clock.minutes)
        for item in ("pump_seal", "signal_amplifier", "spare_parts", "sedative", "mining_charge", "cargo_manifest"):
            if rng.random() < 0.5:
                w.player.inventory.add(item, 2)
        w.player.credits = 300
    for _ in range(3):
        for dialogue_id in sorted(content.dialogues):
            if w.ended:
                return
            npc_id = dialogue_id if dialogue_id in w.characters else ("mote" if dialogue_id == "generic" else "")
            if npc_id and not w.characters[npc_id].present:
                continue
            runner = DialogueRunner(w, dialogue_id, npc_id).start()
            for _step in range(80):
                if runner.finished:
                    break
                choices = read(runner)
                if runner.finished:
                    break
                enabled = [i for i, c in enumerate(choices) if c.enabled]
                runner.choose(rng.choice(enabled))
            w.requests.clear()
            w.events.pump()
    assert w.script.errors == []
