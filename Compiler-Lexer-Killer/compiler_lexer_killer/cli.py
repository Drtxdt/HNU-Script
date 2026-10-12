import argparse
import json
import sys
from dataclasses import asdict
from pathlib import Path
from .core import compile, simulate, compile_lexer, tokenize, minimize, check_equivalence, CompileError
from .core.model import charset_label, set_label
from .project import load_project
from .exporting import dfa_data


def main(argv=None):
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, 'reconfigure'):
            stream.reconfigure(encoding='utf-8')
    parser = argparse.ArgumentParser(description='词法分析实验室 2.0')
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument('--regex', help='单表达式')
    source.add_argument('--definitions', type=Path, help='UTF-8 命名定义文件')
    source.add_argument('--project', type=Path, help='.clkproj 工程文件')
    source.add_argument('--rules', type=Path, help='含 definitions 和 rules 的 JSON')
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--lex', action='store_true', help='使用工程内的词法规则')
    mode.add_argument('--drawing', action='store_true', help='使用工程内的画布')
    parser.add_argument('--entry')
    inputs = parser.add_mutually_exclusive_group()
    inputs.add_argument('--input')
    inputs.add_argument('--input-file', type=Path, help='UTF-8 文本，保留原始换行')
    parser.add_argument('--construction', choices=['thompson', 'simplified'], default='thompson')
    parser.add_argument('--minimize', action='store_true')
    parser.add_argument('--recover', action='store_true')
    compare = parser.add_mutually_exclusive_group()
    compare.add_argument('--compare-regex')
    compare.add_argument('--compare', type=Path, help='与另一工程的画布比较')
    parser.add_argument('--categories', action='store_true')
    parser.add_argument('--json', action='store_true', dest='json_output')
    parser.add_argument('--trace', action='store_true')
    args = parser.parse_args(argv)
    try:
        if (args.lex or args.drawing) and not args.project:
            raise CompileError('--lex / --drawing 需要 --project')
        data = load_project(args.project) if args.project else {}
        lexer = args.lex or args.rules is not None
        if args.rules:
            if args.rules.stat().st_size > 8*1024*1024:
                raise CompileError('规则文件超过 8 MiB')
            data = json.loads(args.rules.read_text(encoding='utf-8-sig'))
        compilation, steps = None, ()
        if args.drawing:
            from .core.manual import machine_from_document
            from .core import determinize
            from .core.model import NFA, Context, Limits
            machine = machine_from_document(data.get('drawing', {}))
            dfa = determinize(machine, Context(Limits()))[0] if isinstance(machine, NFA) else machine
        elif lexer:
            compilation = compile_lexer(data.get('definitions', ''), data.get('rules', []), construction=args.construction)
            dfa = compilation.dfa
        else:
            text = args.definitions.read_text(encoding='utf-8-sig') if args.definitions else args.regex if args.regex is not None else data.get('source', '')
            mode = 'definitions' if args.definitions else data.get('mode', 'expression') if args.project else 'expression'
            compilation = compile(text, mode, args.entry or data.get('entry') or None, construction=args.construction)
            dfa = compilation.dfa
        if compilation:
            steps = compilation.nfa_steps + compilation.dfa_steps
        if args.minimize:
            result = minimize(dfa)
            dfa, steps = result.dfa, steps + result.steps
        output, code = {'dfa': dfa_data(dfa)}, 0
        if compilation and hasattr(compilation, 'diagnostics'):
            output['diagnostics'] = compilation.diagnostics
        if args.compare is not None or args.compare_regex is not None:
            if args.compare:
                from .core.manual import machine_from_document
                other = machine_from_document(load_project(args.compare).get('drawing', {}))
            else:
                other = compile(args.compare_regex).dfa
            comparison = check_equivalence(dfa, other, compare_categories=args.categories)
            output['equivalence'] = asdict(comparison)
            code = 0 if comparison.equivalent is True else 1 if comparison.equivalent is False else 2
        text = args.input
        if args.input_file:
            with args.input_file.open(encoding='utf-8-sig', newline='') as stream:
                text = stream.read(65537)
            if len(text) > 65536:
                raise CompileError('输入文件超过 65,536 字符')
        if text is not None:
            run = tokenize(dfa, text, args.recover) if lexer else simulate(dfa, text)
            output['scan' if lexer else 'simulation'] = asdict(run)
            code = max(code, 0 if (not run.errors if lexer else run.accepted) else 1)
        if args.trace:
            output['steps'] = [s.message for s in steps]
        if args.json_output:
            print(json.dumps(output, ensure_ascii=False, indent=2))
        else:
            if compilation and hasattr(compilation, 'ast'):
                print('AST')
                stack = [(compilation.ast, 0)]
                while stack:
                    node, depth = stack.pop()
                    print('  '*depth + node.kind + (' '+node.name if node.name else '') +
                          (' '+charset_label(node.chars) if node.kind == 'chars' else ''))
                    stack.extend((child, depth+1) for child in reversed(node.children))
            print(f'DFA: {len(dfa.states)} states')
            print('状态\t来源 NFA / DFA 成员\t类别\t' + '\t'.join(map(charset_label, dfa.alphabet)))
            for state, row in zip(dfa.states, dfa.transitions):
                print(f'S{state.id}' + ('*' if state.accepting else '') + '\t' + set_label(state.members or state.nfa_states) + '\t' +
                      (state.winner.output if state.winner else '') + '\t' + '\t'.join(f'S{i}' for i in row))
            for key in ('diagnostics', 'equivalence', 'scan', 'simulation', 'steps'):
                if key in output:
                    print(json.dumps(output[key], ensure_ascii=False, indent=2))
        return code
    except (CompileError, OSError, ValueError, TypeError, KeyError) as exc:
        if args.json_output:
            print(json.dumps({'error': str(exc), 'source_id': getattr(exc, 'source_id', None)}, ensure_ascii=False))
        else:
            print(str(exc), file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())