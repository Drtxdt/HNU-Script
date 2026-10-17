# 1.2 自学体验验收

本记录区分源码测试、真实数据库、桌面自动化和真人学习效果。1.0 / 1.1 的旧验收记录不能替代本版本验证。

## 本地执行结果（2026-10-17）

- Rust 核心测试：90 项通过；另外两个需要服务的 MySQL 集成测试单独执行并通过。
- 前端：18 项测试通过；ESLint、Vue / TypeScript 类型检查、Vite 生产构建通过；Rust workspace Clippy 通过。
- 真 MySQL：独立 13367 实例验证双会话；四种案件策略各覆盖五类场景，测试清理临时数据库。
- 自学桌面：`build/qa/self-study-final-help/result.json` 通过。完整调查、证据板、模型修复、结案、保存恢复、报告导出、浏览不标脏、提示使用跨页保留、别名插入及同快照对照通过。
- 发布版完整自学流程：`build/qa/self-study-release/result.json` 通过，与 debug 自学验收使用同样的五任务和持久化检查。
- 便携版隔离测试：`build/qa/portable-final-isolated/result.json` 通过。清空开发工具 PATH、指定不可用外部运行时并阻断代理网络后，实际运行路径来自包内 `WebView2`；查询、回放、导出、保存恢复与切换竞争通过。
- 九组视口 / 像素比例截图和深浅主题截图保存在同目录。该机器原生 Windows 像素比例为 150%；其余组合通过 WebView2 CDP 模拟。
- 原有工作台 / 动画 / 代数 / 事务 / 课程 / MySQL 会话 / 导出与切换竞争回归记录在 `build/qa/regression-1.2`；最终源码复验记录另存 `build/qa/regression-final-runtime`。

Vite 对包含 Monaco 与 ELK 的较大脚本块保留体积提示，未影响构建。当前受限文件系统的 Rust 增量缓存偶有硬链接降级为复制的环境提示。

## 自动化命令

```powershell
node tools/check-version.mjs
cargo test --locked -p sql-killer-core
cargo clippy --locked --workspace --all-targets -- -D warnings
pnpm test
pnpm lint
pnpm build
```

Windows 受限执行环境若不能在系统临时目录重命名测试缓存，可仅为当前命令将 TEMP / TMP 指向本项目已存在的 build 目录。不要更改系统级设置。

真实 MySQL 使用 tools/mysql-qa.ps1 的独立 13367 实例，不使用个人数据库服务：

```powershell
cargo test --locked -p sql-killer-core --test case_mysql --test mysql_integration -- --ignored
```

案件修复原方案 / 单独唯一约束应失败，行锁 / 原子条件更新应通过。五类场景是剩一席、已满、空课程、重复请求、插入失败回滚。验证后检查没有残留 sqlkiller_case_ 临时库。

构建带 custom-protocol 的桌面程序后，运行 `pnpm test:self-study` 和 `pnpm test:e2e`。脚本使用 build/qa 下的独立应用数据，截图与 result.json 同目录保存。self-study 测试覆盖三次调查查询、证据固定、复现、修复和结案、保存、报告、浏览不标脏、别名插入、同快照对照、主题，以及九组视口 / 缩放截图。

视口为 960×640、1366×768、1500×960，缩放为 1 / 1.25 / 1.5。CDP device-metrics 是 WebView2 视口与像素比例模拟，不等同于物理 Windows DPI 切换。真实系统 DPI 切换和安装版安装 / 卸载仍需在发布候选中分别复验。

## 人类学习效果：待招募，不能用自动化代替

至少五名未学过 SQL 的志愿者，在知情同意后进行本地试用；不采集姓名或账号，不自动上传录屏或日志。

1. 从首次启动开始计时，观察是否能在十分钟内独立完成第一次查询。
2. 完成 NULL / 外连接引导后，解释为什么 LEFT JOIN 保留没有选课的学生。
3. 完成案件后，用两条读取记录、覆盖写入和最终记录数解释 39 → 41。
4. 记录卡点、是否需要口头提示，以及查看提示前后的解释变化。
5. 根据共同卡点修改说明和提示，再做一次复测。

| 匿名编号 | 首次查询耗时 | 是否外部提示 | LEFT JOIN 解释 | 并发解释 | 卡点 |
| --- | --- | --- | --- | --- | --- |
| P1 | 待测 | 待测 | 待测 | 待测 | 待测 |
| P2 | 待测 | 待测 | 待测 | 待测 | 待测 |
| P3 | 待测 | 待测 | 待测 | 待测 | 待测 |
| P4 | 待测 | 待测 | 待测 | 待测 | 待测 |
| P5 | 待测 | 待测 | 待测 | 待测 | 待测 |

学习效果门槛：至少四人达到首次查询要求，并能在相应模块后独立解释外连接与并发故障。未取得真人记录前，不声明已经通过该门槛。
