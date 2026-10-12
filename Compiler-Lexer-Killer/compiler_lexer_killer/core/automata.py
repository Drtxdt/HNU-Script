from collections import deque
from .model import LimitExceeded, Limits
from .model import (AST, NFA, DFA, Edge, DFAState, Context, CompileError, Step,
                    SimulationStep, SimulationResult, charset_label, set_label, char_label)


def thompson(ast: AST, context: Context):
    edges, steps, fragments = [], [], []
    count = 0

    def state():
        nonlocal count
        context.check()
        if count >= context.limits.nfa_states:
            raise LimitExceeded(f"NFA 状态超过 {context.limits.nfa_states} 个，请缩小表达式", node.span)
        value = count
        count += 1
        return value

    def edge(a, b, chars=None):
        edges.append(Edge(a, b, chars))

    stack = [(ast, False, -1)]
    occurrences = 0
    while stack:
        context.check()
        node, visited, occurrence = stack.pop()
        if not visited:
            occurrence = occurrences
            occurrences += 1
            if occurrences > context.limits.steps:
                raise LimitExceeded(f"展开后的 AST 超过 {context.limits.steps:,} 个节点，请缩小表达式", node.span)
            stack.append((node, True, occurrence))
            stack.extend((child, False, -1) for child in reversed(node.children))
            continue
        size = len(node.children)
        children = fragments[-size:] if size else []
        if size:
            del fragments[-size:]
        before = len(edges)
        if node.kind in ("chars", "epsilon"):
            start, end = state(), state()
            edge(start, end, node.chars if node.kind == "chars" else None)
        elif node.kind in ("reference", "group"):
            start, end = children[0]
        elif node.kind == "concat":
            if not children:
                start, end = state(), state()
                edge(start, end)
            else:
                start, end = children[0][0], children[-1][1]
                for left, right in zip(children, children[1:]):
                    edge(left[1], right[0])
        elif node.kind == "union":
            start, end = state(), state()
            for a, b in children:
                edge(start, a)
                edge(b, end)
        else:
            start, end = state(), state()
            a, b = children[0]
            edge(start, a)
            edge(b, end)
            if node.kind in ("star", "plus"):
                edge(b, a)
            if node.kind in ("star", "optional"):
                edge(start, end)
        fragments.append((start, end))
        context.tick()
        labels = {"chars": "字符片段", "epsilon": "空串片段", "concat": "连接：用 ε 边连接片段",
                  "union": "并：建立两个可选分支", "star": "闭包：允许零次或多次", "plus": "正闭包：至少一次",
                  "optional": "可选：零次或一次", "group": "括号分组", "reference": f"展开引用 {node.name}（独立片段）"}
        steps.append(Step("thompson", labels[node.kind], node.span, frozenset((start, end)),
                          tuple(range(before, len(edges))), count, len(edges), fragment=(start, end), ast_occurrence=occurrence))
    start, end = fragments[0]
    return NFA(tuple(range(count)), start, frozenset([end]), tuple(edges)), tuple(steps)


def closure_details(nfa, states):
    adjacency = {}
    for index, edge in enumerate(nfa.edges):
        if edge.chars is None:
            adjacency.setdefault(edge.source, []).append((edge.target, index))
    result = set(states)
    queue = deque(sorted(result))
    used = []
    while queue:
        current = queue.popleft()
        for target, index in adjacency.get(current, ()):
            used.append(index)
            if target not in result:
                result.add(target)
                queue.append(target)
    return frozenset(result), tuple(used)


def epsilon_closure(nfa, states):
    return closure_details(nfa, states)[0]


def move_details(nfa, states, char):
    selected = tuple(i for i, e in enumerate(nfa.edges) if e.source in states and e.chars is not None and char in e.chars)
    return frozenset(nfa.edges[i].target for i in selected), selected


def move(nfa, states, char):
    return move_details(nfa, states, char)[0]


def partition_alphabet(nfa):
    conditions = tuple(dict.fromkeys(e.chars for e in nfa.edges if e.chars))
    groups = {}
    for code in range(128):
        char = chr(code)
        signature = tuple(i for i, chars in enumerate(conditions) if char in chars)
        if signature:
            groups.setdefault(signature, set()).add(char)
    return tuple(frozenset(value) for value in groups.values())


def determinize(nfa, context):
    alphabet = partition_alphabet(nfa)
    start, traversed = closure_details(nfa, [nfa.start])
    rule_order = tuple(dict.fromkeys(rule for _, rules in nfa.accepting_rules for rule in rules))
    def make_state(index, subset):
        candidates = {r for state, rules in nfa.accepting_rules if state in subset for r in rules}
        return DFAState(index, subset, bool(subset & nfa.accepts), tuple(r for r in rule_order if r in candidates))
    states = [make_state(0, start)]
    known = {start: 0}
    rows, steps = [], []
    committed = 0

    def emit(kind, message, nodes=frozenset(), edges=(), source=-1, group=-1, target=-1):
        context.tick()
        steps.append(Step(kind, message, nodes=nodes, edges=edges, node_count=len(states),
                          source=source, group=group, target=target, committed=committed))

    emit("start", f"S0 = ε-closure({set_label([nfa.start])}) = {set_label(start)}", start, traversed, target=0)
    current = 0
    while current < len(states):
        context.check()
        subset = states[current].nfa_states
        emit("select", f"处理 S{current} = {set_label(subset)}", subset, source=current)
        row = []
        for group, chars in enumerate(alphabet):
            representative = min(chars)
            moved, used = move_details(nfa, subset, representative)
            label = charset_label(chars)
            emit("move", f"move(S{current}, {label}) = {set_label(moved)}", moved, used, current, group)
            closed, used = closure_details(nfa, moved)
            emit("closure", f"ε-closure({set_label(moved)}) = {set_label(closed)}", closed, used, current, group)
            if closed not in known:
                if len(states) >= context.limits.dfa_states:
                    raise LimitExceeded(f"DFA 状态超过 {context.limits.dfa_states} 个，请缩小表达式")
                index = len(states)
                known[closed] = index
                states.append(make_state(index, closed))
                emit("register", f"登记 S{index} = {set_label(closed)}" + ("（陷阱状态）" if not closed else ""), closed, source=current, group=group, target=index)
            target = known[closed]
            row.append(target)
            committed += 1
            emit("transition", f"DTran[S{current}, {label}] = S{target}", closed, source=current, group=group, target=target)
        rows.append(tuple(row))
        current += 1
    emit("complete", f"构造完成：{len(states)} 个 DFA 状态，{len(alphabet)} 个互不重叠的字符组")
    return DFA(tuple(states), alphabet, tuple(rows)), tuple(steps)


def nfa_match(nfa, text):
    states = epsilon_closure(nfa, [nfa.start])
    for char in text:
        states = epsilon_closure(nfa, move(nfa, states, char))
    return bool(states & nfa.accepts)


def simulate(dfa, text, *, limits=None, cancelled=None):
    context = Context(limits or Limits(), cancelled)
    context.tick()
    state = dfa.start
    steps = [SimulationStep(0, state, state, -1, f"从 S{state} 开始，尚未读取字符")]
    failure = None
    live = live_states(dfa)
    for index, char in enumerate(text):
        context.tick()
        group = next((i for i, chars in enumerate(dfa.alphabet) if char in chars), -1)
        if group < 0:
            steps.append(SimulationStep(index + 1, state, -1, -1, f"第 {index + 1} 个字符 {char_label(char)} 不在字母表中，拒绝"))
            return SimulationResult(False, -1, tuple(steps), index)
        target = dfa.transitions[state][group]
        if target < 0:
            steps.append(SimulationStep(index + 1, state, -1, group, f"第 {index+1} 个字符没有迁移，拒绝"))
            return SimulationResult(False, -1, tuple(steps), index)
        steps.append(SimulationStep(index + 1, state, target, group, f"读取 {char_label(char)}：S{state} → S{target}"))
        state = target
        if state not in live and failure is None:
            failure = index
    accepted = dfa.states[state].accepting
    if not accepted and failure is None:
        failure = len(text)
    context.tick()
    steps.append(SimulationStep(len(text), state, state, -1,
                               f"输入结束：S{state} " + ("是接受状态，匹配成功 ✓" if accepted else "不是接受状态，匹配失败")))
    return SimulationResult(accepted, state, tuple(steps), failure)


def live_states(dfa):
    reverse = {}
    for source, row in enumerate(dfa.transitions):
        for target in row:
            if target >= 0:
                reverse.setdefault(target, set()).add(source)
    live = {s.id for s in dfa.states if s.accepting}
    queue = deque(live)
    while queue:
        for source in reverse.get(queue.popleft(), ()):
            if source not in live:
                live.add(source)
                queue.append(source)
    return frozenset(live)
