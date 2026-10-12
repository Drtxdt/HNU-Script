from collections import deque
from .model import LimitExceeded
from .model import (DFA, DFAState, NFA, Context, Limits, Step, CompileError,
                    MinimizationResult, EquivalenceResult)
from .automata import determinize


def _output(state, categories=False):
    if state is None or not state.accepting:
        return None
    if categories:
        return (state.winner.output, state.winner.skip) if state.winner else ("accept", False)
    return True


def minimize(dfa, *, limits=None, cancelled=None):
    context = Context(limits or Limits(), cancelled)
    rows = [list(row) for row in dfa.transitions]
    states = list(dfa.states)
    if any(t < 0 for row in rows for t in row):
        sink = len(states)
        states.append(DFAState(sink, frozenset(), False))
        rows = [[sink if t < 0 else t for t in row] for row in rows]
        rows.append([sink] * len(dfa.alphabet))
    if len(states) > context.limits.dfa_states:
        raise LimitExceeded("补全后的 DFA 超过状态限制")
    reachable, queue = {dfa.start}, deque([dfa.start])
    while queue:
        context.check()
        for target in rows[queue.popleft()]:
            if target not in reachable:
                reachable.add(target)
                queue.append(target)
    grouped = {}
    for s in sorted(reachable):
        state = states[s]
        key = (state.accepting, state.winner)
        grouped.setdefault(key, set()).add(s)
    partitions = tuple(frozenset(p) for p in grouped.values())
    steps = []

    def emit(message):
        context.tick()
        steps.append(Step("partition", message, partitions=partitions))

    emit("去除不可达状态；按接受性及获胜规则／动作建立初始分组")
    while True:
        owner = {s: i for i, group in enumerate(partitions) for s in group}
        refined = []
        for group in partitions:
            buckets = {}
            for s in sorted(group):
                context.check()
                signature = tuple(owner[t] for t in rows[s])
                buckets.setdefault(signature, set()).add(s)
            refined.extend(frozenset(v) for v in buckets.values())
        updated = tuple(refined)
        if updated == partitions:
            break
        partitions = updated
        emit("按各字符组的目标分组拆分：" + " | ".join(str(sorted(p)) for p in partitions))
    owner = {s: i for i, p in enumerate(partitions) for s in p}
    order, known, queue = [], {owner[dfa.start]}, deque([owner[dfa.start]])
    while queue:
        context.check()
        part = queue.popleft()
        order.append(part)
        for t in rows[min(partitions[part])]:
            target = owner[t]
            if target not in known:
                known.add(target)
                queue.append(target)
    renumber = {p: i for i, p in enumerate(order)}
    result_states, result_rows = [], []
    for i, part in enumerate(order):
        group = partitions[part]
        representative = states[min(group)]
        # NFA subsets belong to subset construction, not to quotient states.
        result_states.append(DFAState(i, frozenset(), representative.accepting,
                                      (representative.winner,) if representative.winner else (), group))
        result_rows.append(tuple(renumber[owner[t]] for t in rows[min(group)]))
    emit(f"分组稳定：{len(reachable)} → {len(order)} 个状态；编号按 BFS 生成")
    result = DFA(tuple(result_states), dfa.alphabet, tuple(result_rows))
    mapping = tuple((s, renumber[owner[s]]) for s in sorted(reachable) if s < len(dfa.states))
    return MinimizationResult(result, mapping, tuple(partitions[p] for p in order), tuple(steps))


def check_equivalence(left, right, *, compare_categories=False, limits=None, cancelled=None):
    context = Context(limits or Limits(), cancelled)
    try:
        def normal(machine):
            if hasattr(machine, "dfa"):
                machine = machine.dfa
            if isinstance(machine, NFA):
                if len(machine.states) > context.limits.nfa_states:
                    raise LimitExceeded("NFA 超过状态限制")
                return determinize(machine, context)[0]
            if len(machine.states) > context.limits.dfa_states:
                raise LimitExceeded("DFA 超过状态限制")
            return machine
        left, right = normal(left), normal(right)
        maps = [{c: i for i, group in enumerate(d.alphabet) for c in group} for d in (left, right)]
        def output(d, s):
            return _output(d.states[s] if s >= 0 else None, compare_categories)
        def target(d, groups, s, c):
            return d.transitions[s][groups[c]] if s >= 0 and c in groups else -1
        start = (left.start, right.start)
        parents, queue = {start: None}, deque([start])
        while queue:
            context.tick()
            a, b = pair = queue.popleft()
            if output(left, a) != output(right, b):
                chars = []
                while parents[pair] is not None:
                    pair, char = parents[pair]
                    chars.append(char)
                word = "".join(reversed(chars))
                return EquivalenceResult(False, word, "语言或类别不同，已找到最短反例")
            for code in range(128):
                char = chr(code)
                pair = (target(left, maps[0], a, char), target(right, maps[1], b, char))
                if pair not in parents:
                    parents[pair] = ((a, b), char)
                    queue.append(pair)
        return EquivalenceResult(True, None, "ASCII 全集上的乘积自动机检查通过：等价")
    except CompileError as exc:
        return EquivalenceResult(None, None, f"未完成判定：{exc.message}")
