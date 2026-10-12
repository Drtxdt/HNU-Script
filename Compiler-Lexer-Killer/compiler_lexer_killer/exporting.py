import csv
import json
from dataclasses import asdict
from pathlib import Path
from .core.model import charset_label
from . import __version__


def dfa_data(dfa):
    return {"alphabet": [sorted(ord(c) for c in group) for group in dfa.alphabet], "start": dfa.start,
            "states": [{"id": s.id, "accepting": s.accepting, "nfa_states": sorted(s.nfa_states),
                        "members": sorted(s.members), "candidates": [asdict(r) for r in s.candidates]} for s in dfa.states],
            "transitions": [list(row) for row in dfa.transitions]}


def write_data(path, dfa=None, run=None):
    path = Path(path)
    if path.suffix.lower() == ".json":
        value = {"dfa": dfa_data(dfa) if dfa else None, "scan": asdict(run) if run else None}
        path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")
    else:
        with path.open("w", encoding="utf-8-sig", newline="") as stream:
            writer = csv.writer(stream)
            if run:
                writer.writerow(["原文", "规则", "类别", "start", "end", "行", "列", "跳过"])
                for t in sorted(run.tokens+run.skipped, key=lambda t:t.start):
                    writer.writerow([t.text, t.rule, t.category, t.start, t.end, t.line, t.column, t.skip])
            elif dfa:
                writer.writerow(["状态", "接受", "类别"] + [charset_label(g) for g in dfa.alphabet])
                for s in dfa.states:
                    writer.writerow([s.id, s.accepting, s.winner.output if s.winner else ""] + list(dfa.transitions[s.id]))


def write_report(path, project, dfa=None, run=None, image_name=None, steps=(), note="", *, context="regular", strategy="标准 Thompson 构造", conclusion=""):
    def block(text):
        longest = current = 0
        for char in str(text):
            current = current+1 if char == "~" else 0
            longest = max(longest, current)
        marker = "~" * max(3, longest+1)
        return marker + "text\n" + str(text) + "\n" + marker
    source = (project.get("source", "") if context == "regular" else project.get("definitions", "") if context == "lexer"
              else json.dumps(project.get("exercise" if context == "exercise" else "drawing", {}), ensure_ascii=False, indent=2))
    lines = ["# 词法分析实验报告", "", f"工具：Compiler-Lexer-Killer {__version__}；匹配字符域 ASCII 0–127。", "",
             "## 输入与构造策略", "", f"来源：{context}；策略：{strategy}。", "",
             block(source), "", note, "", "状态编号与布局不影响语言。课件 NFA 简化不保证全局最少状态。", ""]
    if context == "lexer" and project.get("rules"):
        lines.extend(["## 词法规则", "", block(project.get("definitions", "")), "", block(json.dumps(project["rules"], ensure_ascii=False, indent=2)), ""])
    if image_name:
        from urllib.parse import quote
        lines.extend(["## 自动机图", "", f"![自动机]({quote(image_name)})", ""])
    if conclusion:
        lines.extend(["## 结论", "", conclusion, ""])
    if dfa:
        lines.extend(["## DFA 转换表", "", "| 状态 | 接受 | 类别 | " + " | ".join(charset_label(g).replace('|', '\\|') for g in dfa.alphabet) + " |",
                      "|" + " --- |"*(3+len(dfa.alphabet))])
        for s in dfa.states:
            values = [f"S{s.id}", "是" if s.accepting else "否", s.winner.output if s.winner else "—"] + [f"S{t}" for t in dfa.transitions[s.id]]
            lines.append("| " + " | ".join(v.replace("|", "\\|").replace("\n", " ") for v in values) + " |")
    if run:
        lines.extend(["", "## 词法分析结果", "", f"扫描{'完成' if run.complete else '遇错停止'}，{len(run.tokens)} 个 Token，{len(run.errors)} 个错误。", "",
                      block(json.dumps(asdict(run), ensure_ascii=False, indent=2))])
    lines.extend(["", "## 关键步骤", ""])
    lines.extend(f"{i+1}. {step.message}" for i, step in enumerate(steps))
    Path(path).write_text("\n".join(lines), encoding="utf-8")
