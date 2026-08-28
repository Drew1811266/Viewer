# 持续评审阶段 D：Task 16 桌面桥接验证记录

> Status: Development evidence
>
> 2026-08-28：获批的素材预绑定、完整命令信封和取消链路已实现。
> 当前处于全仓验证及独立只读复核，Task 16 尚未验收，累计仍为 15 / 23。

## 范围

隔离工作树 `/Users/abc/Project/Viewer/.worktrees/continuous-review-domain`，分支
`codex/continuous-review-domain`，实现起点 `afb50d9`。主代理实施，结束前独立只读复核。
不启用新 UI、不迁移真实旧项目、不读取或改动下载/测试图、不改原媒体。
所有创建评审元数据的测试使用自有 TempDir，静态夹具仅复制后使用。
没有新增依赖、网络或文件系统权限，没有修改架构趋势基线。

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
不把它们当作最终验收结果。全仓门禁、最终提交号和独立复核结论待下节记录。

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
独立只读复核：待执行。首次保存失败后的重开恢复、响应资源边界继续补查，不提前验收。

Task 17–23 未开始；本记录不声称新 UI 已可使用，也不替代后续真实交互验收。
签名、公证、正式安装包、上架、公开发布和发售不属于当前开发任务或验收条件。
