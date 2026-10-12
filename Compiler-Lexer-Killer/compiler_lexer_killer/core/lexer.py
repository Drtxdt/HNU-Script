from .model import (Context, Limits, CompileError, LimitExceeded, RuleSpec, NFA, Edge, Step,
                    LexerCompilation, LexToken, LexStep, LexResult, LexError)
from .parser import parse_definitions, parse_expression, name_start, name_char
from .automata import thompson, determinize, epsilon_closure, live_states
from .simplified import simplified


def compile_lexer(definitions, rules, *, construction="thompson", limits=None, cancelled=None):
    context = Context(limits or Limits(), cancelled)
    if construction not in ("thompson", "simplified"):
        raise CompileError("未知构造策略")
    try:
        names = parse_definitions(definitions, context)
    except CompileError as exc:
        exc.source_id = "definitions"
        raise
    normalized = []
    for row, value in enumerate(rules):
        context.check()
        try:
            rule = value if isinstance(value, RuleSpec) else RuleSpec(**value)
        except (TypeError, ValueError) as exc:
            raise CompileError("规则字段无效，需要 name、pattern 和可选 category、skip、enabled", source_id=f"rule:{row}") from exc
        if (any(not isinstance(v, str) for v in (rule.name, rule.pattern, rule.category))
                or type(rule.enabled) is not bool or type(rule.skip) is not bool):
            raise CompileError("规则名称、表达式、类别须为文本，启用／跳过须为布尔值", source_id=f"rule:{row}")
        normalized.append(rule)
        if len(normalized) > context.limits.steps:
            raise LimitExceeded("规则条数超过资源限制")
    rules = tuple(normalized)
    selected = [(i, r) for i, r in enumerate(rules) if r.enabled]
    if not selected:
        raise CompileError("请至少启用一条规则")
    seen, asts, edges, tags, steps = set(), [], [], [], []
    count = 1
    for row, rule in selected:
        context.check()
        source_id = f"rule:{row}"
        if (not rule.name or not name_start(rule.name[0]) or
                not all(name_char(c) for c in rule.name) or rule.name in seen):
            raise CompileError("规则名称须为唯一的 ASCII 标识符", source_id=source_id)
        seen.add(rule.name)
        try:
            if len(rule.pattern) > 65_536:
                raise CompileError("规则表达式超过长度限制")
            ast = parse_expression(rule.pattern, names, context=context)
            part, _ = (simplified if construction == "simplified" else thompson)(ast, context)
            if epsilon_closure(part, [part.start]) & part.accepts:
                raise CompileError("规则不能匹配空串；重复空白请使用 + 而不是 *", ast.span)
        except CompileError as exc:
            exc.source_id = source_id
            raise
        asts.append(ast)
        mapping = {s: count+i for i, s in enumerate(part.states)}
        count += len(part.states)
        if count > context.limits.nfa_states:
            raise LimitExceeded("联合 NFA 超过状态限制", source_id=source_id)
        before = len(edges)
        edges.append(Edge(0, mapping[part.start], None))
        edges.extend(Edge(mapping[e.source], mapping[e.target], e.chars) for e in part.edges)
        tags.extend((mapping[s], (rule,)) for s in sorted(part.accepts))
        context.tick()
        steps.append(Step("rule", f"连接规则 {rule.name} → {rule.output}（{'跳过' if rule.skip else '输出'}）",
                          nodes=frozenset(mapping.values()), edges=tuple(range(before, len(edges))),
                          node_count=count, edge_count=len(edges)))
    nfa = NFA(tuple(range(count)), 0, frozenset(s for s, _ in tags), tuple(edges), tuple(tags))
    dfa, dfa_steps = determinize(nfa, context)
    winners = {s.winner for s in dfa.states if s.winner}
    diagnostics = tuple(f"规则 {r.name} 从未胜出：语言为空或被更靠前的规则完全遮蔽" for _, r in selected if r not in winners)
    return LexerCompilation(definitions, rules, tuple(asts), nfa, dfa, tuple(steps), dfa_steps, diagnostics)


def tokenize(lexer, text, recover=False, *, limits=None, cancelled=None):
    context = Context(limits or Limits(), cancelled)
    dfa = lexer.dfa if hasattr(lexer, "dfa") else lexer
    if dfa.states[dfa.start].accepting:
        raise CompileError("词法分析器初态不可接受空串")
    groups = {c: i for i, chars in enumerate(dfa.alphabet) for c in chars}
    live = live_states(dfa)
    positions, line, col = [], 1, 1
    for i, char in enumerate(text):
        context.check()
        positions.append((line, col))
        if char == "\r" or (char == "\n" and (i == 0 or text[i-1] != "\r")):
            line, col = line+1, 1
        elif char != "\n":
            col += 1
    positions.append((line, col))
    tokens, skipped, errors, steps = [], [], [], []

    def emit(kind, pos, begin, state, last, message, token=None, source=-1, group=-1):
        context.tick()
        steps.append(LexStep(kind, pos, begin, state, last, message, token, source, group))

    begin = 0
    while begin < len(text):
        context.check()
        state, cursor, last, winner = dfa.start, begin, -1, None
        last_state = -1
        emit("start", cursor, begin, state, last, f"从偏移 {begin} 开始扫描")
        while cursor < len(text):
            context.check()
            group = groups.get(text[cursor], -1)
            target = dfa.transitions[state][group] if group >= 0 else -1
            if target < 0 or target not in live:
                emit("jam", cursor, begin, state, last, "无法继续，检查最后接受位置")
                break
            source, state = state, target
            cursor += 1
            if dfa.states[state].winner:
                last, winner = cursor, dfa.states[state].winner
                last_state = state
            emit("read", cursor, begin, state, last,
                 f"读取 {text[cursor-1]!r} → S{state}；最后接受位置 {last if last >= 0 else '无'}", source=source, group=group)
        if winner is None:
            line, col = positions[begin]
            error = LexError(begin, line, col, f"第 {line} 行第 {col} 列：无法识别 {text[begin]!r}")
            errors.append(error)
            emit("error", begin, begin, state, last, error.message)
            if not recover:
                return LexResult(tuple(tokens), tuple(skipped), tuple(errors), tuple(steps), False)
            begin += 1
            continue
        if cursor != last:
            emit("rewind", last, begin, last_state, last, f"回退 {cursor-last} 个字符至偏移 {last}")
        state = last_state
        line, col = positions[begin]
        token = LexToken(winner.name, winner.output, text[begin:last], begin, last, line, col, winner.skip)
        (skipped if winner.skip else tokens).append(token)
        emit("skip" if winner.skip else "emit", last, begin, state, last,
             f"{'跳过' if winner.skip else '提交'} {token.text!r} → {token.category}（{token.rule}）", token)
        begin = last
    emit("complete", len(text), begin, dfa.start, -1, f"扫描结束：{len(tokens)} 个 Token，{len(errors)} 个错误")
    return LexResult(tuple(tokens), tuple(skipped), tuple(errors), tuple(steps), True)
