from __future__ import annotations

from dataclasses import dataclass
from typing import Callable

ASCII = frozenset(map(chr, range(128)))


@dataclass(frozen=True)
class Span:
    start: int
    end: int
    line: int = 1
    column: int = 1


class CompileError(ValueError):
    def __init__(self, message: str, span: Span | None = None, source_id: str = "source"):
        self.message = message
        self.span = span or Span(0, 0)
        self.source_id = source_id
        super().__init__(f"第 {self.span.line} 行，第 {self.span.column} 列：{message}")


class Cancelled(Exception):
    pass


class LimitExceeded(CompileError):
    pass


@dataclass(frozen=True)
class Limits:
    nfa_states: int = 512
    dfa_states: int = 256
    steps: int = 20_000


class Context:
    def __init__(self, limits: Limits, cancelled: Callable[[], bool] | None = None):
        self.limits = limits
        self.cancelled = cancelled or (lambda: False)
        self.steps = 0

    def check(self):
        if self.cancelled():
            raise Cancelled("构建已取消")

    def tick(self):
        self.check()
        self.steps += 1
        if self.steps > self.limits.steps:
            raise LimitExceeded(f"步骤超过 {self.limits.steps:,} 条，请缩小表达式")


@dataclass(frozen=True)
class AST:
    kind: str
    span: Span
    children: tuple[AST, ...] = ()
    chars: frozenset[str] = frozenset()
    name: str = ""


@dataclass(frozen=True)
class Edge:
    source: int
    target: int
    chars: frozenset[str] | None


@dataclass(frozen=True)
class NFA:
    states: tuple[int, ...]
    start: int
    accepts: frozenset[int]
    edges: tuple[Edge, ...]
    accepting_rules: tuple[tuple[int, tuple[RuleSpec, ...]], ...] = ()


@dataclass(frozen=True)
class DFAState:
    id: int
    nfa_states: frozenset[int]
    accepting: bool
    candidates: tuple[RuleSpec, ...] = ()
    members: frozenset[int] = frozenset()

    @property
    def winner(self):
        return self.candidates[0] if self.candidates else None


@dataclass(frozen=True)
class DFA:
    states: tuple[DFAState, ...]
    alphabet: tuple[frozenset[str], ...]
    # Row indexed by state, column by disjoint character group.
    transitions: tuple[tuple[int, ...], ...]
    start: int = 0


@dataclass(frozen=True)
class Step:
    kind: str
    message: str
    span: Span | None = None
    nodes: frozenset[int] = frozenset()
    edges: tuple[int, ...] = ()
    node_count: int = 0
    edge_count: int = 0
    source: int = -1
    group: int = -1
    target: int = -1
    # Compact persistent replay: number of committed table cells.
    committed: int = 0
    fragment: tuple[int, int] | None = None
    ast_occurrence: int = -1
    graph: NFA | None = None
    partitions: tuple[frozenset[int], ...] = ()
    merges: tuple[tuple[int, int], ...] = ()


@dataclass(frozen=True)
class Compilation:
    source: str
    ast: AST
    definitions: tuple[str, ...]
    entry: str | None
    nfa: NFA
    dfa: DFA
    nfa_steps: tuple[Step, ...]
    dfa_steps: tuple[Step, ...]


@dataclass(frozen=True)
class SimulationStep:
    position: int
    source: int
    target: int
    group: int
    message: str


@dataclass(frozen=True)
class SimulationResult:
    accepted: bool
    final_state: int
    steps: tuple[SimulationStep, ...]
    failure_position: int | None = None


@dataclass(frozen=True)
class RuleSpec:
    name: str
    pattern: str
    category: str = ""
    skip: bool = False
    enabled: bool = True

    @property
    def output(self):
        return self.category or self.name


@dataclass(frozen=True)
class LexerCompilation:
    definitions: str
    rules: tuple[RuleSpec, ...]
    asts: tuple[AST, ...]
    nfa: NFA
    dfa: DFA
    nfa_steps: tuple[Step, ...]
    dfa_steps: tuple[Step, ...]
    diagnostics: tuple[str, ...] = ()


@dataclass(frozen=True)
class LexToken:
    rule: str
    category: str
    text: str
    start: int
    end: int
    line: int
    column: int
    skip: bool = False


@dataclass(frozen=True)
class LexStep:
    kind: str
    position: int
    start: int
    state: int
    last_accept: int
    message: str
    token: LexToken | None = None
    source: int = -1
    group: int = -1


@dataclass(frozen=True)
class LexError:
    position: int
    line: int
    column: int
    message: str


@dataclass(frozen=True)
class LexResult:
    tokens: tuple[LexToken, ...]
    skipped: tuple[LexToken, ...]
    errors: tuple[LexError, ...]
    steps: tuple[LexStep, ...]
    complete: bool


@dataclass(frozen=True)
class MinimizationResult:
    dfa: DFA
    mapping: tuple[tuple[int, int], ...]
    partitions: tuple[frozenset[int], ...]
    steps: tuple[Step, ...]


@dataclass(frozen=True)
class EquivalenceResult:
    equivalent: bool | None
    counterexample: str | None = None
    message: str = ""


def char_label(char: str) -> str:
    return {"\n": r"\n", "\r": r"\r", "\t": r"\t", " ": "␠", "\\": r"\\"}.get(
        char, char if 33 <= ord(char) <= 126 else f"\\x{ord(char):02X}"
    )


def charset_label(chars: frozenset[str] | None) -> str:
    if chars is None:
        return "ε"
    if not chars:
        return "∅"
    if len(chars) == 1:
        return char_label(next(iter(chars)))
    values = sorted(map(ord, chars))
    parts = []
    start = end = values[0]
    for value in values[1:] + [999]:
        if value == end + 1:
            end = value
            continue
        parts.append(char_label(chr(start)) + ("–" + char_label(chr(end)) if end > start else ""))
        start = end = value
    return "[" + ", ".join(parts) + "]"


def set_label(states) -> str:
    return "{" + ", ".join(map(str, sorted(states))) + "}" if states else "∅"
