"""Validated, JSON-compatible editable graphs. Coordinates never affect language."""
import math
from .model import NFA, DFA, DFAState, Edge, RuleSpec, Limits, CompileError, LimitExceeded
from .parser import parse_expression
from .automata import partition_alphabet


def edge_chars(label):
    if label.strip() == "ε":
        return None
    node = parse_expression(label)
    while node.kind in ("group", "concat") and len(node.children) == 1:
        node = node.children[0]
    if node.kind != "chars":
        raise CompileError("边只能使用单字符、转义、字符类或 ε，不能使用多字符正则")
    return node.chars


def pattern(chars):
    if chars is None:
        return "ε"
    if not chars:
        return r"[^\x00-\x7F]"
    codes = sorted(ord(c) for c in chars)
    runs, start, end = [], codes[0], codes[0]
    for c in codes[1:] + [999]:
        if c == end + 1:
            end = c
            continue
        runs.append(f"\\x{start:02X}" + (f"-\\x{end:02X}" if end > start else ""))
        start = end = c
    return "[" + "".join(runs) + "]"


def validate_document(document, limits=None):
    limits = limits or Limits()
    if not isinstance(document, dict) or document.get("kind") not in ("nfa", "dfa"):
        raise CompileError("画布类型必须是 nfa 或 dfa")
    nodes, edges = document.get("nodes", []), document.get("edges", [])
    maximum = limits.nfa_states if document["kind"] == "nfa" else limits.dfa_states
    if not isinstance(nodes, list) or not isinstance(edges, list):
        raise CompileError("画布节点／边格式错误")
    if len(nodes) > maximum or len(edges) > limits.steps:
        raise LimitExceeded("画布节点／边超过限制")
    ids = set()
    for node in nodes:
        if not isinstance(node, dict) or type(node.get("id")) is not int or node["id"] < 0 or node["id"] in ids:
            raise CompileError("状态 ID 必须是唯一的非负整数")
        ids.add(node["id"])
        if any(type(node.get(key, False)) is not bool for key in ("start", "accept")):
            raise CompileError("初态／终态标记必须是布尔值")
        for coordinate in ("x", "y"):
            value = node.get(coordinate, 0)
            if not isinstance(value, (int, float)) or not math.isfinite(value) or abs(value) > 1_000_000:
                raise CompileError("状态坐标无效")
        for key in ("label", "category"):
            if not isinstance(node.get(key, ""), str) or len(node.get(key, "")) > 1000:
                raise CompileError("状态名称／类别格式错误")
    for edge in edges:
        if (not isinstance(edge, dict) or type(edge.get("source")) is not int or type(edge.get("target")) is not int
                or edge.get("source") not in ids or edge.get("target") not in ids
                or not isinstance(edge.get("label"), str) or len(edge["label"]) > 2048):
            raise CompileError("边的端点或条件格式错误")
    return document


def machine_from_document(document, *, limits=None):
    validate_document(document, limits)
    nodes = document["nodes"]
    if not nodes:
        raise CompileError("请先创建状态")
    starts = [n["id"] for n in nodes if n.get("start")]
    if len(starts) != 1:
        raise CompileError("必须且只能指定一个初态")
    ids = tuple(n["id"] for n in nodes)
    accepts = frozenset(n["id"] for n in nodes if n.get("accept"))
    edges = tuple(Edge(e["source"], e["target"], edge_chars(e["label"])) for e in document["edges"])
    tags = tuple((n["id"], (RuleSpec(f"state_{n['id']}", "", n["category"]),))
                 for n in nodes if n.get("accept") and n.get("category"))
    nfa = NFA(ids, starts[0], accepts, edges, tags)
    if document["kind"] == "nfa":
        return nfa
    if any(e.chars is None for e in edges):
        raise CompileError("DFA 不允许 ε 边")
    mapping = {s: i for i, s in enumerate(ids)}
    alphabet = partition_alphabet(nfa)
    rows = []
    for s in ids:
        row = []
        for chars in alphabet:
            c = min(chars)
            targets = {e.target for e in edges if e.source == s and c in e.chars}
            if len(targets) > 1:
                raise CompileError(f"状态 {s} 的字符 {c!r} 对应多个目标，不是 DFA")
            row.append(mapping[next(iter(targets))] if targets else -1)
        rows.append(tuple(row))
    candidates = dict(tags)
    states = [DFAState(i, frozenset(), s in accepts, candidates.get(s, ()), frozenset([s])) for i, s in enumerate(ids)]
    if any(t < 0 for row in rows for t in row):
        if len(states) >= (limits or Limits()).dfa_states:
            raise LimitExceeded("补全陷阱状态后超过 DFA 状态限制")
        sink = len(states)
        rows = [tuple(sink if t < 0 else t for t in row) for row in rows]
        rows.append((sink,) * len(alphabet))
        states.append(DFAState(sink, frozenset(), False))
    return DFA(tuple(states), alphabet, tuple(rows), mapping[starts[0]])


def document_from_machine(machine):
    if isinstance(machine, NFA):
        nodes = [{"id": s, "label": f"q{s}", "start": s == machine.start, "accept": s in machine.accepts,
                  "category": next((rs[0].output for state, rs in machine.accepting_rules if state == s), ""),
                  "x": i*100, "y": (i%2)*90} for i, s in enumerate(machine.states)]
        edges = [{"source": e.source, "target": e.target, "label": pattern(e.chars)} for e in machine.edges]
        kind = "nfa"
    else:
        nodes = [{"id": s.id, "label": f"S{s.id}", "start": s.id == machine.start, "accept": s.accepting,
                  "category": s.winner.output if s.winner else "", "x": i*100, "y": (i%2)*90}
                 for i, s in enumerate(machine.states)]
        edges = [{"source": s, "target": t, "label": pattern(machine.alphabet[g])}
                 for s, row in enumerate(machine.transitions) for g, t in enumerate(row) if t >= 0]
        kind = "dfa"
    return {"kind": kind, "nodes": nodes, "edges": edges}
