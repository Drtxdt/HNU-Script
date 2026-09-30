# 首版验收记录

日期：2026-09-29。构建环境：Windows x64（系统构建 26200）、Python 3.11.8、PySide6 6.10.1、PyInstaller 6.16.0、Graphviz 12.2.1。

## 已完成

- `python -m pytest -q`：**81 passed**。覆盖算法、解析、引用隔离、深层表达式限制、取消任务、回放及 GUI 错误定位。
- 对十个指定表达式分别枚举 `{a,b}` 上长度不超过 6 的 127 个字符串，比较 NFA / DFA 结果；经典语言另使用独立语言性质判断。
- 全部 9 个内置示例完成解析、自动机生成、图形布局和模拟。
- 打包后的 EXE 从含中文和空格的新目录运行，PATH 仅保留 Windows System32，移除 Python、Qt 和 Graphviz 的开发环境变量。
- 两次便携自检均通过，确认 Python 搜索路径、Qt 模块和 dot 均位于便携目录，完成经典例题、命名定义、构造回放、字符串模拟及窗口截图。
- 检查了 Qt 缩放因子 1 与 1.5 的截图。因子应用在本机系统缩放之上，不代表修改了 Windows 的显示设置。
- 构建中排除了 PATH 上其他软件的 ICU DLL 污染，统一了 VC 运行库。打包后实际启动成功。
- 保留仓库 README 原有 CRLF 修改；使用 `git -c core.whitespace=cr-at-eol diff --check` 检查新增改动。

本机详细报告和截图位于 `build/qa/portable-1.json`、`portable-1.5.json` 及同名 PNG；该目录为生成产物，不纳入 Git。

## 尚未验证

当前主机没有可用的 Windows Sandbox。本次没有在全新、未安装 Python / Graphviz 的 Windows VM 中验收，也没有单独在 Windows 10 1809 上运行。隔离 PATH 的本机验证不能替代上述验证。

可在干净 Windows VM 中解压便携包，运行 `Compiler-Lexer-Killer.exe --smoke-test`，确认 `smoke-result.json` 的 `ok` 为 `true`。

## 交付包

- 文件：`dist/Compiler-Lexer-Killer-1.0.0-win64.zip`
- 大小：46,878,845 字节
- SHA256：`e4b77d128662a2413f498930e1d3d94bb89bd5207e5bde8f52cbf2a86b608610`
- 包含 EXE、动态运行库、示例、使用说明、许可文件和 `source.zip`。
- 未发布 Release，未创建提交。
