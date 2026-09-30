"""Handwritten scanner and iterative precedence parser. No regex engine."""
from __future__ import annotations

from dataclasses import dataclass
from .model import ASCII, AST, CompileError, Context, Span


@dataclass(frozen=True)
class Token:
    kind: str
    span: Span
    value: object = None


def name_start(c):
    return c == "_" or "a" <= c <= "z" or "A" <= c <= "Z"


def name_char(c):
    return name_start(c) or "0" <= c <= "9"


class Scanner:
    def __init__(self, text, offset=0, line=1, column=1, named=False):
        self.text, self.i = text, 0
        self.offset, self.line, self.column = offset, line, column
        self.named = named

    def span(self, start, end=None):
        prefix = self.text[:start]
        lines = prefix.count("\n")
        col = len(prefix.rsplit("\n", 1)[-1]) + 1 if lines else self.column + start
        return Span(self.offset + start, self.offset + (self.i if end is None else end), self.line + lines, col)

    def error(self, message, start):
        raise CompileError(message, self.span(start))

    def character(self):
        start = self.i
        c = self.text[self.i]
        self.i += 1
        if c == "\\":
            if self.i == len(self.text):
                self.error("转义符后缺少字符", start)
            c = self.text[self.i]
            self.i += 1
            escapes = {"n": "\n", "r": "\r", "t": "\t"}
            if c in escapes:
                c = escapes[c]
            elif c == "x":
                digits = self.text[self.i:self.i + 2]
                if len(digits) != 2 or any(v not in "0123456789abcdefABCDEF" for v in digits):
                    self.error(r"\x 后需要两位十六进制数", start)
                c = chr(int(digits, 16))
                self.i += 2
            elif c.isalnum():
                self.error("不支持此转义；请使用字符类或字面字符", start)
        if c not in ASCII:
            self.error("匹配字符必须属于 ASCII 0–127", start)
        return c

    def tokens(self, context=None):
        result = []
        while self.i < len(self.text):
            if context:
                context.check()
            start = self.i
            c = self.text[self.i]
            if c in " \t\r\n":
                self.i += 1
                continue
            if c in "|()*+?·∙⋅":
                self.i += 1
                result.append(Token("·" if c in "·∙⋅" else c, self.span(start)))
            elif c == "ε":
                self.i += 1
                result.append(Token("atom", self.span(start), AST("epsilon", self.span(start))))
            elif c in "'\"":
                self.i += 1
                children = []
                while self.i < len(self.text) and self.text[self.i] != c:
                    pos = self.i
                    char = self.character()
                    children.append(AST("chars", self.span(pos), chars=frozenset([char])))
                if self.i == len(self.text):
                    self.error("引号未闭合", start)
                self.i += 1
                node = AST("concat" if children else "epsilon", self.span(start), tuple(children))
                result.append(Token("atom", self.span(start), node))
            elif c == "[":
                self.i += 1
                negate = self.i < len(self.text) and self.text[self.i] == "^"
                if negate:
                    self.i += 1
                chars = set()
                while self.i < len(self.text) and self.text[self.i] != "]":
                    pos = self.i
                    first = self.character()
                    if self.i + 1 < len(self.text) and self.text[self.i] == "-" and self.text[self.i + 1] != "]":
                        self.i += 1
                        last = self.character()
                        if ord(first) > ord(last):
                            self.error("字符范围的起点不能大于终点", pos)
                        chars.update(map(chr, range(ord(first), ord(last) + 1)))
                    else:
                        chars.add(first)
                if self.i == len(self.text):
                    self.error("字符类缺少右方括号 ]", start)
                self.i += 1
                if not chars:
                    self.error("字符类不能为空", start)
                values = ASCII - chars if negate else frozenset(chars)
                result.append(Token("atom", self.span(start), AST("chars", self.span(start), chars=frozenset(values))))
            elif self.named and name_start(c):
                self.i += 1
                while self.i < len(self.text) and name_char(self.text[self.i]):
                    self.i += 1
                result.append(Token("name", self.span(start), self.text[start:self.i]))
            else:
                if c in "]{}^$" or (self.named and c != "\\"):
                    self.error("不支持的语法；命名定义中的字面文字请加引号，特殊字符请转义", start)
                char = self.character()
                result.append(Token("atom", self.span(start), AST("chars", self.span(start), chars=frozenset([char]))))
        return result


def parse_expression(text, definitions=None, offset=0, line=1, column=1, context=None):
    scanner = Scanner(text, offset, line, column, definitions is not None)
    tokens = scanner.tokens(context)
    values, operators = [], []
    expected = True
    previous = None
    priorities = {"|": 1, "·": 2}

    def reduce():
        operator = operators.pop()
        if len(values) < 2:
            raise CompileError("运算符缺少操作数", operator.span)
        right, left = values.pop(), values.pop()
        values.append(AST("union" if operator.kind == "|" else "concat",
                          Span(left.span.start, right.span.end, left.span.line, left.span.column), (left, right)))

    def binary(token):
        while operators and operators[-1].kind != "(" and priorities[operators[-1].kind] >= priorities[token.kind]:
            reduce()
        operators.append(token)

    for token in tokens:
        if context:
            context.check()
        kind = token.kind
        if kind in ("atom", "name", "("):
            if not expected:
                binary(Token("·", token.span))
            if kind == "(":
                operators.append(token)
                expected = True
            else:
                if kind == "name":
                    if token.value not in definitions:
                        raise CompileError(f"名称 {token.value!r} 未在前面定义；不允许自引用或向后引用", token.span)
                    values.append(AST("reference", token.span, (definitions[token.value],), name=token.value))
                else:
                    values.append(token.value)
                expected = False
        elif kind == ")":
            if expected:
                raise CompileError("右括号前缺少表达式；空串请使用 ε", token.span)
            while operators and operators[-1].kind != "(":
                reduce()
            if not operators:
                raise CompileError("右括号没有对应的左括号", token.span)
            opening = operators.pop()
            child = values.pop()
            values.append(AST("group", Span(opening.span.start, token.span.end, opening.span.line, opening.span.column), (child,)))
            expected = False
        elif kind in "*+?":
            if expected or previous in ("*", "+", "?"):
                raise CompileError("重复运算缺少操作数，或连续使用了重复运算符", token.span)
            child = values.pop()
            values.append(AST({"*": "star", "+": "plus", "?": "optional"}[kind],
                              Span(child.span.start, token.span.end, child.span.line, child.span.column), (child,)))
        else:
            if expected:
                raise CompileError("连接或并运算缺少左操作数", token.span)
            binary(token)
            expected = True
        previous = kind
    if expected:
        raise CompileError("表达式为空或缺少右操作数", scanner.span(len(text), len(text)))
    while operators:
        if operators[-1].kind == "(":
            raise CompileError("左括号未闭合", operators[-1].span)
        reduce()
    return values[0]


def parse(source, mode="expression", entry=None, context: Context | None = None):
    if len(source) > 65_536:
        raise CompileError("输入超过 65,536 个字符，请缩小定义")
    if mode == "expression":
        return parse_expression(source, context=context), (), None
    if mode != "definitions":
        raise CompileError("未知输入模式")
    definitions = {}
    offset = 0
    for number, raw in enumerate(source.splitlines(keepends=True), 1):
        if context:
            context.check()
        line = raw.rstrip("\r\n")
        if line.strip():
            index = line.find("→")
            arrow = 1
            ascii_index = line.find("->")
            if ascii_index >= 0 and (index < 0 or ascii_index < index):
                index, arrow = ascii_index, 2
            if index < 0:
                raise CompileError("命名定义需要 → 或 ->", Span(offset, offset + len(line), number, 1))
            name = line[:index].strip()
            if not name or not name_start(name[0]) or not all(name_char(c) for c in name):
                raise CompileError("名称只能包含 ASCII 字母、数字、下划线，且不能以数字开头", Span(offset, offset + index, number, 1))
            if name in definitions:
                raise CompileError(f"名称 {name!r} 重复定义", Span(offset, offset + index, number, 1))
            begin = index + arrow
            definitions[name] = parse_expression(line[begin:], definitions, offset + begin, number, begin + 1, context)
        offset += len(raw)
    if not definitions:
        raise CompileError("请至少输入一行命名定义")
    chosen = entry or next(reversed(definitions))
    if chosen not in definitions:
        raise CompileError(f"入口 {chosen!r} 不存在")
    return definitions[chosen], tuple(definitions), chosen
