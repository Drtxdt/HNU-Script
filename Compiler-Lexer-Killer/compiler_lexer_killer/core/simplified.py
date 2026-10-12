"""Course boundary-aware construction (not globally minimal NFA)."""
from .model import NFA, Edge, Step, CompileError, LimitExceeded


def simplified(ast, context):
    states, edges, steps, fragments = set(), [], [], []
    serial = 0

    def new():
        nonlocal serial
        context.check()
        if len(states) >= context.limits.nfa_states:
            raise LimitExceeded("简化构造超过 NFA 状态限制")
        value = serial
        serial += 1
        states.add(value)
        return value

    def link(a, b, chars=None):
        edges.append(Edge(a, b, chars))

    def incoming(s):
        return any(e.target == s for e in edges)

    def outgoing(s):
        return any(e.source == s for e in edges)

    def merge(keep, remove):
        if keep == remove:
            return
        edges[:] = list(dict.fromkeys(Edge(keep if e.source == remove else e.source,
                                          keep if e.target == remove else e.target, e.chars) for e in edges))
        states.remove(remove)
        merges.append((remove, keep))
        operations.append(f"合并 q{remove} → q{keep}")

    def isolate(a, b):
        if incoming(a):
            start = new()
            operations.append(f"q{a} 有入边，增加初态 q{start}")
            link(start, a)
            a = start
        if outgoing(b):
            end = new()
            operations.append(f"q{b} 有出边，增加终态 q{end}")
            link(b, end)
            b = end
        return a, b

    stack, occurrences = [(ast, False, -1)], 0
    while stack:
        context.check()
        node, visited, occurrence = stack.pop()
        if not visited:
            occurrence = occurrences
            occurrences += 1
            if occurrences > context.limits.steps:
                raise LimitExceeded("展开后的 AST 超过步骤限制", node.span)
            stack.append((node, True, occurrence))
            stack.extend((c, False, -1) for c in reversed(node.children))
            continue
        size = len(node.children)
        children = fragments[-size:] if size else []
        if size:
            del fragments[-size:]
        reason = ""
        operations, merges = [], []
        if node.kind in ("chars", "epsilon") or (node.kind == "concat" and not children):
            a, b = new(), new()
            link(a, b, node.chars if node.kind == "chars" else None)
            reason = "建立基本片段"
        elif node.kind in ("group", "reference"):
            a, b = children[0]
            reason = "独立展开引用" if node.kind == "reference" else "保留括号片段"
        elif node.kind == "concat":
            a, b = children[0]
            merged = 0
            for c, d in children[1:]:
                if not outgoing(b) or not incoming(c):
                    operations.append(f"q{b} 无出边" if not outgoing(b) else f"q{c} 无入边")
                    merge(b, c)
                    merged += 1
                else:
                    link(b, c)
                    operations.append(f"q{b} 有出边且 q{c} 有入边，增加 ε 桥")
                b = d
            reason = f"连接：安全合并 {merged} 个边界；两侧存在回路时保留 ε 桥，防止倒灌"
        elif node.kind == "union":
            parts = [isolate(*part) for part in children]
            a, b = parts[0]
            for c, d in parts[1:]:
                merge(a, c)
                merge(b, d)
            reason = "并：先隔离有入边的初态／有出边的终态，再合并分支端点"
        else:
            a, b = isolate(*children[0])
            if node.kind in ("star", "plus"):
                link(b, a)
            if node.kind in ("star", "optional"):
                link(a, b)
            reason = {"star": "闭包", "plus": "正闭包", "optional": "可选"}[node.kind] + "：按端点入／出边决定是否增加隔离状态"
        fragments.append((a, b))
        context.tick()
        graph = NFA(tuple(sorted(states)), a, frozenset([b]), tuple(edges))
        steps.append(Step("simplified", reason + ("\n" + "；".join(operations) if operations else ""), node.span, frozenset([a, b]),
                          node_count=serial, edge_count=len(edges), fragment=(a, b),
                          ast_occurrence=occurrence, graph=graph, merges=tuple(merges)))
    a, b = fragments[0]
    return NFA(tuple(sorted(states)), a, frozenset([b]), tuple(edges)), tuple(steps)
