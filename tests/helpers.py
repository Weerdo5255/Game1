"""Helpers for driving the story at the logic level (no rendering)."""
from game.story.dialogue import DialogueRunner


def read(runner, limit=200):
    """Advance pages until the runner offers choices or ends; returns the choices."""
    for _ in range(limit):
        if runner.finished:
            return []
        choices = runner.choices()
        if choices:
            return choices
        runner.advance()
    raise AssertionError("dialogue %s did not settle (node %s)" % (runner.id, runner.node_id))


def talk(world, dialogue_id, picks=(), npc_id=None, node=None, obj_id=""):
    """Run a dialogue, choosing in order the options whose text contains each
    string in ``picks``.  After the picks are used up the conversation is
    walked to its end by taking the last enabled option each time.
    Returns every line of text that was shown."""
    if npc_id is None and dialogue_id in world.characters:
        npc_id = dialogue_id
    runner = DialogueRunner(world, dialogue_id, npc_id or "", obj_id, node).start()
    seen = []
    picks = list(picks)
    skipped = 0
    for _ in range(300):
        if runner.finished:
            break
        seen.append(runner.text)
        choices = read(runner)
        seen.append(runner.text)
        if runner.finished:
            break
        if picks:
            want = picks[0]
            index = next((i for i, c in enumerate(choices) if want.lower() in c.text.lower()), None)
            if index is None:
                # not offered here (an introduction, say): move on towards the topic hub
                skipped += 1
                if skipped > 6:
                    raise AssertionError("no choice containing %r at %s/%s; offered: %s" % (
                        want, dialogue_id, runner.node_id, [c.text for c in choices]))
                enabled = [i for i, c in enumerate(choices) if c.enabled]
                runner.choose(enabled[-1])
                continue
            if not choices[index].enabled:
                raise AssertionError("choice %r is disabled at %s/%s" % (want, dialogue_id, runner.node_id))
            picks.pop(0)
            runner.choose(index)
        else:
            enabled = [i for i, c in enumerate(choices) if c.enabled]
            runner.choose(enabled[-1])
    else:
        raise AssertionError("dialogue %s never ended" % dialogue_id)
    if picks:
        raise AssertionError("dialogue %s ended before choosing %r" % (dialogue_id, picks))
    world.events.pump()
    return [s for s in seen if s]


def place_player(world, map_id, x, y):
    world.transfer(world.player, map_id, x, y)
    world.requests.clear()


def run_hours(world, hours):
    world.fast_forward(hours * 60.0)
    requests = list(world.requests)
    world.requests.clear()
    return requests
