from .model import (CompileError, Cancelled, Limits, Context, Compilation,
                    AST, NFA, DFA, Step, SimulationResult)
from .parser import parse
from .automata import thompson, determinize, epsilon_closure, move, nfa_match, simulate
from .simplified import simplified
from .lexer import compile_lexer, tokenize
from .analysis import minimize, check_equivalence
from .model import RuleSpec


def compile(source: str, mode: str = "expression", entry: str | None = None,
            *, limits: Limits | None = None, cancelled=None, construction="thompson") -> Compilation:
    context = Context(limits or Limits(), cancelled)
    context.check()
    ast, definitions, chosen = parse(source, mode, entry, context)
    if construction not in ("thompson", "simplified"):
        raise CompileError("未知构造策略")
    nfa, nfa_steps = (simplified if construction == "simplified" else thompson)(ast, context)
    dfa, dfa_steps = determinize(nfa, context)
    return Compilation(source, ast, definitions, chosen, nfa, dfa, nfa_steps, dfa_steps)


def grade(question, answer, **kwargs):
    from .exercises import grade as evaluate
    return evaluate(question, answer, **kwargs)
