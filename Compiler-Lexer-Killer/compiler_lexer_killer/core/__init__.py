from .model import (CompileError, Cancelled, Limits, Context, Compilation,
                    AST, NFA, DFA, Step, SimulationResult)
from .parser import parse
from .automata import thompson, determinize, epsilon_closure, move, nfa_match, simulate


def compile(source: str, mode: str = "expression", entry: str | None = None,
            *, limits: Limits | None = None, cancelled=None) -> Compilation:
    context = Context(limits or Limits(), cancelled)
    context.check()
    ast, definitions, chosen = parse(source, mode, entry, context)
    nfa, nfa_steps = thompson(ast, context)
    dfa, dfa_steps = determinize(nfa, context)
    return Compilation(source, ast, definitions, chosen, nfa, dfa, nfa_steps, dfa_steps)
