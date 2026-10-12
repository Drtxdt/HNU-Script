"""Editable classroom presets; categories are independent of rule names."""
from ..core.model import RuleSpec

LEXER_PRESETS = (
    ("课堂 A 组 · reserved / id", "letter -> [A-Za-z]", (
        RuleSpec("IF", "'if'", "reserved"), RuleSpec("INT", "'int'", "reserved"),
        RuleSpec("ID", "letter+", "id"), RuleSpec("WS", r"[ \t\r\n]+", skip=True)),
     "if iff int integer", "课件 136–145、155 页：最长匹配优先；同长时预留字排在 ID 前。"),
    ("课堂 B 组 · 数值类别", "digit -> [0-9]\ndigits -> digit+", (
        RuleSpec("SCIENTIFIC", "digits('.'digits)?'E'('+'|'-')?digits", "scientific"),
        RuleSpec("FLOAT", "digits'.'digits", "float"), RuleSpec("INTEGER", "digits", "integer"),
        RuleSpec("WS", r"[ \t\r\n]+", skip=True)),
     "12 12.1 12.1E3 01 3E-2", "课件 40、45、155 页：大写 E；允许前导零，不接受尾随小数点。"),
    ("综合示例 · 关键字、数值与注释", "letter -> [A-Za-z_]\ndigit -> [0-9]\ndigits -> digit+", (
        RuleSpec("IF", "'if'", "reserved"), RuleSpec("INT", "'int'", "reserved"),
        RuleSpec("ID", "letter(letter|digit)*", "id"),
        RuleSpec("NUMBER", "digits('.'digits)?('E'('+'|'-')?digits)?", "number"),
        RuleSpec("LE", "'<='"), RuleSpec("LT", "'<'"), RuleSpec("INC", "'++'"), RuleSpec("PLUS", "'+'"),
        RuleSpec("LPAREN", "'('"), RuleSpec("RPAREN", "')'"), RuleSpec("ASSIGN", "'='"), RuleSpec("SEMI", "';'"),
        RuleSpec("COMMENT", r"'//'[^\r\n]*", skip=True), RuleSpec("WS", r"[ \t\r\n]+", skip=True)),
     "if(initial<=10)\nint count=12; // example\ncount++;", "课件 8–10、43、50 页：显式跳过空白和行注释，保留原文位置。"),
)
