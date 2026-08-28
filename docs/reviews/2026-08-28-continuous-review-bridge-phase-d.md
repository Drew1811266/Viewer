# 持续评审阶段 D：Task 16 桌面桥接验证记录

> Status: Development evidence
>
> 2026-08-28：获批的素材预绑定、完整命令信封和取消链路已实现。
> Task 16 实施、审阅修正后的全仓门禁及独立只读复验通过；阶段 D 完成，累计 16 / 23。

## 范围

隔离工作树 `/Users/abc/Project/Viewer/.worktrees/continuous-review-domain`，分支
`codex/continuous-review-domain`，实现起点 `afb50d9`。主代理实施，结束前独立只读复核。
不启用新 UI、不迁移真实旧项目、不读取或改动下载/测试图、不改原媒体。
所有创建评审元数据的测试使用自有 TempDir，静态夹具仅复制后使用。
没有引入新依赖包/版本、网络或文件系统权限，没有修改架构趋势基线。Desktop 将已锁定并在
依赖白名单中的 serde_json 从测试依赖提升为运行依赖，以流式度量 DTO 响应；通知清单已更新。

## 桥接对应关系

全部请求携带 `sessionId` 和 `generation`；写入必须有可写的当前会话。

| Desktop command | TypeScript port | 责任 |
| --- | --- | --- |
| `get_review_workspace` | `getWorkspace` | 当前状态、适用性、能力及历史摘要 |
| `prepare_review_assets` | `prepareAssets` | 从索引实体预绑定素材版本并返回一致预览 |
| `prepare_review_command` | `prepareCommand` | 生成完整可重试信封，不提交 |
| `apply_review_command` | `applyCommand` | 核验上下文、摘要及期望版本后提交 |
| `preview_review_archive` | `previewArchive` | 只读存档影响预览 |
| `preview_review_restore` | `previewRestore` | 只读恢复冲突及内容预览 |
| `get_review_history` | `getHistory` | 精确 selector 的只读历史 |
| `inspect_review_usage` | `inspectUsage` | 只接受当前索引实体，不接受任意路径 |
| `inspect_review_migration` | `inspectMigration` | 显式迁移之前的只读检查 |
| `get_review_evidence` | `getEvidence` | 已提交 selector / asset / role 的授权 PNG |
| `cancel_review_workspace_task` | `cancelTask` | 取消已登记任务，不影响后续调用 |

## 边界与资源策略

- 新 TypeScript port 独立于旧 ViewerBridge，旧组件和 mock 不必提前切换。
- DTO 拆分为命令、选择器、视图、历史、迁移和边界校验模块；serde 只在 Desktop。
- 完整信封包含 context、generated、usageSelections；前端回传不构成信任。重启后的相同
  信封只返回原提交回执，不生成重复状态。`CommittedViewUnavailable` 不丢回执。
- 后端按一次核验过的索引观察选择唯一人工 Stream；查询不会迁移项目或创建 v3 索引。
- 会话关闭先撤销图片授权和取消任务，等待已开始的原生渲染退出后再清理资源；调用方
  放弃等待也不会丢失后台任务的所有权。返回值和 token 登记均复核 session/generation。
- 证据 token 保存核验后的不可变字节，路径被替换不会换成另一张图。单会话缓存上限
  128 项 / 64 MiB，预绑定单次上限 128 个实体 / 64 MiB PNG；同一批返回值不会互相淘汰。
  超量请求在捕获前明确失败；淘汰的是临时链接，不是历史证据。
  后续 UI 必须将链接视为可重新获取的临时资源。
- 输入数组、文本、路径及数值有闭合校验和逐字段资源限制，具体见 ADR 0006。
  逐字段限制不等同于整个 IPC 消息的内存分配上限。

## 定向验证

已覆盖：会话关闭/跨会话 token、替换证据路径、缓存复用和数量/字节限额；新图预绑定与
源换版拒绝；完整信封往返、摘要/上下文篡改、未知/缺失字段、超量目标/画笔点；
保存后 view 取消但回执保留；只读工程；当前空不回退历史；普通取消后仍可继续；
渲染中关闭工程且清理顺序正确；旧桥接兼容与新 port 的 11 项命令映射。

定向日志在 `target/continuous-review-task16-*.log`。早期 RED 和静态检查失败日志保留，
不把它们当作最终验收结果。全仓门禁、最终提交号和独立复核结论见下节。

## 验收状态

首次 `pnpm verify:clean`：exit 101。协议及评审测试、UI 133 文件 / 1144 项通过（既有 1 跳过），
但既有视频 `timeout_kills_and_awaits_the_ffprobe_child` 没等到 PID 文件，门禁不算通过。
单独测试通过；整组重跑曾再次失败；串行整组通过，加入临时诊断后的四次整组均通过，
均返回 `TimedOut` 且 PID 存在。测试将进程启动和运行共用 1 秒预算，不能保证一定先写出 PID；
失败时具体触发条件未捕获，不声称已修复或已证明仅由系统负载造成。诊断修改已撤销，
视频源码与起点完全一致。失败和诊断日志均保留在 `target/continuous-review-task16-video-*.log`。

门禁后的边界补查补上预绑定 128 个实体的单次上限（129 明确拒绝、128 可用，RED→GREEN），
与缓存数量上限共用常量，避免同一批返回的早期 token 已被后续 token 淘汰。

固定工作树全仓门禁重跑：`pnpm verify:clean` exit 0，日志
`target/continuous-review-task16-verify-clean-stable.log`。包含协议 54 项、UI 133 文件 / 1144 项
通过（既有 1 跳过）、workspace Rust、Clippy、格式、架构边界、安全和依赖政策。
先前的视频超时测试本次通过，视频源码未改动；保留初次失败事实。
独立只读复核固定范围 `afb50d9..db456ef`：未发现 Critical，3 项 Important、1 项 Minor。
不将全仓门禁通过等同于交叉边界无缺陷；以下修正需再次门禁和复验：

1. 首次保存失败后仅有 recovery、没有 index，随机人工 Stream 会使重开过滤掉旧输入。
   真实桌面 SourceChanged→重开测试先 RED，改为固定 project ID 命名空间派生后 GREEN；
   既有人工 Stream 优先，生产 Stream 不被选中，无元数据写入。派生规则见 ADR 0006。
2. 排队中的已提交信封重试被取消，会被 Desktop 提前截断而丢回执。真实存储提交→持有 gate→
   重试排队→cancel 测试 RED 后，为 apply 保留 Application 既有提交查询路径，GREEN。
   其余查询仍提前取消；未提交的取消不会生成成功状态。
3. RestorePlan 的共享 Arc 在 JSON 中会重复展开。以 65,536 字节正文 / 10,000 目标的共享
   Domain 计划验证 64 MiB 流式输出预算；不分配约 625 MiB 展开 JSON，不返回部分成功，
   已知提交时仍返回 CommittedViewUnavailable + 原 receipt。11 个命令统一经过该边界。
4. 旧视频位置可能超过 JavaScript 精确整数。负例确认旧编码会输出 9007199254740993；
   新编码明确拒绝该位置/区间，精确边界仍可往返，避免静默舍入。

专项日志为 `target/continuous-review-task16-first-recovery-{red,green}.log`、
`target/continuous-review-task16-queued-retry-{red,green}.log`、
`target/continuous-review-task16-response-budget-{red,green}.log` 与
`target/continuous-review-task16-response-and-anchor-green.log`。response-budget-green 日志
同时记录了 Anchor 负例的失败，是其 RED 证据，不是整份测试的最终通过证明。

修正提交 `384d4cce28b9ed0d1e0d6f1f64d6805b0d2c095f` 的固定工作树
`pnpm verify:clean` 完整通过，exit 0，日志
`target/continuous-review-task16-verify-clean-review-fixes.log`。独立审阅者只读复验
`db456ef..384d4cc`，确认原三项 Important 和一项 Minor 均解决，无新增 Critical/Important；
复验没有并行编辑、编译或测试，不把主代理测试结果冒称独立动态测试。

| 最终验证 | 结果 |
| --- | --- |
| 桌面定向测试（原生关闭、排队重试、命令、DTO、响应预算） | 8 + 4 + 7 + 2 = 21 通过 |
| Application 持续评审服务（全仓门禁内） | 32 通过 |
| `pnpm verify:clean` | exit 0；协议 54、UI 133 文件 / 1144 通过 / 既有 1 跳过，workspace Rust、Clippy、构建、边界及依赖政策通过 |
| `pnpm architecture:trends` | exit 0，48 项非阻断告警，未修改趋势基线 |
| 原工作树检查 | 干净，仍为 `d7abb9961f377ae257c3283ef7893218ec0c53f9` |

实现检查点为 `db456ef`，审阅修正检查点为 `384d4cc`。未合并、推送或清理隔离工作树。
阶段 D 验收关闭；下一步是 Task 17 的 UI 持续评审协调器和输入保留。

Task 17–23 未开始；本记录不声称新 UI 已可使用，也不替代后续真实交互验收。
签名、公证、正式安装包、上架、公开发布和发售不属于当前开发任务或验收条件。
