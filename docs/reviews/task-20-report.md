# 持续评审 Task 20：历史恢复与来源确认

> Status: Development evidence

## Fix round — 2026-08-30

修复提交：`25b619a833d2fdc53ad8defe62d872064ce7dc9c`

- 恢复预览现在冻结 `decisions + plan`；只有完全相同的已展示决策才可调用 coordinator 的
  `restore`。选择变更立即使旧预览在 UI 中失效，必须重新展示影响后确认。
- 面板明确列出预览将恢复的目标、存档覆盖撤销与待重新核验素材，并不把这些影响表述为已经执行。
- snapshot 的空选择从实际可见历史目标派生单键 continuation refs；任何 legacy、多键或空键
  continuation 都以 `invalid_data` 拒绝。
- 真实 legacy DTO 即使 `entries=[]` 也显示原文、目标和素材；历史证据缺失仍是能力限制，
  不伪装为无内容或完整性错误。
- 历史读、证据、准备候选与恢复操作采用带 kind/token 的 busy 状态；entity、selector、session
  或关闭会使过期 UI 结果失效并释放可读操作的忙碌状态。已经开始的 coordinator 写命令不被 UI
  声称为已取消。
- 历史面板和来源确认保持单一模态；从来源确认返回会显示保留的历史面板。初次历史读取失败会
  在 context bar 中报告，而不是伪装为空历史。脏输入的内部连续存档入口可显示“先保存/取消”
  指引，且不提交输入。

## RED → GREEN 与验证

先加入并观察了恢复决策变更、multi-key continuation、snapshot 空选择、legacy `entries=[]`、
来源候选失效、modal exclusivity、首次 integrity 读取失败、dirty archive、以及 entity/selector/
session/close busy 生命周期的失败测试；随后最小实现转绿。

- 聚焦回归：5 个文件、51 项通过。
- UI 全仓：145 个文件、1285 项通过，1 项既有跳过。
- `pnpm --dir ui check` 与 `pnpm --dir ui build` 通过。
- `pnpm architecture:boundaries`（含 contracts）通过。
- `pnpm architecture:trends` exit 0；47 项既有/非阻断告警，Task 20 未新增 history Hook 长函数告警。
- `git diff --check` 通过。

持续评审生产入口仍未启用；未修改 Rust、协议或 `useImageReviewWorkbench`。

## Fix round 2 — 2026-08-30

修复提交：`19e8b98fb4e358796735a0e4756197d572beafdd`

- 展示读取与 coordinator 写事务拆为独立的 `presentationOperation` 与
  `transactionOperation`。selector 可以隐藏旧展示并失效读取；entity 只失效素材准备。
  已提交的 restore/continue 写事务以同步 guard 保持到 promise settle，拒绝重复、关闭和其他
  公共历史操作，不把写入误称为取消。
- 同一 session 即使 selector/entity 改变仍报告写入成功或失败；真正 session 替换不会把旧结果
  带入新 session。新增 pending restore/continue 跨 lifecycle、重复调用和最终结果回归。
- context bar 现在优先显示 history integrity error，不被较早的 dirty archive notice 覆盖。
- 来源确认同步派生候选与 target identity 的有效性，并在点击时再次验证；候选移除或 target
  替换的瞬时渲染不能继续确认。

### Fix round 2 verification

- 聚焦回归：5 个文件、55 项通过。
- UI 全仓：145 个文件、1289 项通过，1 项既有跳过。
- `pnpm --dir ui check`、`pnpm --dir ui build`、`pnpm architecture:boundaries`（含 contracts）
  通过；`git diff --check` 通过。
- `pnpm architecture:trends` exit 0，47 项既有/非阻断告警；拆分 restore 预览/提交路径后没有新增
  Task 20 trend warning。
