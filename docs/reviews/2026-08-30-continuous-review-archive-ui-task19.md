# Task 19：精确手动存档 UI 实施与复验记录

> 日期：2026-08-30  
> 分支：`codex/continuous-review-domain`  
> 隔离工作区：`.worktrees/continuous-review-domain`  
> 范围：实施计划 Task 19；Task 20–23 未开始；连续评审生产入口仍未启用。

## 结果

手动存档现在具备一个内部、显式 opt-in 的精确范围预览面板。它只提交用户确认的
`ReviewArchiveSelection`，并保持以下边界：

- 有核验依据时可保留 `agent_declared` 或 `user_selected` 的已知 B；没有依据时使用明确的
  “交接版本未确认”，不根据时间、文件变化或 Agent 行为猜测版本。
- 预览和提交均绑定当前 C 的 `expectedSnapshotId`；C 变化、会话／generation 变化或旧异步
  结果返回时，旧面板不能写入替换后的上下文。
- “将移入历史”“保留后补意见”“当前已不存在”“未选素材”和“已有效覆盖”按精确
  Feedback／文字 revision／Target／目标 revision 分开展示；显示编号不参与身份比较。
- 只对 `retain_later_edit` 使用“保留后补意见”；`remove_current` 和 `already_absent` 不被
  错写成后补，更不显示“已修复”或“已通过”。
- 全部选中键已有有效覆盖时显示“没有新的可存档内容”并禁止提交；首次记录历史但保留后补
  或当前目标已不存在时仍可建立有效历史事实。
- 取消最后一个目标会清空旧结果，同时保留 preview 与取消前 selection 的精确候选并集；
  重新勾选必须重新取得后端预览后才能确认。
- 成功只提示意见已移入可撤销历史，不声称 Agent 已读、已执行或问题已解决。

生产 `App` 尚未传入 continuous coordinator 或存档 selection；legacy 入口及用户可见行为保持
不变。Task 21 才负责协议选择、可选声明／迁移入口和完整流程切换。

## 架构

Task 19 没有把存档／历史状态放入 `useImageReviewWorkbench`。连续存档的面板状态、固定选择、
会话 epoch 和异步失效集中在 `ReviewWorkspaceLayer` 内的专用 `useContinuousArchivePreview` 协调
单元；总层组件只做组合。独立审查后曾出现的 `ReviewWorkspaceLayer` 超过 200 行、决策复杂度
28 的新增趋势告警已通过职责拆分消除，未修改架构趋势基线或分类来隐藏问题。

组件仍只依赖 Task 16 的 typed DTO 和 Task 17 coordinator；没有导入桌面 bridge、读取
`.viewer`、增加第三方依赖、修改原素材或接触视频／搜索／文件操作实现。

## 审查与修正

首次独立审查发现四项 Important：已知依据没有集成入口、跨会话面板状态未清理、只有
`alreadyCovered` 的 no-op 可继续确认、retention disposition 被统一误标。`9a722dc` 修正后
复审通过。

完成前的架构趋势检查又发现总层函数膨胀；`a3305bd` 抽离专用协调职责。后续 scoped re-review
依次发现空选择仍显示旧结果、selection-only 候选会丢失；`e96831d` 和 `4537cba` 以精确候选
并集及重新预览约束修正。最终独立复审没有剩余 Critical／Important，也没有修正引入的新
Minor；首次审查的一项交互说明 Minor 按下述范围延后处理。

审查遗留一项非阻断 Minor：有未保存输入时 context-bar 存档按钮当前直接禁用，因而其点击
处理器中的保存／取消提示不可达。数据安全边界正确，连续入口尚未启用；在 Task 21 激活前需
结合最终入口的 disabled reason／放弃输入交互统一处理，不能以解除数据守卫代替说明。

## 验证证据

最终提交 `4537cba` 上重新执行：

- `pnpm --dir ui test src/components/review/ReviewArchiveDialog.test.tsx src/components/review/reviewComponents.test.tsx src/app/review/useContinuousReviewCoordinator.test.tsx`
  - 3 个文件，37 项通过。
- `pnpm --dir ui check`
  - exit 0；仅现有 Biome 配置弃用信息。
- `pnpm architecture:trends`
  - exit 0；47 项既有分支非阻断告警；无 `ReviewWorkspaceLayer` 长函数或决策复杂度新增告警；
    未更新基线。
- `pnpm verify:clean`
  - exit 0；UI 141 个文件、1251 项通过，1 项既有跳过；UI build、Rust workspace test、
    `cargo fmt --check`、Clippy、协议读取、评审闭环、架构／安全、依赖和许可证门禁均通过。
- `git diff --check`
  - exit 0。

## 提交

- `d869049` — `feat(review): preview exact manual archive scope`
- `9a722dc` — `fix(review): preserve exact archive preview safety`
- `a3305bd` — `refactor(review): isolate archive preview coordination`
- `e96831d` — `fix(review): clear empty archive preview`
- `4537cba` — `fix(review): preserve selection-only archive choices`

## 未进入本任务的范围

- 未启用生产连续评审入口，未迁移真实工程，未操作用户原素材。
- 未开始历史查看／恢复／素材适用性确认 UI（Task 20）。
- 未开始声明／迁移入口和流程切换（Task 21）。
- 未开始端到端原生验收（Task 22）或 Current 产品文档同步（Task 23）。
- 代码签名、Apple 公证、正式安装包、上架、公开发布和发售仍不是任务、阻塞或验收项。
