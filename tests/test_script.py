import pytest

from game.story.script import ScriptEngine, ScriptError


@pytest.fixture
def engine():
    e = ScriptEngine()
    store = {"a": 1}
    e.register("flag", lambda name, default=False: store.get(name, default), "cond")
    e.register("set_flag", lambda name, value=True: store.__setitem__(name, value))
    e.store = store
    return e


def test_expressions(engine):
    assert engine.eval("flag('a') == 1 and not flag('b')") is True
    assert engine.eval("3 > 2 >= 2 and 'x' in ['x', 'y']") is True
    assert engine.eval("(2 + 3) * 4 - 10 // 3") == 17
    assert engine.eval("'yes' if flag('a') else 'no'") == "yes"
    assert engine.eval("max(1, round(2.6)) + len('abc')") == 6
    assert engine.eval("") is True and engine.eval(None) is True


def test_statements_and_if(engine):
    engine.run(["set_flag('b', 5)", "if flag('b') > 3: set_flag('c')", "if flag('zzz'): set_flag('never')"])
    assert engine.store["b"] == 5 and engine.store["c"] is True and "never" not in engine.store


@pytest.mark.parametrize("bad", [
    "__import__('os').system('echo hi')",
    "().__class__.__bases__",
    "flag.__globals__",
    "[x for x in range(3)]",
    "(lambda: 1)()",
    "open('/etc/passwd')",
    "flag('a')[0]",
])
def test_sandbox_blocks_python(engine, bad):
    assert engine.eval(bad, default="blocked") == "blocked"
    assert engine.errors
    engine.strict = True
    with pytest.raises(ScriptError):
        engine.eval(bad)


def test_errors_do_not_propagate(engine):
    assert engine.eval("1 / 0", default=-1) == -1
    engine.run("set_flag(")            # syntax error is recorded, not raised
    assert len(engine.errors) == 2


def test_format(engine):
    assert engine.format("a={flag('a')} {{literal}} {1 + 1}") == "a=1 {literal} 2"
    assert engine.format("half {5 / 2} whole {4 / 2}") == "half 2.5 whole 2"


def test_static_check(engine):
    assert engine.check("flag('a') and npc == 'x'") == []
    assert any("unknown function" in p for p in engine.check("nope(1)"))
    assert any("unknown name" in p for p in engine.check("mystery > 2"))
    assert any("not allowed" in p for p in engine.check("flag.__class__"))
    assert engine.check("if flag('a'): set_flag('b')", statement=True) == []
    assert engine.check_format("ok {flag('a')} bad {nope()}")
