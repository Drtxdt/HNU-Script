"""Graphviz geometry service. Runs outside the UI thread."""
import os
from pathlib import Path
import shutil
import shlex
import subprocess
import sys
import time

from ..core.model import Cancelled, charset_label, set_label


def find_dot():
    root = Path(getattr(sys, "_MEIPASS", Path(__file__).resolve().parents[2]))
    candidates = [root / "vendor/graphviz/bin/dot.exe", root / "graphviz/bin/dot.exe"]
    if os.environ.get("GRAPHVIZ_DOT"):
        candidates.insert(0, Path(os.environ["GRAPHVIZ_DOT"]))
    for candidate in candidates:
        if candidate.is_file():
            return str(candidate)
    executable = shutil.which("dot")
    if executable:
        return executable
    raise RuntimeError("未找到 Graphviz。源码运行请先执行 tools/prepare_graphviz.py，或设置 GRAPHVIZ_DOT。")


def graph_data(compilation, kind):
    if kind == "nfa":
        nfa = compilation.nfa
        tags = dict(nfa.accepting_rules)
        nodes = [(s, f"q{s}", s in nfa.accepts, f"NFA 状态 q{s}" + "\n" + ", ".join(r.output for r in tags.get(s, ()))) for s in nfa.states]
        edges = [(e.source, e.target, charset_label(e.chars)) for e in nfa.edges]
        return nodes, edges, nfa.start
    dfa = compilation.dfa
    nodes = [(s.id, f"S{s.id}", s.accepting, (f"原 DFA 成员 {set_label(s.members)}" if s.members else f"NFA 集合 {set_label(s.nfa_states)}")
              + ("\n候选：" + ", ".join(r.name for r in s.candidates) if s.candidates else "")
              + (f"\n获胜：{s.winner.name} → {s.winner.output}" if s.winner else "")
              + ("\n接受状态" if s.accepting else "")) for s in dfa.states]
    edges = [(i, target, charset_label(dfa.alphabet[j])) for i, row in enumerate(dfa.transitions) for j, target in enumerate(row)]
    return nodes, edges, dfa.start


def compute_layout(compilation, kind, cancelled=lambda: False):
    nodes, edges, start = graph_data(compilation, kind)
    return compute_data_layout(nodes, edges, start, cancelled)


def machine_layout(machine, cancelled=lambda: False):
    from types import SimpleNamespace
    from ..core.model import NFA
    kind = "nfa" if isinstance(machine, NFA) else "dfa"
    return compute_layout(SimpleNamespace(**{kind: machine}), kind, cancelled)


def history_layout(graphs, cancelled=lambda: False):
    nodes, edges = {}, {}
    for graph in graphs:
        for state in graph.states:
            nodes[state] = (state, f"q{state}", state in graph.accepts, f"状态 q{state}")
        for edge in graph.edges:
            key = (edge.source, edge.target, charset_label(edge.chars))
            edges[key] = key
    return compute_data_layout(list(nodes.values()), list(edges.values()), graphs[-1].start, cancelled)


def compute_data_layout(nodes, edges, start, cancelled=lambda: False):
    lines = ['digraph G { graph [rankdir=LR, nodesep=0.45, ranksep=0.65];',
             'node [shape=circle, fontname="Arial", fontsize=12, width=0.58];',
             'edge [fontname="Arial", fontsize=11];']
    lines.append(f'{{ rank=source; n{start}; }}')
    for index, label, accepting, _ in nodes:
        lines.append(f'n{index} [label="{label}", peripheries={2 if accepting else 1}];')
    # Unique ASCII labels make parallel-edge identity unambiguous in plain output.
    # Width reservation uses a label padded to the actual visible label length.
    for index, (source, target, label) in enumerate(edges):
        reservation = f"e{index}" + "_" * max(0, min(len(label), 34) - len(f"e{index}"))
        lines.append(f'n{source} -> n{target} [label="{reservation}"];')
    lines.append("}")
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    process = subprocess.Popen([find_dot(), "-Tplain"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, creationflags=flags)
    payload = "\n".join(lines).encode()
    deadline = time.monotonic() + 25
    try:
        while True:
            if cancelled():
                raise Cancelled("布局已取消")
            if time.monotonic() > deadline:
                raise RuntimeError("图形布局超过 25 秒，请缩小表达式")
            try:
                output, errors = process.communicate(payload, timeout=0.1)
                break
            except subprocess.TimeoutExpired:
                payload = None
        if process.returncode:
            raise RuntimeError("Graphviz 布局失败：" + errors.decode("utf-8", errors="replace")[:400])
    finally:
        if process.poll() is None:
            process.kill()
            process.communicate()
    positions, curves = {}, {}
    height = 0.0
    for line in output.decode("utf-8").splitlines():
        parts = shlex.split(line)
        if not parts:
            continue
        if parts[0] == "graph":
            height = float(parts[3]) * 90
        elif parts[0] == "node":
            positions[int(parts[1][1:])] = (float(parts[2]) * 90, height - float(parts[3]) * 90,
                                             float(parts[4]) * 90, float(parts[5]) * 90)
        elif parts[0] == "edge":
            count = int(parts[3])
            base = 4 + count * 2
            index = int(parts[base].split("_", 1)[0][1:])
            points = [(float(parts[4 + 2*i]) * 90, height - float(parts[5 + 2*i]) * 90) for i in range(count)]
            curves[index] = (points, float(parts[base + 1]) * 90, height - float(parts[base + 2]) * 90)
    if len(positions) != len(nodes) or len(curves) != len(edges):
        raise RuntimeError("Graphviz 返回了不完整的布局")
    return nodes, edges, start, positions, curves
