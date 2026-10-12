# 湖南大学辅助脚本

这里是 **HNU-Script**，一个可以让你减轻工作量的脚本集合项目！

本项目持续更新，各种使用脚本让你脱离限制，直接起飞！

如果本项目帮助了你，麻烦你Star本仓库，十分感谢你的支持！

*注：本项目的各个脚本互相独立，具体用法见下文*

## 目前更新的项目

| 文件名                   | 说明                                                |
| ------------------------ | --------------------------------------------------- |
| Discrete_Mathematics.ahk | 离散数学粘贴工具（适用于罗纲/杨圣洪老师开发的软件） |
| code.ahk                 | 超星在线代码框粘贴工具                              |
| Karnaugh-map-killer.py   | 数电的一键卡诺图绘制工具，并可以生成答案            |
| Compiler-Lexer-Killer | 编译原理词法分析实验室：正则、AST、NFA、DFA 和字符串模拟 |
| [SQL-Killer](SQL-Killer/README.md) | Tauri + Rust 离线数据库学习实验室：SQL、逐行动画、关系代数、事务与练习 |

## 环境要求

**SQL-Killer 独立要求**：Windows 10 22H2／Windows 11 x64；便携版携带 SQLite 和固定 WebView2，无需 AutoHotkey、Python 或 Node。MySQL 功能需自行提供 MySQL 服务。详见其 [说明](SQL-Killer/README.md) 和 [验收记录](SQL-Killer/VALIDATION.md)。其他工具的要求保留如下。

要运行本项目，你需要满足以下条件

- Windows 7 或更高版本的操作系统
- [AutoHotkey v2.0](https://www.autohotkey.com/) 或更高版本 **本项目所有脚本为v2.0标准，不兼容v1.0**
- Python3.10 或更高版本

## 快速开始

如果你之前从来没有用过类似工具，你可以从Release页面下载编译好的.exe文件，双击后直接使用。具体详见每个脚本的介绍

如果你想要定制化你的脚本，请参考以下步骤

首先，将仓库克隆到本地

```bash
git clone https://github.com/Drtxdt/HNU-Script.git
```

然后，转到项目文件夹`HNU-Script`

你可以在记事本等软件中编辑脚本

双击即可运行脚本

Python脚本使用方法详见具体介绍

## 具体介绍

**所有AutoHotkey脚本的退出方法都是系统托盘右键->Exit**

### Discrete_Mathematics.ahk

这是应对离散数学课程软件不能将外部内容粘贴到软件内部的问题。双击运行后，在软件内部按下热键 `Ctrl+Win+V`即可粘贴

原理：使用`SendText()`方法

使用前，你必须确保剪贴板中有文字，且在源代码中填入**软件当前窗口名**。使用时，请保持登录状态（没登录不要登录，登录后不要退出，务必保证当前窗口名与脚本中一致）

### code.ahk

这是应对部分网页代码框无法粘贴（甚至开发者工具都不行，因为它们是把你输入的文本分割）的问题，适用于超星等平台。

双击运行后，在软件内部按下热键 `Ctrl+Alt+V`即可粘贴

原理：逐个发送字符，并防止网站的花括号自动补全导致错误

使用前，你必须确保剪贴板中有文字

### Karnaugh-map-killer.py

这是一键完成所有关于卡诺图的问题的脚本，支持2~4个变量且支持4变量卡诺图的绘制。支持无关项。

要使用它，首先应该转到文件夹：

```bash
cd Karnaugh-map-killer
```

然后安装依赖：

```bash
pip install -r requirements.txt
```

最后运行即可

```bash
python Karnaugh-map-killer.py
```


### Compiler-Lexer-Killer

编译原理词法分析可视化工具。2.0 支持命名定义、Thompson／课件简化 NFA、DFA 子集构造与最小化、最长匹配与规则优先级、自由画图、练习判题、工程保存和 SVG／报告导出，可单步、后退和自动播放。

本工具独立要求 **Windows 10（1809 起）/ Windows 11 x64**。便携包解压后运行 `Compiler-Lexer-Killer.exe`，无需安装 Python 或 Graphviz。

源码运行要求 Python 3.11+。输入语法、CLI、示例工程和构建方式见 [工具说明](Compiler-Lexer-Killer/README.md)，实际验证范围见 [验收记录](Compiler-Lexer-Killer/VALIDATION.md)。
