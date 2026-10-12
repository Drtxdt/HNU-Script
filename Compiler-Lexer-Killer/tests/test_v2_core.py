import itertools
import random
from dataclasses import replace
import pytest
from compiler_lexer_killer.core import (compile, simulate, compile_lexer, tokenize, RuleSpec,
    minimize, check_equivalence, CompileError, Limits, Cancelled)
from compiler_lexer_killer.core.manual import machine_from_document, document_from_machine
from compiler_lexer_killer.core.replay import Event, Replay
from compiler_lexer_killer.project import save_project, load_project


@pytest.mark.parametrize("pattern", ["a+b+", "(a|b)*abb", "(a+)?", "(a+)*", "(a?b?)*", "(a*|b*)+",
    "ε|a", "(a|ε)b*", "(a+b+|b+a+)*", "((a|b)+)?", "[^a]", "[^\\x00-\\x7F]", "ε"])
def test_simplified_equivalence(pattern):
    original = compile(pattern)
    simple = compile(pattern, construction="simplified")
    result = minimize(simple.dfa)
    assert check_equivalence(original.dfa, simple.dfa).equivalent is True
    assert check_equivalence(original.dfa, result.dfa).equivalent is True
    for size in range(7):
        for chars in itertools.product("ab", repeat=size):
            text = "".join(chars)
            assert simulate(original.dfa, text).accepted == simulate(result.dfa, text).accepted
    if pattern == "a+b+":
        assert simulate(simple.dfa, "aabbb").accepted
        assert not simulate(simple.dfa, "abab").accepted


def test_seeded_constructions():
    rng = random.Random(310)
    def expression(depth):
        if depth == 0:
            return rng.choice(["a", "b", "ε", "[ab]"])
        left = expression(depth-1)
        choice = rng.randrange(5)
        if choice < 3:
            return "("+left+")"+"*+?"[choice]
        return "("+left+("|" if choice == 3 else "")+expression(depth-1)+")"
    for _ in range(100):
        source = expression(3)
        a, b = compile(source), compile(source, construction="simplified")
        assert check_equivalence(a.dfa, b.dfa).equivalent, source


def test_lexer_priority_rewind_and_minimize():
    rules = [RuleSpec("A", "'a'"), RuleSpec("LONG", "'ab*c'"), RuleSpec("B", "'b'+")]
    # Operators belong outside quotes in the named-rule syntax.
    rules[1] = RuleSpec("LONG", "'a''b'*'c'")
    for strategy in ["thompson", "simplified"]:
        lexer = compile_lexer("", rules, construction=strategy)
        for dfa in [lexer.dfa, minimize(lexer.dfa).dfa]:
            result = tokenize(dfa, "abbb")
            assert [(t.rule, t.text, t.start, t.end) for t in result.tokens] == [("A", "a", 0, 1), ("B", "bbb", 1, 4)]
            assert any(s.kind == "rewind" for s in result.steps)


def test_keywords_skip_positions_errors():
    rules = [RuleSpec("IF", "'if'", "reserved"), RuleSpec("INT", "'int'", "reserved"),
             RuleSpec("ID", "letter+", "id"), RuleSpec("LE", "'<='"), RuleSpec("LT", "'<'"),
             RuleSpec("WS", r"[ \r\n\t]+", skip=True)]
    lexer = compile_lexer("letter -> [A-Za-z]", rules)
    result = tokenize(lexer, "if iff\r\nint <= < @x", recover=True)
    assert [(t.rule, t.text) for t in result.tokens] == [("IF", "if"), ("ID", "iff"), ("INT", "int"), ("LE", "<="), ("LT", "<"), ("ID", "x")]
    assert (result.tokens[2].line, result.tokens[2].column) == (2, 1)
    assert len(result.errors) == 1 and result.complete
    assert tokenize(lexer, "@x").complete is False
    assert tokenize(lexer, "汉🙂x", recover=True).tokens[0].start == 2
    other = replace(lexer, dfa=minimize(lexer.dfa).dfa)
    assert tokenize(lexer, "if iff\nint").tokens == tokenize(other, "if iff\nint").tokens
    assert check_equivalence(lexer.dfa, other.dfa, compare_categories=True).equivalent
    shadow = compile_lexer("", [RuleSpec("ID", "[a-z]+"), RuleSpec("IF", "'if'")])
    assert shadow.diagnostics and "IF" in shadow.diagnostics[0]


@pytest.mark.parametrize("pattern", ["ε", "'a'*", "('a'|'')", "[ ]*"])
def test_nullable_rule_rejected(pattern):
    with pytest.raises(CompileError) as exc:
        compile_lexer("", [RuleSpec("BAD", pattern, skip=True)])
    assert exc.value.source_id == "rule:0"


def test_equivalence_counterexample_unknown_cancel():
    assert check_equivalence(compile("a*"), compile("a+")).counterexample == ""
    assert check_equivalence(compile("a"), compile("b")).counterexample == "a"
    assert check_equivalence(compile("a*"), compile("a*"), limits=Limits(steps=1)).equivalent is None
    with pytest.raises(Cancelled):
        minimize(compile("a").dfa, cancelled=lambda: True)


def test_manual_and_project(tmp_path):
    d = document_from_machine(compile("(a|b)*abb").dfa)
    assert check_equivalence(machine_from_document(d), compile("(a|b)*abb")).equivalent
    path = tmp_path / "中文 工程.clkproj"
    data = {"format_version": 1, "drawing": d, "source": "a"}
    save_project(path, data)
    assert load_project(path) == data
    d["edges"].append({"source": 0, "target": 0, "label": "ε"})
    with pytest.raises(CompileError):
        machine_from_document(d)


def test_replay():
    events = [Event((("node", i),)) for i in range(180)] + [Event(removed=("node",))]
    replay = Replay(events)
    for count in [0, 65, 180, 128, 1, 181, 65]:
        assert replay.at(count) == ({} if count in [0, 181] else {"node": count-1})


def test_classroom_numeric_and_all_strategies():
    from compiler_lexer_killer.presets import LEXER_PRESETS
    for _, definitions, rules, text, _ in LEXER_PRESETS:
        standard = compile_lexer(definitions, rules)
        simple = compile_lexer(definitions, rules, construction="simplified")
        assert check_equivalence(standard, simple, compare_categories=True).equivalent
        expected = tokenize(standard, text)
        for dfa in (simple.dfa, minimize(standard.dfa).dfa, minimize(simple.dfa).dfa):
            actual = tokenize(dfa, text)
            assert (actual.tokens, actual.skipped, actual.errors) == (expected.tokens, expected.skipped, expected.errors)
    _, definitions, rules, _, _ = LEXER_PRESETS[1]
    lexer = compile_lexer(definitions, rules)
    assert [(t.text, t.category) for t in tokenize(lexer, "12 12.1 12.1E3 3E-2 01").tokens] == [
        ("12", "integer"), ("12.1", "float"), ("12.1E3", "scientific"), ("3E-2", "scientific"), ("01", "integer")]
    assert tokenize(lexer, "1.").errors[0].position == 1


def test_exercise_semantics_limits_and_invalid_projects(tmp_path):
    from compiler_lexer_killer.core.exercises import make_question, grade, reference
    for kind in ("closure", "move", "table", "partition", "tokens"):
        question = make_question(33, "advanced", kind)
        assert question == make_question(33, "advanced", kind)
        assert grade(question, question["expected"]).correct
    for kind in ("automaton", "minimal", "categories"):
        question = make_question(12, kind=kind)
        document = document_from_machine(reference(question))
        assert grade(question, document).correct
        if kind != "categories":
            result = grade(question, document, limits=Limits(steps=1))
            assert result.correct is None
    for data in [dict(format_version=9), dict(format_version=1, exercise={"question":{"kind":"closure", "seed":"bad"}}),
                 dict(format_version=1, rules=[{"name":"X", "enabled":"false"}]), dict(format_version=1, view={"tab":"bad"})]:
        with pytest.raises(CompileError):
            save_project(tmp_path/"bad.clkproj", data)
    with pytest.raises(Cancelled):
        tokenize(compile_lexer("", [RuleSpec("A", "'a'")]), "a", cancelled=lambda: True)
    with pytest.raises(CompileError):
        simulate(compile("a*").dfa, "a"*30, limits=Limits(steps=10))


def test_cli_project_json_and_crlf(tmp_path, capsys):
    import json
    from dataclasses import asdict
    from compiler_lexer_killer.cli import main
    rules = [RuleSpec("A", "'a'"), RuleSpec("WS", r"[\r\n]+", skip=True)]
    project = tmp_path/"test.clkproj"
    save_project(project, {"format_version":1, "definitions":"", "rules":[asdict(r) for r in rules]})
    source = tmp_path/"input.txt"
    source.write_bytes(b"a\r\na")
    assert main(["--project", str(project), "--lex", "--input-file", str(source), "--minimize", "--json"]) == 0
    result = json.loads(capsys.readouterr().out)
    assert result["scan"]["tokens"][1]["start"] == 3
    assert result["scan"]["skipped"][0]["text"] == "\r\n"
    assert main(["--regex", "a*", "--compare-regex", "a+", "--json"]) == 1
    assert json.loads(capsys.readouterr().out)["equivalence"]["counterexample"] == ""


@pytest.mark.parametrize("source,count", [("(a|b)*abb", 4), ("a*", 1), ("a+", 2), ("ab", 4), ("a|b", 3), ("ε", 1), (r"[^\x00-\x7F]", 1)])
def test_minimal_state_counts(source, count):
    assert len(minimize(compile(source).dfa).dfa.states) == count


def test_graph_replay_removals_and_arbitrary_seek():
    from compiler_lexer_killer.core.replay import graph_replay
    result = compile("((a|b)*a?)+abb", construction="simplified")
    replay = graph_replay(result.nfa_steps)
    assert any(event.removed for event in replay.events)
    edge_ids = {}
    for step in result.nfa_steps:
        for edge in step.graph.edges:
            edge_ids.setdefault(edge, len(edge_ids))
    rng = random.Random(16)
    for _ in range(80):
        count = rng.randint(1, len(result.nfa_steps))
        state = replay.at(count)
        expected = result.nfa_steps[count-1].graph
        assert {v for k,v in state.items() if k.startswith("node/")} == set(expected.states)
        assert {v for k,v in state.items() if k.startswith("edge/")} == {edge_ids[e] for e in expected.edges}
