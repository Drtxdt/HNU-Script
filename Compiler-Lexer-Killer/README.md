# Compiler-Lexer-Killer · 词法分析实验室

离线中文桌面工具，将正则表达式或命名定义变成 AST、Thompson NFA、DFA，并展示每一步构造和字符串模拟。

## 直接运行

Windows 10（1809 或更新）/ Windows 11，x64。解压整个 `Compiler-Lexer-Killer-1.0.0-win64.zip`，双击目录中的 `Compiler-Lexer-Killer.exe`。请保留同目录 `_internal` 文件夹。便携包携带 Python、Qt、Graphviz，无需额外安装。

选择示例 → 点击“生成自动机” → 使用上一步、下一步、播放或进度条观察推导。三个页面分别展示 NFA 构造、DFA 子集构造和完整字符串匹配。AST 节点可以定位构造步骤，DFA 状态可以查看 NFA 集合。滚轮缩放，拖动画布平移。

源码、构建脚本和测试保留在仓库，便携包同时附 `source.zip`。程序不联网，不上传输入。

## 源码启动

Python 3.10+；便携包使用 Python 3.11.8 x64 构建。以下 PowerShell 命令在本工具目录执行：

```powershell
py -3.11 -m venv .venv
.\.venv\Scripts\python.exe -m pip install -r requirements.txt
.\.venv\Scripts\python.exe tools/prepare_graphviz.py
.\.venv\Scripts\python.exe main.py
```

Graphviz 准备脚本从官方 GitLab 下载固定版本 12.2.1，并核对官方 SHA256，仅写入工具的 `vendor` 目录。也可自行安装 Graphviz 到 PATH，或将 `GRAPHVIZ_DOT` 设为 `dot.exe` 的完整路径。CLI 和算法核心不需要 Qt 或 Graphviz。

## 输入规则

### 单表达式

```text
(a|b)*abb
(a|b)*·a·b·b
[a-zA-Z_][a-zA-Z0-9_]*
```

相邻字符表示连接。运算优先级为 `* + ?`、连接、`|`。支持显式连接符 `·`、`∙`、`⋅`。

### 课件式命名定义

```text
letter → [A-Za-z]
digit  → [0-9]
id     → letter(letter|digit)*
```

每行一个定义，接受 `→` 和 `->`。名称以 ASCII 字母或下划线开头，之后允许字母、数字和下划线。裸名称是引用；字面文本加单引号或双引号，如 `keyword → 'if'`。两个裸名称之间需要空格，例如 `pair → letter letter`；`letterletter` 是一个名称。

只允许引用之前定义的名称，禁止重复定义、自引用和向后引用。默认入口为最后一项，也可从入口下拉框选择。多个定义是正则复用，不代表多个 token 类别。

### 字符与空白

| 写法 | 含义 |
| --- | --- |
| `ε`、`''` | 空串 |
| `a*` / `a+` / `a?` | 零次以上 / 一次以上 / 零次或一次 |
| `[a-z]` / `[abc]` | ASCII 字符范围 / 字符集合 |
| `[^a]` | ASCII 0–127 中除 a 外的字符 |
| `\n` `\r` `\t` `\xHH` | 换行、回车、制表符、两位十六进制 ASCII 字符 |
| `\*` `\|` `\[` `\\` | 字面特殊字符 |
| `.` | 字面句点，不是通配符 |

引号和字符类外的空白忽略，匹配空格请写 `' '`、`[ ]` 或 `\x20`。GUI 字符串输入不做 trim，编辑器换行按 LF 处理；CLI / API 可精确输入 CR 等控制字符。课件 `/n`、`/t` 等写法需换成反斜杠。

不支持 `\d`、锚点、计数量词、回溯引用、断言、惰性量词、Unicode 匹配字符。空表达式不等于空串，空串请写 `ε`。

## 如何理解结果

- Thompson 构造为各子表达式分配独立状态，连接使用 ε 边。不会为了复制课件编号而合并状态。
- NFA 构造中蓝色节点标出当前片段的入口和出口，绿色双圈为完整自动机的接受状态。
- `move(S, a)` 只经过字符 a 的边；`ε-closure(S)` 包括集合自身和沿 ε 边可达的全部状态。
- 字符集合按相同的边匹配条件划分为不重叠组，DTran 每列对应一组。悬停表头可查看所有字符。
- DFA 的 NFA 状态集合中包含接受状态时，该 DFA 状态也接受。可达空集合显式显示为陷阱状态。
- 匹配为完整匹配，不是子串搜索；输入全部读取后必须停在接受状态。
- 每次构造采用固定顺序。不同构造方法的状态编号、数量可以不同，语言仍然等价。

## CLI 与 API

```powershell
python -m compiler_lexer_killer.cli --regex '(a|b)*abb' --input abababb --trace
python -m compiler_lexer_killer.cli --definitions rules.txt --entry id --input initial10
```

定义文件为 UTF-8，允许 BOM。退出码：0 为构造成功或匹配成功，1 为匹配失败，2 为语法／文件错误。

```python
from compiler_lexer_killer.core import compile, simulate, Limits

result = compile('(a|b)*abb', mode='expression')
run = simulate(result.dfa, 'abababb')
assert run.accepted
# result.ast / .nfa / .dfa / .nfa_steps / .dfa_steps
```

`mode` 为 `expression` 或 `definitions`；`entry` 可指定定义入口。API 另有 `limits=Limits(...)` 和 `cancelled=callable` 参数。数据和步骤为不可变 dataclass，错误包含从 1 起算的行列、从 0 起算的源区间；模拟失败位置从 0 起算，等于输入长度时表示 EOF 未接受。

默认上限：512 个 NFA 状态、256 个 DFA 状态、20,000 条构造步骤、65,536 个源字符；GUI 模拟上限 4096 字符。超限报错，不输出部分自动机。

## 测试与构建

```powershell
python -m pip install -r requirements-lock.txt
python tools/prepare_graphviz.py
python -m pytest -q
python tools/build_portable.py
```

测试包括解析、错误位置、引用隔离、ASCII 字符类、ε 环、陷阱状态、资源限制、取消、GUI 回放，以及每个指定表达式在 `{a,b}` 长度 ≤6 的全部 127 个字符串上的 NFA/DFA 等价性。经典语言另用人工已知性质校验。

`build_portable.py` 要求 Windows / Python 3.11。它收集 Graphviz 的 dot 布局所需依赖、许可文件、源码，使用 PyInstaller 目录模式生成 `dist/Compiler-Lexer-Killer` 和 ZIP / SHA256。构建时会隔离 PATH，防止本机其他 SDK 的同名 DLL 被误收集，并统一随 Qt 分发的 VC 运行库。

便携包支持 `--smoke-test`：在真实 Qt 事件循环中构造经典例题、回放、模拟并输出 `smoke-result.json` 和截图后退出。可用环境变量 `LEXER_SMOKE_OUTPUT` 指定报告路径。`python tools/check_portable.py` 会解压 ZIP 到中文路径，以隔离 PATH 和 100%／150% 缩放执行此检查。干净 Windows VM 验收时，应解压后运行自检并确认报告 `ok: true`；清空 PATH 的本机验证不能替代全新系统验证。

## 版本边界

首版不含多规则分词、最长匹配、规则优先级、类别输出、最小化、图形导出或练习判题。课件页码基于用户提供的《chaper 2 - 词法分析2026.pptx》，内置示例仅收录教学表达式和说明，不附原课件。

依赖和版权说明见 `THIRD_PARTY_NOTICES.md`。
