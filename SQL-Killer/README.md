# SQL-Killer 1.2.0

离线数据库学习实验室。Tauri 2 + Rust + Vue 3 + TypeScript + Monaco；不使用 Electron。内置 SQLite，无需安装 Python、Node 或数据库服务即可学习。MySQL 由使用者另行提供。

## 1.2 自学入口

- **课程学习**：六段入门引导、33 道课程题的专属三级提示、八个原生 SQLite 求值的预测检查点。概念题可直接选择作答，独立 / 辅助通过分别记录；MySQL 专属题不计入离线主线分母。
- **自由实验**：SQL、关系代数和事务工作台。结构树按光标作用域插入别名引用；表支持前 100 行预览。查询回放显示当前操作及全部直接输入，完整 DAG 可展开导出；SQL 对照在同一 SQLite 快照上比较两个查询。
- **案件挑战**：《选课系统里的第 41 个人》包含容量调查、请求关联、日志分析、并发复现和修复验证。固定证据后，结案重新执行 SQL 检查，接受不同正确解法。

引导与自由方式可随时切换，章节没有硬性解锁。学习记录和案件报告可以本地导出。案件、练习使用独立数据，不替换自由实验数据库。

案件离线修复是两个请求的有限模型枚举；真实 MySQL 验证在新建临时库中执行五类场景并清理，需连接用户具备 CREATE / DROP DATABASE 权限。两种通过标记分别记录。可运行 SQL 模板见 [修复模板](curriculum/case-repair.mysql.sql)，自定义脚本在自由实验中使用，不自动授予内置策略的通过标记。

工程格式为版本 2，仍由 manifest.json 与 database.sqlite 两个归档条目组成。版本 1 在内存中迁移，首次保存提示另存，不覆盖原文件；新版工程需要 1.2 应用打开。证据板最多 20 卡、每卡 200 行 / 256 KiB、总计 4 MiB，节选结果不替代结案时的完整核验。

1.2 的验证步骤与真人试用记录见 [自学验收](curriculum/acceptance/SELF-STUDY-1.2.md)。真人试用和物理 Windows DPI 验收尚需单独完成。

## 启动

便携 ZIP 解压后运行 `SQL-Killer.exe`，保留同目录的 `WebView2` 和 `portable.flag`。数据写入 `data`。安装版使用当前用户应用数据目录。请放在可写位置，不要直接在压缩包内运行。支持目标为 Windows 10 22H2 / Windows 11 x64；实际环境和未验证项目见 [验收记录](VALIDATION.md)。

## 学习流程

1. 首次启动进入课程学习首页；可以从零开始或直接进入自由实验。SQL 工作台载入 HNU Student。`Ctrl+Enter` 执行选区或当前语句，`Ctrl+Shift+Enter` 执行脚本；`Ctrl+Space` 补全，右键提供格式化和查找。错误区区分分析器、模式检查和课程建议。
2. 点击“生成查询过程”，逐步查看输入、连接、筛选、分组、投影、去重、排序及分页。过程与同一快照的原查询核对；它是逻辑解释，EXPLAIN 才是数据库原生计划。
3. 关系代数页输入函数式表达式，执行并查看符合前提的下推规则。独立代数采用集合语义，SQL 保留多重集语义。
4. 事务页编辑调度，比较原始调度与四种课本锁模型，查看版本、变量、锁、等待、异常及优先图。视图判定比较读取来源和最终写入者。
5. 课程练习提供查询、修改、结构／约束、触发器、代数、概念、调度及 MySQL 例程题。答案在独立数据上验证，保留重复行与 NULL；“通过”只代表通过这些测试数据。
6. 保存 `.sqlkproj` 保留编辑器、SQLite 数据、答案、错题与实验配置；30 秒自动恢复副本。保存前结束 SQLite 活动事务，避免将未提交实验误认为已持久化事务。

内置 Student、Hotel、Bank、Empty Sandbox；切换下拉选项后点击“重置”加载。重置会明确确认。导出结果 CSV／JSON、图形 SVG／PNG、Markdown 报告均在本地完成。CSV 的 NULL 显示为 `NULL`，无损交换请用带类型 JSON。

## 1.1 动画教学

查询、代数、事务和练习公开试运行共用暂停、逐事件前后移动、速度与跳转。点击图节点跳到该操作；点击输入／输出行暂停并查看关联。专注演示扩大画布和表格，减少动态效果保留高亮而关闭连续位移。

默认每个操作最多 20 个重点事件，其余连续区段合批；切换“完整逐行”可查看全部事件。完整结果未被动画抽样截断。事件由冻结快照生成，播放不会重复执行 SQL。蓝＝当前、绿＝保留、琥珀＝筛除、紫＝NULL／UNKNOWN、红＝冲突，同时提供文字说明。

详见 [动画说明与实测边界](curriculum/ANIMATION.md) 和 [真实演示视频](curriculum/demos/README.md)。

## MySQL

连接设置支持主机、端口、账号、数据库、TLS。密码仅内存保存，也可选择 Windows 凭据存储；工程不包含密码。连接会建立 main、A、B 三个独立会话。点击“创建并切换”新建随机名称的专用沙箱，重置仅允许工具记录的自有沙箱。

真实事务实验建议选择 Bank。分别执行 BEGIN、普通 SELECT、SELECT … FOR UPDATE、UPDATE、COMMIT／ROLLBACK，观察真实等待、超时与死锁。MySQL 的快照读与教学锁模型分开解释。过程分析使用独立只读事务，不包含 main/A/B 的未提交写入。取消不会被描述为自动回滚全部写入；结果不确定时必须核对服务器状态并重连。

例程判题需要创建临时数据库和低权限临时用户的权限。安装例程时显式设置沙箱用户为 DEFINER，调用以沙箱权限执行，最后清理临时用户和数据库。不会修改服务器全局配置。没有所需权限时返回未完成。课堂原材料混用方言，SQLite 不支持存储过程／函数，相关练习明确使用 MySQL。

## 关系代数

```text
select{Sage >= 18}(Student)
project{Sno,Sname}(Student)
rename{S;Sno:id}(Student)
join(Student,SC)
join{S.Sno=C.Sno}(rename{S}(Student),rename{C}(SC))
union(project{Sno}(Student),project{Sno}(SC))
diff(project{Sno}(Student),project{Sno}(SC))
divide(project{Sno,Cno}(SC),project{Cno}(Course))
group{Sdept;COUNT(*) AS n,AVG(Sage) AS avg_age}(Student)
```

同时支持 intersect、product、leftjoin、rightjoin、fulljoin。集合运算按列位置检查列数和已有值的属性域；SQLite 动态类型下空列没有可观察的类型。NULL 谓词为三值逻辑，集合去重将 NULL 视为同值。除法至少保留一个结果属性。优化不对外连接擅自下推筛选。

## 调度语言

每行一个操作：`T1 read A`、`T1 set A A-100`、`T1 write A`、`T2 write B 200`、`T1 scan A 0 1000`、`T2 insert A2 50`、`T1 commit`、`T2 rollback`。局部算术支持括号与加减乘除，整数范围 ±(2^53−1)。scan 按数据项前缀及值范围查询。view 判定最多 8 个事务、256 个操作、5 秒；超限或模型不覆盖时显示未完成。

## 限制和结果语义

查询默认 10 秒，预览最多 50,000 行／32 MiB；过程最多 10,000 行／20,000 步。截断数据不能用作判题通过。过程不支持递归 CTE、窗口函数、用户例程或副作用函数，这些 SQL 仍可真实执行。未知分析器语法不被冒充为引擎必然拒绝。

课程来源与页码见 [课程说明](curriculum/README.md)，详细实测边界见 [VALIDATION.md](VALIDATION.md)。本工具不是生产数据库管理工具，也不声称证明一般 SQL 等价。

## 源码构建

已锁定依赖于 Cargo.lock、pnpm-lock.yaml 和 runtime.lock.json。安装 Rust 1.94.1、MSVC C++ 工具链、Windows SDK、Node 24.18.0、pnpm 10.33.0。Python 仅用于获取固定运行时和打包辅助，不是应用运行依赖。

```powershell
pnpm install --frozen-lockfile
python tools/fetch-runtime.py
pwsh tools/build.ps1
pwsh tools/package.ps1
```

开发：`pnpm tauri dev`。单元检查：`cargo test -p sql-killer-core`、`pnpm test`、`pnpm lint`、`pnpm typecheck`、`cargo clippy --workspace --all-targets -- -D warnings`。

独立 MySQL 验收：`pwsh tools/mysql-qa.ps1` 后运行 `cargo test -p sql-killer-core --test mysql_integration -- --ignored`，结束后 `pwsh tools/mysql-qa.ps1 -Action stop`。脚本默认使用本机 MySQL 9.5 二进制、独立数据目录及 13367 端口，不使用原服务。

真实窗口验收：先 `cargo install tauri-driver --locked --root build/tools`，构建应用后 `pnpm test:e2e`。可用 `SQL_KILLER_TEST_EXE` 指向便携版；截图和日志在 build/qa/e2e。构建不自动发布 Release。
