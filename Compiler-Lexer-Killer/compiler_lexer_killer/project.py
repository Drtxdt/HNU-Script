"""Versioned project storage; no executable serialization or cached algorithms."""
import json
import os
import tempfile
from pathlib import Path
from .core.model import CompileError
from .core.manual import validate_document


def validate(data):
    if not isinstance(data, dict) or type(data.get("format_version")) is not int or data["format_version"] != 1:
        raise CompileError("未知工程格式版本（需要 format_version=1）")
    if not isinstance(data.get("source", ""), str) or len(data.get("source", "")) > 65_536:
        raise CompileError("工程表达式格式错误或过长")
    if data.get("mode", "expression") not in ("expression", "definitions"):
        raise CompileError("工程输入模式无效")
    for key in ("input", "lexer_input", "definitions"):
        if not isinstance(data.get(key, ""), str) or len(data.get(key, "")) > 65_536:
            raise CompileError(f"工程 {key} 格式错误或过长")
    if not isinstance(data.get("rules", []), list) or len(data.get("rules", [])) > 512:
        raise CompileError("工程规则格式错误")
    for rule in data.get("rules", []):
        if not isinstance(rule, dict) or set(rule) - {"name", "pattern", "category", "skip", "enabled"}:
            raise CompileError("规则字段无效")
        for key in ("name", "pattern", "category"):
            if not isinstance(rule.get(key, ""), str) or len(rule.get(key, "")) > 65_536:
                raise CompileError("规则文字字段无效")
        for key in ("skip", "enabled"):
            if key in rule and type(rule[key]) is not bool:
                raise CompileError("规则开关格式无效")
    if "drawing" in data:
        validate_document(data["drawing"])
    if not isinstance(data.get("exercise", {}), dict) or not isinstance(data.get("mistakes", []), list):
        raise CompileError("练习记录格式错误")
    def exercise(record):
        if not isinstance(record, dict) or not isinstance(record.get("answer", ""), str):
            raise CompileError("练习答案格式错误")
        question = record.get("question")
        if question is not None:
            from .core.exercises import KINDS
            if (not isinstance(question, dict) or question.get("kind") not in KINDS
                    or type(question.get("seed")) is not int or not 0 <= question["seed"] <= 2_000_000_000
                    or question.get("difficulty") not in ("basic", "advanced")):
                raise CompileError("题目种子、难度或题型无效")
        if "drawing" in record:
            validate_document(record["drawing"])
    exercise(data.get("exercise", {}))
    if len(data.get("mistakes", [])) > 100:
        raise CompileError("错题记录超过 100 条")
    for record in data.get("mistakes", []):
        exercise(record)
        if not record.get("question"):
            raise CompileError("错题缺少题目")
    view = data.get("view", {})
    if (not isinstance(view, dict) or type(view.get("tab", 0)) is not int or not 0 <= view.get("tab", 0) <= 6
            or view.get("sim_source", "regular") not in ("regular", "minimized", "drawing", "lexer")):
        raise CompileError("工程视图状态无效")
    if not isinstance(data.get("entry", ""), str) or type(data.get("recover", False)) is not bool:
        raise CompileError("工程入口或错误恢复选项无效")
    return data


def load_project(path):
    path = Path(path)
    if path.stat().st_size > 8 * 1024 * 1024:
        raise CompileError("工程文件超过 8 MiB")
    try:
        return validate(json.loads(path.read_text(encoding="utf-8-sig")))
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise CompileError("工程文件不是有效的 UTF-8 JSON") from exc


def save_project(path, data):
    validate(data)
    path = Path(path)
    payload = json.dumps(data, ensure_ascii=False, indent=2, allow_nan=False)
    if len(payload.encode("utf-8")) > 8*1024*1024:
        raise CompileError("工程文件超过 8 MiB")
    descriptor, temporary = tempfile.mkstemp(prefix=".clk-", suffix=".tmp", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
