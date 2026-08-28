# 持续评审 Task 17：协调器与输入保留

> Status: Development evidence
>
> 2026-08-28：Task 17 首次完整门禁通过；独立复核指出三项 Important，已补回归并修正，
> 修正后的完整门禁与独立复验待完成。
> 阶段 A–D 仍为已验收基线，累计验收 16 / 23；本记录不表示新 UI 已启用。

## 范围与结构

仅在 `/Users/abc/Project/Viewer/.worktrees/continuous-review-domain`、
`codex/continuous-review-domain` 实施，基线 `b61cf93`。主代理实施，验收前独立只读复核。
不改旧工作台、不启用新入口、不迁移真实工程、不访问下载/测试图、不改原媒体。
未新增依赖、IPC、存储格式或权限；不修改版本号与架构趋势基线。

协调器使用 Task 16 的独立 `ReviewWorkspacePort`，不直接调用 Tauri 或写协议文件。
`workspace/ports.ts` 在 Task 16 已提供类型导出，本次无需把半成品流程加入旧 WorkspacePorts。

- `continuousReviewModel.ts`：闭合状态、编辑输入、错误归一化与值比较。
- `continuousReviewSession.ts`：一个会话的请求生命周期、唯一在途写入、完整重试信封与输入保护。
- `continuousReviewActions.ts`：具名用例与存档/恢复/迁移预览的本地确认依据。
- `useContinuousReviewCoordinator.ts`：React 订阅及会话生命周期适配，不堆叠业务/几何规则。
- 测试按输入、生命周期、预览、其他用例、守卫拆分；共享夹具仅供测试使用。

这是计划内实现拆分，不是新增产品流程。补充暴露 Task 16 已有的 `prepareAssets`、
`confirmApplicability`、`continueLegacy`，供后续工作台和历史面板使用，不新增桌面命令。

## 行为契约

1. 编辑输入携带创建时的 `baseSnapshotId`，与当前已提交 view 分离；刷新不自动改变输入依据。
   `beginEditor` 在有未提交输入时拒绝换图/换编辑对象；`discardEditor` 是显式放弃，
   `rebaseEditor` 是用户核对新版本后的显式确认，两者均不能跳过不确定提交的核对。
2. 同一时间只有一个语义写操作；相同操作双击共享结果，竞争操作返回 busy，不排队自动发布
   后来的新文字。准备前复制载荷；prepare 失败重试复用同一 request/commandId；apply 失败
   重试复用完整、不可变的 envelope，包括 generated/context/usageSelections。
3. 已知未提交失败后，用户明确改变载荷或版本依据才产生新操作。不确定提交、完整性/查找
   不可判定及已提交但刷新失败时保留原操作，不能用新 ID 绕过核对。
4. `inputRevision` 仅是 UI 输入计数。保存期间继续输入不会被成功回执清空；首次新意见保存
   后，新文字继续关联已保存 Feedback ID，下一次是编辑而不是新增重复意见。保存期间不改
   几何；失败后明确改了几何则要求保存新载荷，不能重试旧载荷并清掉新标记。
5. 当前 view 只取 apply 的重读 view，不把可能属于历史的 receipt.snapshot 当作当前 head。
   `lastReceipt` 单独保留；CommittedViewUnavailable 明确为 recovery_required，不假称未提交。
   未识别或缺少必要字段的错误按未知结果保留输入，不作成功处理。
6. 每个请求固定 sessionId/generation。切工程/关闭/StrictMode 清理后，旧响应不能更新新状态；
   已知回执仍随过期操作的 rejection 返回给原调用者。新加载等待旧取消确认，避免相互干扰。
   取消不回滚；在途取消确认不会覆盖已完成的成功结果，也不允许新写入抢在取消确认前执行。
   刷新在途时重试返回 busy 并保留原信封；不能让迟到刷新覆盖重试产生的新 view。
7. 存档/恢复只提交本协调器生成并仍有效的预览范围；新读取、写入或取消使旧预览失效。
   乱序预览只保留最后请求；StaleSnapshot 必须刷新并重新确认，不能自动换依据重发。
   未保存输入不能被附带存档，恢复冲突未确认不能提交。
   本地校验失败也会使前一个预览请求失效；接受结果前再次核对请求身份与 viewVersion，
   不能在通知订阅者后重新激活已被替代的范围。
8. 历史、证据、预绑定与声明检查是独立只读结果，不替换当前快照、不暗中采用声明。
   无效声明不阻止无声明手动存档。迁移必须显式确认匹配的 inspectionDigest；只读能力禁止写入。
   当前空状态不回退历史、不生成“全部通过”。
9. 新提交、迁移和直接重试共用无关恢复记录守卫；原命令自己的恢复记录仍允许原信封核对。
   不因迁移能力为 true 绕过未决记录；已提交迁移刷新后不再需要迁移，也仍可核对原回执。

## 验证证据

恢复工作前的既有协调器/工作台/桥接基线：3 文件 / 14 项通过。
RED→GREEN 日志保留在 `target/continuous-review-task17-*.log`：

- `initial-{red,green}`：无提交实现时真实断言失败；完整信封重试与重读 head。
- `input-{red,behavior-red,green}`：输入保留、双击、后续文字身份、prepare/apply 失败与未知错误。
- `lifecycle-{red,green}`：跨会话/关闭、StrictMode、取消与只读；RED 中含尚未定义的新方法。
- `preview-{red,behavior-red,green}`：固定选择、过期预览、乱序响应与恢复冲突。
- `operations-red`、`review-suite`：其他用例与只读能力、可选声明、迁移及恢复状态。
- `guards-{red,green}`：原输入版本、失败后改几何、刷新乱序、取消加载与错误 DTO 形状。
- `cancel-order-{red,green}`：取消确认在途/迟到及无关恢复记录隔离；GREEN 包含评审目录
  11 个测试文件 / 80 项通过，其中本次新增 55 项，其余为既有回归。
- `handover-red`：去掉跨协调器取消屏障的负向控制失败；恢复后纳入最终定向验证。

第一次类型检查发现 TS1294（仓库禁止构造参数属性）和测试数组缺失值检查，已改为显式字段
及存在性检查，不放宽 tsconfig。之后类型检查通过。Biome 仅报告仓库已有 recommended
配置弃用提示，没有修改无关工具配置。

首轮定向验证 `focused-final`：11 文件 / 81 项通过（新增 56）；`ui-check-final`、
`boundaries`、`policy` 均 exit 0。`trends` exit 0 / 49 项非阻断告警，较 Task 16 增加
`continuousReviewActions` 的分支复杂度 18（阈值 15）：当前集中拥有三个预览确认依据，
未继续扩张旧协调器；下轮若在此增加面板状态分支，应按预览职责拆分。未修改趋势基线。
首轮完整 `verify-clean` exit 0，UI 139 文件 / 1200 通过 + 1 个既有跳过；Rust 全仓、
格式、Clippy、协议、构建、安全/许可和架构边界门禁通过。此内容提交为 `da1dcff`，
独立复核范围 `b61cf93..da1dcff`，并未据此提前标记任务验收。

### 独立复核修正

独立只读复核通过真实 session 的内存执行复现三项 Important；未发现 Critical：

1. 失败后刷新与直接重试并发，迟到刷新可回退新 head。`review-refresh-{red,green}`：
   先复现重试错误通过，再增加 loading 守卫；刷新结束后仍可使用原信封重试。
2. 直接重试和迁移绕过无关恢复记录。`review-recovery-{red,green}`：两个入口均先复现
   错误通过，再共用恢复守卫；同时验证同命令恢复及已知迁移回执核对不会被误锁。
3. 较新的预览在本地被拒绝后，旧异步结果仍能成为提交依据。
   `review-preview-{red,green}` 覆盖 stale_snapshot 与 archive/restore 的脏输入拒绝；
   将本地验证纳入已递增请求序列的 query，避免旧响应覆盖最新错误。
   `review-preview-notification-{red,green}` 进一步复现订阅通知期间发起新请求的情况；
   archive/restore 保存尚无 plan 的请求对象，并在写入 plan 前核对身份和 viewVersion。

修正后定向 `review-focused-final`：11 文件 / 90 项通过（新增 65），
`review-ui-check-final` exit 0；`review-trends-final` exit 0 / 49 项非阻断告警，
`continuousReviewActions` 的分支复杂度为 24，未修改基线。
修正后完整门禁与独立复验结果在完成后追加。

## 后续边界

Task 18–21 才接入真实工作台、存档/历史/换版及迁移面板；Task 22–23 才做完整原生交互验收
和 Current 产品文档同步。本次通过 renderHook 和真实 session 订阅驱动协调器，桌面端口由完整的 typed
测试替身替代，不声称浏览器截图、原生点击或真实工程已经验收。

持久恢复草稿仍是未提交数据，当前只展示并阻止自动重放，不增加清除/覆盖/自动消解恢复记录
的后端能力。后续恢复面板必须核对已有端口可表达的显式处置，不能靠 UI 隐藏恢复记录来解锁，
也不能把新 ID 当作旧操作的重试。底层留存限制继续适用。

签名、公证、正式安装包、上架、公开发布和发售不属于当前任务或验收条件。
