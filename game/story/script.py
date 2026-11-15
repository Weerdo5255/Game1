"""A small, safe scripting language for content files.

Dialogue conditions, quest triggers, item effects and NPC goals are written
as Python-looking expressions inside JSON, for example::

    "if": "skill('tech') >= 4 and not flag('lab_hacked')"
    "do": ["set_flag('lab_hacked')", "add_xp(40)", "add_rel(-10, 'lin')"]

They are *not* run with ``eval``.  The text is parsed into a syntax tree and
interpreted here, allowing only literals, comparisons, boolean logic,
arithmetic and calls to the functions registered in ``story/api.py``.  There
is no attribute access, no subscripting and no way to reach Python builtins,
so content files (and mods) cannot execute arbitrary code.

Names available inside scripts: ``npc`` (the character being talked to),
``self`` (the character running a goal step), ``obj`` (the map object being
used), ``player`` and ``item``.  They evaluate to id strings.
"""
from __future__ import annotations

import ast
import operator
from typing import Any, Callable, Dict, List, Optional, Sequence, Union

Statements = Union[str, Sequence[str], None]


class ScriptError(Exception):
    pass


_BIN_OPS = {
    ast.Add: operator.add, ast.Sub: operator.sub, ast.Mult: operator.mul,
    ast.Div: operator.truediv, ast.FloorDiv: operator.floordiv, ast.Mod: operator.mod,
}
_CMP_OPS = {
    ast.Eq: operator.eq, ast.NotEq: operator.ne, ast.Lt: operator.lt, ast.LtE: operator.le,
    ast.Gt: operator.gt, ast.GtE: operator.ge,
    ast.In: lambda a, b: a in b, ast.NotIn: lambda a, b: a not in b,
}
_PURE = {
    "min": min, "max": max, "abs": abs, "round": round, "int": int, "float": float,
    "str": str, "len": len, "bool": bool,
}
CONTEXT_NAMES = ("npc", "self", "obj", "player", "item", "map")


def script_fn(kind: str = "effect"):
    """Mark an API method as callable from scripts.

    ``kind`` is documentation only: "cond" (a getter usable in conditions),
    "effect" (changes the world) or "text" (returns display text).
    """
    def deco(fn):
        fn.__script_kind__ = kind
        return fn
    return deco


class ScriptEngine:
    def __init__(self, world=None):
        self.world = world
        self.funcs: Dict[str, Callable] = {}
        self.kinds: Dict[str, str] = {}
        self._expr_cache: Dict[str, ast.AST] = {}
        self._stmt_cache: Dict[str, List[ast.stmt]] = {}
        self._ctx: List[Dict[str, Any]] = [{}]
        self.errors: List[str] = []          # recent runtime problems (for debugging)
        self.strict = False                  # raise instead of logging (used by tests)
        for name, fn in _PURE.items():
            self.funcs[name] = fn
            self.kinds[name] = "cond"

    # -- registration ------------------------------------------------------------
    def register(self, name: str, fn: Callable, kind: str = "effect") -> None:
        self.funcs[name] = fn
        self.kinds[name] = kind

    def register_object(self, api) -> None:
        """Register every method of ``api`` decorated with ``@script_fn``."""
        for name in dir(api):
            if name.startswith("_"):
                continue
            fn = getattr(api, name)
            kind = getattr(fn, "__script_kind__", None)
            if kind:
                self.register(name, fn, kind)

    # -- context -----------------------------------------------------------------
    @property
    def ctx(self) -> Dict[str, Any]:
        return self._ctx[-1]

    def _push(self, ctx: Optional[dict]) -> None:
        merged = dict(self._ctx[-1]) if len(self._ctx) > 1 else {}
        if ctx:
            merged.update(ctx)
        self._ctx.append(merged)

    def _pop(self) -> None:
        if len(self._ctx) > 1:
            self._ctx.pop()

    # -- parsing -----------------------------------------------------------------
    def _parse_expr(self, text: str) -> ast.AST:
        node = self._expr_cache.get(text)
        if node is None:
            try:
                node = ast.parse(text.strip(), mode="eval").body
            except SyntaxError as exc:
                raise ScriptError("syntax error in %r: %s" % (text, exc.msg))
            self._expr_cache[text] = node
        return node

    def _parse_stmts(self, text: str) -> List[ast.stmt]:
        body = self._stmt_cache.get(text)
        if body is None:
            try:
                body = ast.parse(text.strip(), mode="exec").body
            except SyntaxError as exc:
                raise ScriptError("syntax error in %r: %s" % (text, exc.msg))
            self._stmt_cache[text] = body
        return body

    # -- public API ----------------------------------------------------------------
    def eval(self, expr: Any, ctx: Optional[dict] = None, default: Any = False) -> Any:
        """Evaluate a condition/expression.  Empty or missing means True.

        Runtime errors are recorded in ``errors`` and ``default`` is returned,
        so one broken condition in a content file cannot crash the game.
        """
        if expr is None or expr == "":
            return True
        if isinstance(expr, bool):
            return expr
        if isinstance(expr, (int, float)):
            return expr
        self._push(ctx)
        try:
            return self._ev(self._parse_expr(str(expr)))
        except ScriptError as exc:
            self._fail("%s  [in: %s]" % (exc, expr))
            return default
        except Exception as exc:  # noqa: BLE001 - content must never crash the game
            self._fail("%s: %s  [in: %s]" % (type(exc).__name__, exc, expr))
            return default
        finally:
            self._pop()

    def run(self, statements: Statements, ctx: Optional[dict] = None) -> None:
        """Execute effect statements (a string or a list of strings)."""
        if not statements:
            return
        if isinstance(statements, str):
            statements = [statements]
        self._push(ctx)
        try:
            for text in statements:
                try:
                    for stmt in self._parse_stmts(str(text)):
                        self._exec(stmt)
                except ScriptError as exc:
                    self._fail("%s  [in: %s]" % (exc, text))
                except Exception as exc:  # noqa: BLE001
                    self._fail("%s: %s  [in: %s]" % (type(exc).__name__, exc, text))
        finally:
            self._pop()
        # deliver any events the effects raised once the outermost script is done
        if len(self._ctx) == 1 and self.world is not None:
            self.world.events.pump()

    def format(self, text: str, ctx: Optional[dict] = None) -> str:
        """Substitute ``{expression}`` placeholders inside display text."""
        if "{" not in text:
            return text
        out: List[str] = []
        i, n = 0, len(text)
        while i < n:
            ch = text[i]
            if ch == "{":
                if text.startswith("{{", i):
                    out.append("{")
                    i += 2
                    continue
                end = text.find("}", i)
                if end == -1:
                    out.append(text[i:])
                    break
                value = self.eval(text[i + 1:end], ctx, default="?")
                if isinstance(value, float):
                    value = ("%.1f" % value).rstrip("0").rstrip(".")
                out.append(str(value))
                i = end + 1
                continue
            if ch == "}" and text.startswith("}}", i):
                out.append("}")
                i += 2
                continue
            out.append(ch)
            i += 1
        return "".join(out)

    def check(self, text: Any, statement: bool = False) -> List[str]:
        """Static problems in a script (syntax, unknown functions/names)."""
        if text is None or text == "" or isinstance(text, (bool, int, float)):
            return []
        problems: List[str] = []
        try:
            tree = ast.parse(str(text).strip(), mode="exec" if statement else "eval")
        except SyntaxError as exc:
            return ["syntax error: %s" % exc.msg]
        called = {id(n.func) for n in ast.walk(tree) if isinstance(n, ast.Call)}
        for node in ast.walk(tree):
            if isinstance(node, ast.Call):
                if not isinstance(node.func, ast.Name):
                    problems.append("only plain function calls are allowed")
                elif node.func.id not in self.funcs:
                    problems.append("unknown function %s()" % node.func.id)
            elif isinstance(node, ast.Name):
                if id(node) not in called and node.id not in CONTEXT_NAMES:
                    problems.append("unknown name %r" % node.id)
            elif isinstance(node, (ast.Attribute, ast.Subscript, ast.Lambda, ast.Dict,
                                   ast.ListComp, ast.GeneratorExp, ast.Await, ast.Starred)):
                problems.append("%s is not allowed in scripts" % type(node).__name__)
        return problems

    def check_format(self, text: str) -> List[str]:
        problems: List[str] = []
        i = 0
        while True:
            i = text.find("{", i)
            if i == -1:
                break
            if text.startswith("{{", i):
                i += 2
                continue
            end = text.find("}", i)
            if end == -1:
                problems.append("unclosed '{'")
                break
            problems += self.check(text[i + 1:end])
            i = end + 1
        return problems

    def _fail(self, message: str) -> None:
        if self.strict:
            raise ScriptError(message)
        self.errors.append(message)
        if len(self.errors) > 200:
            del self.errors[:100]

    # -- interpreter ---------------------------------------------------------------
    def _exec(self, stmt: ast.stmt) -> None:
        if isinstance(stmt, ast.Expr):
            self._ev(stmt.value)
        elif isinstance(stmt, ast.If):
            branch = stmt.body if self._ev(stmt.test) else stmt.orelse
            for s in branch:
                self._exec(s)
        elif isinstance(stmt, ast.Pass):
            pass
        else:
            raise ScriptError("statement type %s is not allowed" % type(stmt).__name__)

    def _ev(self, node: ast.AST) -> Any:
        t = type(node)
        if t is ast.Constant:
            return node.value
        if t is ast.Name:
            name = node.id
            if name in CONTEXT_NAMES:
                if name == "player":
                    return "player"
                return self.ctx.get(name, "")
            raise ScriptError("unknown name %r" % name)
        if t is ast.Call:
            if not isinstance(node.func, ast.Name):
                raise ScriptError("only plain function calls are allowed")
            fn = self.funcs.get(node.func.id)
            if fn is None:
                raise ScriptError("unknown function %s()" % node.func.id)
            args = [self._ev(a) for a in node.args]
            kwargs = {k.arg: self._ev(k.value) for k in node.keywords if k.arg}
            return fn(*args, **kwargs)
        if t is ast.BoolOp:
            if isinstance(node.op, ast.And):
                value: Any = True
                for v in node.values:
                    value = self._ev(v)
                    if not value:
                        return value
                return value
            value = False
            for v in node.values:
                value = self._ev(v)
                if value:
                    return value
            return value
        if t is ast.UnaryOp:
            operand = self._ev(node.operand)
            if isinstance(node.op, ast.Not):
                return not operand
            if isinstance(node.op, ast.USub):
                return -operand
            if isinstance(node.op, ast.UAdd):
                return +operand
            raise ScriptError("operator not allowed")
        if t is ast.BinOp:
            op = _BIN_OPS.get(type(node.op))
            if op is None:
                raise ScriptError("operator not allowed")
            left, right = self._ev(node.left), self._ev(node.right)
            if isinstance(left, str) != isinstance(right, str) and isinstance(node.op, ast.Add):
                return str(left) + str(right)
            return op(left, right)
        if t is ast.Compare:
            left = self._ev(node.left)
            for op_node, comp in zip(node.ops, node.comparators):
                op = _CMP_OPS.get(type(op_node))
                if op is None:
                    raise ScriptError("comparison not allowed")
                right = self._ev(comp)
                if not op(left, right):
                    return False
                left = right
            return True
        if t is ast.IfExp:
            return self._ev(node.body) if self._ev(node.test) else self._ev(node.orelse)
        if t in (ast.List, ast.Tuple):
            return [self._ev(e) for e in node.elts]
        raise ScriptError("%s is not allowed in scripts" % t.__name__)
