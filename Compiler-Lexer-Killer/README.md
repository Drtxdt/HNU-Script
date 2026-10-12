# Compiler-Lexer-Killer 2.0 · 词法分析实验室

离线中文桌面工具：正则／命名定义 → AST → NFA → DFA → 最小化 → 字符串模拟／多规则词法分析。提供自由画图、练习判题、工程保存和图文导出。

## 直接运行

支持目标为 Windows 10（1809 或更新）/ Windows 11，x64。解压整个 `Compiler-Lexer-Killer-2.0.0-win64.zip`，双击目录中的 `Compiler-Lexer-Killer.exe`。请保留同目录 `_internal` 文件夹。便携包携带 Python、Qt、Graphviz，无需额外安装。实际验证范围见 `VALIDATION.md`。

选择示例 → 点击“生成自动机” → 使用上一步、下一步、播放或进度条观察推导。七个页面包括 NFA 构造、DFA 子集构造、字符串模拟、优化比较、词法分析、自由画图、练习判题。AST 节点可以定位构造步骤，DFA 状态可以查看 NFA 集合。滚轮缩放，拖动画布平移。

源码、构建脚本和测试保留在仓库，便携包同时附 `source.zip`。程序不联网，不上传输入。

## 源码启动

Python 3.11+；便携包使用 Python 3.11.8 x64 构建。以下 PowerShell 命令在本工具目录执行：

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

### 多规则词法分析

打开词法分析页，选择课堂 A 组、B 组或综合示例，点击构建，再分析文本。规则表包含启用、名称、类别、表达式和跳过开关。上下移动规则可调整优先级；类别为空时使用规则名称。

- 公共命名定义是可复用片段。规则表达式中裸名称引用公共定义，文字加引号，例如 `'if'`；规则名称不能互相引用。
- 优先选择最长匹配；长度相同选择最靠前的启用规则。算法保存最后接受位置，遇到无法继续的字符或 EOF 时回退后提交。
- 可空辅助定义合法，但启用的完整规则不能匹配空串，包括跳过规则。空白必须显式定义，例如 `[ \t\r\n]+` 并勾选跳过。
- 默认遇错停止；“遇错继续”每次记录错误并跳过一个字符。Token 区间是从零开始的 `[start,end)`，行列从一开始；API 把 CRLF 计作一次换行。
- 可切换最小化 DFA。获胜规则与输出／跳过动作在最小化后保持一致；“从未胜出”诊断提示空语言或被前面的规则遮蔽。
- 课堂 A 组的 ID 仅包含字母；综合示例允许下划线与后续数字。B 组类别为 integer、float、scientific，指数使用大写 E。

### 优化、画图与判题

优化页可选择正则、词法规则或手绘图。课件 NFA 简化根据端点入／出边条件构造，不保证全局最少 NFA，仅对具有 AST 的输入启用。DFA 最小化按接受性、获胜规则和迁移目标逐轮分组，显示成员映射。最小化使用有效字母表；例如经典 `(a|b)*abb` 在 `{a,b}` 上得到四状态 DFA，其他 ASCII 字符直接拒绝。

画图页双击空白新增状态；在连线模式依次点击两个状态，点击自身可画自环。表格编辑初态、终态、类别及边；DFA 不允许 ε 边或冲突迁移。边条件只能是单字符、转义、字符类或 ε，不能直接填整个多字符正则。使用平移模式拖动画布；选择模式支持框选、Ctrl 多选、复制粘贴、删除、撤销重做。自动布局也是可撤销操作。

画布菜单可复制已生成的自动机，也可与当前正则进行语言比较。手绘 NFA 验证后接入 NFA → DFA 的步骤页及字符串模拟，手绘 DFA 接入模拟和最小化。手绘 NFA 的类别终态如存在同词冲突，按状态表顺序选择类别，编号和坐标本身不参与匹配。

练习包含 ε 闭包、move、DTran、最小化分组、Token 分类、等价画图、最小 DFA 和课堂 A 组类别题。输入 JSON 答案或直接提交画布。相同题型、难度和种子会生成相同题目。集合忽略顺序，自动机按语言判题，最小化题还检查状态数；乘积自动机发现不等价时返回最短反例，空字符串也是合法反例。资源不足显示未完成判定。错题保存最近 100 条。

### 工程与导出

文件菜单保存 `.clkproj`（带版本的 UTF-8 JSON），包含输入、规则、测试文本、画布坐标、题目答案、错题和页面选择。算法结果在打开后重新生成。目录 `examples` 提供三个可打开的课堂／综合工程。

Ctrl+N / O / S 分别新建、打开、保存；Ctrl+Shift+S 另存为。退出时提示保存。每 30 秒将未保存工程原子写入 Windows 当前用户的应用数据目录，异常退出后可恢复；可用 `CLK_DATA_DIR` 指定恢复文件目录。

导出当前步骤或完整图为 SVG／PNG；转换表、Token 可导出 CSV／JSON；实验报告导出 Markdown 和同目录 SVG。CSV 为带 BOM 的 UTF-8。PNG 最长边不超过 8192 像素且不超过 2400 万像素，避免过大的图耗尽内存。

### 命令行

```powershell
python -m compiler_lexer_killer.cli --regex '(a|b)*abb' --input abababb --trace
python -m compiler_lexer_killer.cli --definitions rules.txt --entry id --input initial10
python -m compiler_lexer_killer.cli --regex '(a|b)*abb' --construction simplified --minimize --input abb --json
python -m compiler_lexer_killer.cli --project 'examples/1-课堂 A 组.clkproj' --lex --input 'if iff int' --json
python -m compiler_lexer_killer.cli --regex 'a*' --compare-regex 'a+' --json
```

定义文件为 UTF-8，允许 BOM。`--input-file` 保留文件内原始 CR/LF；`--rules` 读取包含 definitions 字符串与 rules 数组的 JSON；`--project ... --drawing` 使用手绘图；`--compare` 对比另一工程的画布，`--categories` 同时比较类别。退出码：0 成功，1 不匹配／不等价／词法错误，2 输入错误或未完成判定。

```python
from compiler_lexer_killer.core import compile, simulate, Limits

result = compile('(a|b)*abb', mode='expression')
run = simulate(result.dfa, 'abababb')
assert run.accepted
# result.ast / .nfa / .dfa / .nfa_steps / .dfa_steps

from compiler_lexer_killer.core import RuleSpec, compile_lexer, tokenize, minimize, check_equivalence, grade
lexer = compile_lexer('letter -> [A-Za-z]', [
    RuleSpec('IF', "'if'", 'reserved'),
    RuleSpec('ID', 'letter+', 'id'),
    RuleSpec('WS', r'[ \t\r\n]+', skip=True),
])
tokens = tokenize(lexer, 'if iff')
optimized = minimize(lexer.dfa)
assert check_equivalence(lexer.dfa, optimized.dfa, compare_categories=True).equivalent
```

`mode` 为 `expression` 或 `definitions`；`entry` 可指定定义入口。API 另有 `limits=Limits(...)` 和 `cancelled=callable` 参数。数据和步骤为不可变 dataclass，错误包含从 1 起算的行列、从 0 起算的源区间；模拟失败位置从 0 起算，等于输入长度时表示 EOF 未接受。

默认上限：512 个 NFA 状态、256 个 DFA 状态、20,000 条构造步骤、65,536 个源字符；GUI 模拟上限 4096 字符。超限报错，不输出部分自动机。

## 测试与构建

```powershell
python -m pip install -r requirements-lock.txt
python tools/prepare_graphviz.py
python -m pytest -q --basetemp=build/test-temp
python tools/build_portable.py
python tools/check_portable.py
```

测试包括解析、错误位置、引用隔离、ASCII 字符类、ε 环、陷阱状态、资源限制、取消、GUI 回放，以及每个指定表达式在 `{a,b}` 长度 ≤6 的全部 127 个字符串上的 NFA/DFA 等价性。经典语言另用人工已知性质校验。

`build_portable.py` 要求 Windows / Python 3.11。它收集 Graphviz 的 dot 布局所需依赖、许可文件、源码，使用 PyInstaller 目录模式生成 `dist/Compiler-Lexer-Killer` 和 ZIP / SHA256。构建时会隔离 PATH，防止本机其他 SDK 的同名 DLL 被误收集，并统一随 Qt 分发的 VC 运行库。

便携包支持 `--smoke-test`：在真实 Qt 事件循环中检查构造、优化、词法扫描、鼠标连线／自环、撤销重做、手绘转换、模拟、判题、保存恢复和导出，并输出 JSON、七个页面的截图及导出样例后退出。环境变量 `LEXER_SMOKE_OUTPUT` 指定报告路径。

`check_portable.py` 将 ZIP 解压到中文和空格路径，清除开发环境变量，以 Qt 缩放因子 1 和 1.5 分别验证。该因子叠加在系统缩放之上，不等同于 Windows 设置的 100%／150%。本次按当前电脑隔离验收，不宣称做过干净 VM 或 Windows 10 1809 实测。

## 版本边界

2.0 不包含账号、云服务、Unicode 匹配、Windows 7、现代正则扩展或扫描器源码生成。课件页码基于《chaper 2 - 词法分析2026.pptx》，内置示例收录教学表达式和说明，不附原课件。

依赖和版权说明见 `THIRD_PARTY_NOTICES.md`。
