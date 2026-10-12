补充成品验收，2026-09-30

最终 NSIS：实际安装至中文空格目录，退出码 0。安装 EXE SHA256 与 VALIDATION.md 中最终 EXE 一致；检查了 WebView2、补充的 webview2-com MIT 许可与最终验收文档。随后在隔离环境下再跑完整 WebDriver 流程，通过。

导出：真实 UI 点击 SVG、PNG、Markdown 报告按钮；仅将原生保存对话框响应替换为专用测试目录路径，其余 IPC 与生成代码保持原样。检查 SVG 中文／Student 标签、PNG 文件头与实际图像、Markdown 输入 SQL 和中文结果表。全部通过。不将此项声称为原生文件选择对话框的键鼠验收。

原始结果见同目录 final-nsis-installed.json 和 final-exports.json。构建机截图、导出样例在 build/qa/final-exports；测试程序退出后未残留 SQL-Killer、tauri-driver 或 EdgeDriver 进程。
