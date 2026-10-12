# 2.0 验收记录

日期：2026-09-30。构建环境：Windows x64、Python 3.11.8、PySide6 6.10.1、PyInstaller 6.16.0、Graphviz 12.2.1。

## 源码验收

- 自动测试：119 passed。保留首版 81 项回归，新增算法、词法规则、手绘、判题、持久化、CLI 和 GUI 场景。
- 经典表达式 `(a|b)*abb`：标准 NFA 14 状态、DFA 5 状态，简化 NFA 5 状态，有效字母表 `{a,b}` 上最小 DFA 4 状态。状态数不要求与课件一致。
- 100 个固定种子生成的受限表达式通过标准／简化构造的完整语言等价检查；指定表达式继续枚举 `{a,b}` 长度不超过 6 的全部字符串，并检查人工已知语言性质。
- 三组词法示例在标准、简化、最小化自动机上得到相同 Token、类别、区间、跳过内容和错误。验证 EOF 回退、同长优先级、规则遮蔽、CRLF 行列、非法 Unicode 字符及可空规则拒绝。
- 最小状态数使用人工确定的典型语言校验；等价判定测试空串／最短反例、类别差异和资源不足，不将未完成判定算作错误答案。
- GUI 测试执行实际鼠标双击创建、点击连线和自环、拖动、撤销重做、删除；并测试异步结果失效、手绘模拟、工程往返和导出。
- 原生 Qt 自检已通过七个页面、综合示例 14 个 Token、手绘 NFA 转换、判题、工程恢复及 SVG/PNG、CSV/JSON、Markdown 导出。详细报告及截图在 `build/qa/source-v2-final.json` 和同名 `-artifacts` 目录。
- `pip check`：No broken requirements found。

## 便携验收

- 便携 EXE 从含中文及空格的新目录启动；PATH 仅保留 Windows System32，移除 Python、Qt、QML、Graphviz 的开发环境变量。
- Qt 缩放因子 1 与 1.5 均完成完整原生自检。报告确认 Python 搜索路径、Qt 模块和 dot 都位于解压后的便携目录。
- 完成七个页面、优化、综合词法扫描、鼠标连线和自环、撤销重做、手绘子集构造、模拟、练习判题、工程保存恢复，以及 SVG/PNG、CSV/JSON、Markdown 导出。
- 每轮报告为 `build/qa/portable-1.json` 和 `portable-1.5.json`；页面截图、工程与导出样例位于同名 `-artifacts` 目录。

## 验证边界

按本次确认，以当前电脑隔离开发依赖后的验收为准。没有进行干净 Windows VM 或 Windows 10 1809 实测。Qt 缩放因子叠加于系统缩放，不等同于 Windows 显示设置的绝对百分比。

交付为本地 `dist/Compiler-Lexer-Killer-2.0.0-win64.zip` 和同名 SHA256 文件，包含 EXE、动态运行库、示例工程、许可、文档及源码。未自动发布 GitHub Release。