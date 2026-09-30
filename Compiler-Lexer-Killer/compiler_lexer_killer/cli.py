import argparse
import sys
from pathlib import Path
from .core import compile, simulate, CompileError
from .core.model import charset_label, set_label


def main(argv=None):
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description="词法分析实验室：正则 → NFA → DFA")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--regex", help="单个正则表达式")
    source.add_argument("--definitions", type=Path, help="UTF-8 命名定义文件")
    parser.add_argument("--entry", help="命名定义入口，默认最后一项")
    parser.add_argument("--input", help="完整匹配的字符串")
    parser.add_argument("--trace", action="store_true", help="输出构造步骤")
    args = parser.parse_args(argv)
    try:
        text = args.definitions.read_text(encoding="utf-8-sig") if args.definitions else args.regex
        result = compile(text, "definitions" if args.definitions else "expression", args.entry)
        print("AST")
        stack = [(result.ast, 0)]
        while stack:
            node, depth = stack.pop()
            print("  " * depth + node.kind + (" " + node.name if node.name else "") +
                  (" " + charset_label(node.chars) if node.kind == "chars" else ""))
            stack.extend((child, depth + 1) for child in reversed(node.children))
        print(f"NFA: {len(result.nfa.states)} states; DFA: {len(result.dfa.states)} states")
        print("状态\tNFA 集合\t" + "\t".join(map(charset_label, result.dfa.alphabet)))
        for state, row in zip(result.dfa.states, result.dfa.transitions):
            print(f"S{state.id}{'*' if state.accepting else ''}\t{set_label(state.nfa_states)}\t" + "\t".join(f"S{i}" for i in row))
        if args.trace:
            for step in result.nfa_steps + result.dfa_steps:
                print(step.message)
        if args.input is not None:
            run = simulate(result.dfa, args.input)
            for step in run.steps:
                print(step.message)
            return 0 if run.accepted else 1
        return 0
    except (CompileError, OSError) as exc:
        print(str(exc), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
