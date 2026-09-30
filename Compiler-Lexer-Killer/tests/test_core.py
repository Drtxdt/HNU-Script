import itertools
from dataclasses import FrozenInstanceError
import pytest

from compiler_lexer_killer.core import compile, simulate, nfa_match, epsilon_closure, CompileError, Limits, Cancelled
from compiler_lexer_killer.core.parser import parse
from compiler_lexer_killer.core.model import ASCII


@pytest.mark.parametrize("pattern,accepts,rejects", [
    ("a", ["a"], ["", "aa", "b"]),
    ("ε", [""], ["a"]),
    ("a|bc", ["a", "bc"], ["ac", "b", "abc"]),
    ("ab", ["ab"], ["a", "b", ""]),
    ("a*", ["", "a", "aaa"], ["b", "ab"]),
    ("a+", ["a", "aaa"], ["", "b"]),
    ("a?", ["", "a"], ["aa", "b"]),
    ("(a|b)*abb", ["abb", "aabb", "abababb"], ["", "ab", "abba"]),
    ("a+b+", ["ab", "aabbb"], ["abab", "a", "b", ""]),
    ("(ε|a)*", ["", "a", "aaa"], ["b"]),
    ("[a-c]|[b-d]", ["a", "b", "c", "d"], ["e", "ab"]),
    ("[^a]", ["b", "\x00", "\x7f", "\n"], ["a", "中", ""]),
    (r"\*\+\?\|\(\)\[\]\\", ["*+?|()[]\\"], ["", "a"]),
    (r"\x41\t\n\r", ["A\t\n\r"], ["A", "A "]),
    (" a · b ", ["ab"], ["a b"]),
    ("'a b'", ["a b"], ["ab"]),
    ("[ ]", [" "], ["", "a"]),
    ("'.'", ["."], ["x"]),
    ("''", [""], ["a"]),
    (r"[^\x00-\x7F]", [], ["", "a", "\x00"]),
])
def test_languages(pattern, accepts, rejects):
    result = compile(pattern)
    for text in accepts + rejects:
        expected = text in accepts
        assert nfa_match(result.nfa, text) == expected
        assert simulate(result.dfa, text).accepted == expected


@pytest.mark.parametrize("pattern", ["a", "a|b", "ab", "a*", "a+", "a?", "(a|b)*abb", "a+b+", "(ε|a)*", "(ab|ba)?a*"])
def test_exhaustive_equivalence(pattern):
    result = compile(pattern)
    for length in range(7):
        for chars in itertools.product("ab", repeat=length):
            text = "".join(chars)
            actual = simulate(result.dfa, text).accepted
            assert actual == nfa_match(result.nfa, text), (pattern, text)
            if pattern == "(a|b)*abb":
                assert actual == text.endswith("abb")
            elif pattern == "a+b+":
                a, b = len(text) - len(text.lstrip("a")), len(text) - len(text.rstrip("b"))
                assert actual == (a > 0 and b > 0 and a + b == len(text))


def test_precedence_and_explicit_concat():
    ast, _, _ = parse("a|bc*")
    assert ast.kind == "union"
    assert ast.children[1].kind == "concat"
    assert ast.children[1].children[1].kind == "star"
    assert compile("(a|b)*abb").dfa == compile("(a|b)*·a·b·b").dfa


@pytest.mark.parametrize("text", ["", " ", "a|", "|a", "()", "(a", "a)", "a**", "*a", "a+?", "[z-a]", "[]", "[^]", "[a", "'abc", "\\", r"\d", r"\x8A", r"\xG0", "中", "a{2}", "^a", "a$", "(?=a)", r"(a)\1"])
def test_invalid_patterns(text):
    with pytest.raises(CompileError) as error:
        compile(text)
    assert error.value.span.line >= 1
    assert error.value.span.column >= 1


def test_named_definitions_and_entry():
    source = "letter → [A-Za-z]\ndigit -> [0-9]\nid → letter(letter|digit)*"
    result = compile(source, "definitions")
    assert result.entry == "id"
    assert result.definitions == ("letter", "digit", "id")
    assert simulate(result.dfa, "initial10").accepted
    assert not simulate(result.dfa, "1initial").accepted
    assert simulate(compile(source, "definitions", "digit").dfa, "5").accepted


def test_reference_fragments_are_independent():
    result = compile("unit -> 'a'\ntwice -> unit unit", "definitions")
    assert len(result.nfa.states) == 4
    assert simulate(result.dfa, "aa").accepted
    for text in ["", "a", "aaa"]:
        assert not simulate(result.dfa, text).accepted
    repeated = compile("unit -> 'a'+\npair -> unit unit", "definitions")
    assert not simulate(repeated.dfa, "a").accepted
    assert simulate(repeated.dfa, "aa").accepted


@pytest.mark.parametrize("source", ["a -> a", "a -> b\nb -> 'x'", "a -> 'x'\na -> 'y'", "a -> missing", "a 'x'", "1a -> 'x'", "a -> 'x'\nb -> 1"])
def test_invalid_definitions(source):
    with pytest.raises(CompileError):
        compile(source, "definitions")


def test_error_source_location():
    source = "letter → [a-z]\nid → missing"
    with pytest.raises(CompileError) as error:
        compile(source, "definitions")
    span = error.value.span
    assert (span.line, span.column) == (2, 6)
    assert source[span.start:span.end] == "missing"
    with pytest.raises(CompileError):
        compile("a → 'x'", "definitions", "missing")


def test_partition_ascii_semantics():
    result = compile("[a-c]|[^b]")
    groups = result.dfa.alphabet
    assert frozenset.union(*groups) == ASCII
    assert sum(map(len, groups)) == 128
    for char in ASCII:
        assert simulate(result.dfa, char).accepted


def test_sink_and_simulation_positions():
    result = compile("ab")
    sink = next(s for s in result.dfa.states if not s.nfa_states)
    assert all(target == sink.id for target in result.dfa.transitions[sink.id])
    assert simulate(result.dfa, "aa").failure_position == 1
    assert simulate(result.dfa, "a").failure_position == 1  # EOF
    assert simulate(result.dfa, "中").failure_position == 0
    assert simulate(result.dfa, "ab").failure_position is None
    assert not simulate(result.dfa, "ab ").accepted


def test_closure_contains_input_and_terminates_cycles():
    result = compile("(ε|a)*")
    closure = epsilon_closure(result.nfa, [result.nfa.start])
    assert result.nfa.start in closure
    assert closure & result.nfa.accepts


def test_trace_determinism_and_immutability():
    first, second = compile("(a|b)*abb"), compile("(a|b)*abb")
    assert first == second
    with pytest.raises(FrozenInstanceError):
        first.dfa_steps[0].message = "changed"
    assert first.nfa_steps[-1].node_count == len(first.nfa.states)
    assert first.nfa_steps[-1].edge_count == len(first.nfa.edges)
    assert first.dfa_steps[-1].committed == sum(map(len, first.dfa.transitions))
    for step in first.dfa_steps:
        assert step.nodes <= set(first.nfa.states)
        if step.kind == "transition":
            assert first.dfa.transitions[step.source][step.group] == step.target


def test_limits_and_cancellation():
    with pytest.raises(CompileError, match="NFA"):
        compile("aaa", limits=Limits(nfa_states=4))
    with pytest.raises(CompileError, match="DFA"):
        compile("ab", limits=Limits(dfa_states=1))
    with pytest.raises(CompileError, match="步骤"):
        compile("a", limits=Limits(steps=1))
    with pytest.raises(Cancelled):
        compile("a", cancelled=lambda: True)
    calls = 0
    def cancel_later():
        nonlocal calls
        calls += 1
        return calls > 20
    with pytest.raises(Cancelled):
        compile("(a|b)*abb", cancelled=cancel_later)


def test_nested_groups_do_not_require_python_recursion():
    result = compile("("*1500 + "a" + ")"*1500)
    assert simulate(result.dfa, "a").accepted


def test_nested_ast_is_bounded_before_postorder_expansion():
    with pytest.raises(CompileError, match="AST"):
        compile("("*300 + "a" + ")"*300, limits=Limits(steps=100))


def test_expanding_references_stop_at_limits():
    lines = ["a0 -> 'a'"] + [f"a{i} -> a{i-1} a{i-1}" for i in range(1, 25)]
    with pytest.raises(CompileError, match="NFA"):
        compile("\n".join(lines), "definitions")
