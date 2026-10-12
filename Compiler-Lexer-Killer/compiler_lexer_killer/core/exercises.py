import random
from dataclasses import dataclass
from . import compile
from .model import CompileError, LimitExceeded, Context, Limits, NFA
from .automata import epsilon_closure, move, determinize
from .analysis import minimize, check_equivalence
from .manual import machine_from_document
from .lexer import compile_lexer, tokenize


KINDS = {"closure": "ε 闭包", "move": "move 集合", "table": "DTran 填表", "partition": "最小化分组",
         "tokens": "Token 分类", "automaton": "画等价自动机", "minimal": "画最小 DFA", "categories": "画类别自动机"}


@dataclass(frozen=True)
class GradeResult:
    correct: bool | None
    message: str
    counterexample: str | None = None


def make_question(seed=1, difficulty="basic", kind=None, *, limits=None, cancelled=None):
    rng = random.Random(seed)
    a, b = rng.sample("ab", 2)
    suffix = "".join(rng.choice("ab") for _ in range(rng.randint(1, 3)))
    source = rng.choice([f"{a}|{b}", f"{a}{b}?", f"{a}+{b}+", f"({a}|{b})*{suffix}"] if difficulty == "basic" else
                        [f"({a}{b}|{b}{a})*{suffix}?", f"({a}?{b}+)*{suffix}", f"{a}({a}|{b})*{suffix}",
                         f"({a}+{b}+|{b}+{a}+)*{suffix}?", f"({a}|ε){b}*{suffix}"])
    kind = kind or rng.choice(tuple(KINDS))
    if kind not in KINDS:
        raise CompileError("未知题型")
    compilation = compile(source, limits=limits, cancelled=cancelled)
    question = {"seed": seed, "difficulty": difficulty, "kind": kind, "source": source,
                "hint": "先确认当前集合，再逐个字符组推导；状态编号和布局不决定语言。"}
    if kind == "closure":
        question.update(prompt=f"求 ε-closure({{{compilation.nfa.start}}})。输入 JSON 数组，例如 [0,1]。",
                        expected=sorted(epsilon_closure(compilation.nfa, [compilation.nfa.start])))
    elif kind == "move":
        states = epsilon_closure(compilation.nfa, [compilation.nfa.start])
        question.update(prompt=f"求 move({sorted(states)}, 'a')。输入 JSON 数组。", expected=sorted(move(compilation.nfa, states, "a")))
    elif kind == "table":
        question.update(prompt="填写所有 DFA 状态的 DTran，每行对应 S0、S1…；字符组顺序见参考图。输入二维 JSON 数组。",
                        expected=[list(row) for row in compilation.dfa.transitions])
    elif kind == "partition":
        question.update(prompt="给出原 DFA 的最终最小化分组。输入二维 JSON 数组；组内及组间顺序不限。",
                        expected=[sorted(p) for p in minimize(compilation.dfa, limits=limits, cancelled=cancelled).partitions])
    elif kind == "tokens":
        from ..presets import LEXER_PRESETS
        index = rng.randrange(2)
        _, definitions, rules, text, _ = LEXER_PRESETS[index]
        description = definitions + "\n" + "\n".join(f"{r.name} → {r.pattern}；{r.output}" + ("（跳过）" if r.skip else "") for r in rules)
        question.update(preset=index, text=text, prompt=f"规则按下列顺序优先：\n{description}\n分析 {text!r}，输入 [[原文,类别],...]，不包含跳过内容。",
                        expected=[[t.text, t.category] for t in tokenize(compile_lexer(definitions, rules, limits=limits, cancelled=cancelled), text, limits=limits, cancelled=cancelled).tokens])
    elif kind == "categories":
        question.update(preset=0, prompt="课堂 A 组：绘制识别 if、int 和纯字母 ID 的自动机，终态标记 reserved 或 id。只判断非空单词，不包含空白规则。")
    else:
        question.update(prompt=f"在画图页构建 {'最小 DFA' if kind == 'minimal' else 'NFA 或 DFA'}，识别 {source}；提交画布判题。")
    return question


def grade(question, answer, *, limits=None, cancelled=None):
    context = Context(limits or Limits(), cancelled)
    context.check()
    kind = question["kind"]
    try:
        if kind in ("automaton", "minimal", "categories"):
            machine = machine_from_document(answer, limits=limits)
            if kind == "minimal" and isinstance(machine, NFA):
                return GradeResult(False, "本题要求 DFA，请切换画布类型并检查确定性。")
            if kind == "categories":
                from ..presets import LEXER_PRESETS
                _, definitions, rules, _, _ = LEXER_PRESETS[0]
                target = compile_lexer(definitions, rules[:-1], limits=limits, cancelled=cancelled).dfa
            else:
                target = compile(question["source"], limits=limits, cancelled=cancelled).dfa
            result = check_equivalence(machine, target, compare_categories=kind == "categories", limits=limits, cancelled=cancelled)
            if result.equivalent is not True:
                return GradeResult(result.equivalent, result.message, result.counterexample)
            if kind == "minimal":
                # Count the submitted states, including unreachable ones; extra states are not minimal.
                minimum = len(minimize(target, limits=limits, cancelled=cancelled).dfa.states)
                if len(machine.states) != minimum:
                    return GradeResult(False, f"语言正确，但有 {len(machine.states)} 个状态；题目有效字母表上最少为 {minimum} 个。")
            return GradeResult(True, "正确：语言／类别等价，所要求的结构条件也通过。")
        expected = question["expected"]
        if kind in ("closure", "move"):
            correct = isinstance(answer, list) and all(type(v) is int for v in answer) and set(answer) == set(expected)
        elif kind == "partition":
            correct = (isinstance(answer, list) and all(isinstance(g, list) and g and all(type(s) is int for s in g) for g in answer)
                       and len([s for g in answer for s in g]) == len(set(s for g in answer for s in g))
                       and {frozenset(g) for g in answer} == {frozenset(g) for g in expected})
        elif kind == "table":
            correct = isinstance(answer, list) and all(isinstance(row, list) and all(type(v) is int for v in row) for row in answer) and answer == expected
        else:
            correct = answer == expected
        return GradeResult(correct, "正确，推导结果一致。" if correct else "答案不一致；请检查集合、表格行列或最长匹配优先级。可点击解析查看逐项结果。")
    except LimitExceeded as exc:
        return GradeResult(None, "未完成判定：" + exc.message)
    except CompileError as exc:
        return GradeResult(False, exc.message)
    except (TypeError, KeyError, ValueError):
        return GradeResult(False, "答案格式不正确，请按题目给出的 JSON 格式填写。")


def reference(question, *, cancelled=None):
    kind = question["kind"]
    if kind in ("tokens", "categories"):
        from ..presets import LEXER_PRESETS
        _, definitions, rules, _, _ = LEXER_PRESETS[question.get("preset", 0)]
        return compile_lexer(definitions, rules[:-1] if kind == "categories" else rules, cancelled=cancelled).dfa
    result = compile(question["source"], cancelled=cancelled)
    if kind == "table":
        if "\n列：" not in question["prompt"]:
            from .model import charset_label, set_label
            question["prompt"] += "\n列：" + " | ".join(charset_label(g) for g in result.dfa.alphabet)
            question["prompt"] += "\n" + "；".join(f"S{s.id}={set_label(s.nfa_states)}" for s in result.dfa.states)
    if kind in ("closure", "move", "table"):
        return result.nfa
    if kind == "minimal":
        return minimize(result.dfa, cancelled=cancelled).dfa
    return result.dfa
