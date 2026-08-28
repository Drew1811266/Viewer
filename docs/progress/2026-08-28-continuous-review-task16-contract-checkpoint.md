# 持续评审 Task 16：桌面桥接契约核对检查点

> Status: Development evidence
>
> 日期：2026-08-28。用户要求从 Task 16 继续；已完成接口核对和最近基线测试。
> 累计仍为 15 / 23；Task 16 未验收。下述契约补充已获用户“同意”，进入隔离实施。
> 桥接实现及新增定向测试已落地，当前全仓验证和独立复核状态见[桥接记录](../reviews/2026-08-28-continuous-review-bridge-phase-d.md)。

## 范围和现状

工作树 `/Users/abc/Project/Viewer/.worktrees/continuous-review-domain`，分支
`codex/continuous-review-domain`，起点 `edf74a4`。Task 14–15 已验收，见
[读取器记录](../reviews/2026-08-28-continuous-review-reader-phase-d.md)。

本检查点初次记录时仅核对计划与现有实现、运行基线测试；没有修改运行代码、创建新桌面命令、
切换 UI、迁移工程、操作下载/测试图或原始媒体。没有派发实现子代理。

## 阻止直接照抄桥接示例的两个契约缺口

### 1. 缺少素材预绑定入口

Application 的 `ContinuousReviewService::prepare_assets(entities, cancellation)` 在预览前
捕获素材版本，并将已准备素材保存在该 service 中。`SaveFeedback` 引用的是 `AssetVersionId`，
不是可重新解析的文件路径或任意前端构造的素材版本。未经此步骤保存会返回 `PreviewRequired`。

证据：`crates/viewer-application/src/review_workspace/service.rs` 的 `prepare_assets`，以及
`editing.rs` 的 `add_asset`。现有 Task 16 的九个命令和 TypeScript port 列表没有这个入口；
Task 18 却要求在新图上直接新增意见。

不能在保存时临时重读路径补救，否则预览和保存之间换图会导致意见绑定到用户没有审查的版本。

建议增加独立的 `prepare_review_assets` / `prepareAssets`：请求包含 sessionId、generation、
entityIds；后端仅从当前会话索引解析实体，返回已绑定版本及对应的受会话授权预览信息。
不接受绝对路径或前端声明的可信 `AssetVersion`，也不将预绑定混入纯查询或保存命令。
实现时必须保证返回预览与评审版本一致，不能只返回 ID 后继续显示未绑定的另一份实时图片。

### 2. 四字段重试 DTO 不能表达现有完整信封

Task 16 的 `PreparedReviewCommand` 示例仅有 commandId、expectedSnapshotId、payloadDigest、
command。但阶段 C 的实际 `ReviewCommandEnvelope` 还包括：

- context：项目、Stream 和 Production Scope；
- generated：生成的快照、意见、目标及修订身份、时间戳和迁移身份；
- usageSelections：准备时已固定的声明来源及摘要。

证据：`crates/viewer-application/src/review_workspace/model.rs` 的 `ReviewCommandEnvelope`、
`GeneratedReviewIds`、`PreparedUsageSelection`，以及
`crates/viewer-infrastructure/src/review/continuous/command.rs` 的完整摘要计算。

这不是可忽略的服务端临时信息：apply 会重新计算整个信封的摘要；只传四个字段再重新 prepare
会生成其他身份，无法保留已批准的同命令幂等重试语义。仅返回内存 token 也不能支持重启后的重试。

建议将桌面 DTO 补齐为闭合的完整类型，原样保留 context、generated、usageSelections；摘要仍为
64 位小写 hex。所有字段在 apply 重新验证，项目和会话的授权来源仍是后端，不因前端回传而可信。
不直接把持久化协议 JSON 暴露给组件，也不添加第二套业务算法。

## 必须一并落实的已有生命周期要求

这些要求已在 Task 16 范围内，不是新的产品功能：

- 补齐与未来 coordinator.cancel 对应的 `cancel_review_workspace_task` / `cancelTask`。
  普通取消只作用于本次会话任务；关闭会话先作废授权、取消任务，再等待任务退出后清理资源。
- 将 cancellation 贯穿预绑定、核验、渲染、保存及保存后的 view 刷新；检查 await 返回时的
  sessionId/generation，不能向新会话发回旧结果。现有 `view()` 内部使用默认 cancellation，
  桌面接入前需要显式补充可传递取消的调用路径。
- `CommittedViewUnavailable(receipt)` 必须保留回执，以区分“写入失败”和“已提交但刷新失败”；
  不能只压缩成一个通用错误后丢掉 receipt。
- 新 service 按后端核验的人工 Stream 固定上下文，不能信任前端指定任意写入范围。
- 历史证据授权仍只接受已提交 selector、assetVersionId、Base/Annotated 角色；已核验字节
  走既有图片 token 通道，关闭后失效，不添加任意路径读取能力。

## 建议的实现顺序和验收

上述契约补充已获确认；先更新 Task 16 接口清单及 ADR，再继续 TDD：

1. 真实临时仓储加载证据，登记图片 token，验证关闭、跨会话和替换路径均不能绕过授权。
2. 闭合 DTO 拒绝未知/缺失/非法字段；完整信封往返不丢字段，篡改摘要或上下文被拒绝。
3. 新图预绑定后保存成功；预览后源换版不能悄悄重绑，未预绑定保存必须失败。
4. 完整信封在新 service 实例重试不生成第二次提交；已提交刷新失败的响应保留原回执。
5. 取消、关闭中渲染、过期 generation、切项目后旧响应和只读工程的集成回归。
6. Rust 命令与 TypeScript port 逐一对应，运行全仓门禁和只读独立复核。

Task 17–23 顺序不变；不提前启用新 UI 或对真实旧工程作迁移。

## 本次已执行验证

| 验证 | 结果 |
| --- | --- |
| `cargo test --locked --offline -p viewer-desktop image_protocol` | 现有图片协议单元测试 10 通过；筛选命中的集成边界测试 1 通过，exit 0 |
| `cargo test --locked --offline -p viewer-desktop --test review_commands` | 7 通过，exit 0 |
| `pnpm architecture:trends` | exit 0，48 项非阻断告警；没有修改趋势基线 |
| `pnpm test:policy` | 33 通过；48 条需求映射检查通过，exit 0 |
| `git diff --check` | exit 0 |

日志位于隔离工作树 `target/continuous-review-task16-baseline-image-protocol.log`、
`target/continuous-review-task16-baseline-review-commands.log`、
`target/continuous-review-task16-trends-baseline.log`。
这些是已有边界基线，不是 Task 16 新能力通过的证明。本次没有重跑全仓门禁或原生 UI 验收。

原工作树 `/Users/abc/Project/Viewer` 仍为 `d7abb9961f377ae257c3283ef7893218ec0c53f9`，
状态干净；未合并、推送或清理现有隔离工作树。

签名、公证、正式安装包、上架、公开发布和发售继续不属于当前开发任务或验收条件。
