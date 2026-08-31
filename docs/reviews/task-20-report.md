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

## Fix round 3 — 2026-08-31

修复提交：`7f9daa38829e5e30957ec5f9bd977d83ebd56eee`

- 未提交的来源确认现在绑定当前 selected entities。实体切换会立即撤销已展示的候选确认、清除其
  相关错误并推进 source preparation token；即使调用方仍持有旧 decision，`confirmSource` 也不能
  调用 coordinator。
- 已提交的 continue transaction 不受此撤销影响：transaction guard 仍到 promise settle，重复操作
  与误导性的关闭保持拒绝，且同 session 的最终结果仍显示。

### Fix round 3 verification

- RED：来源确认已打开后切换到空 entities 时 source 未清除；GREEN：回归证明 source 立即清除且旧
  decision 无法写入。
- 聚焦回归：5 个文件、56 项通过；UI 全仓：145 个文件、1290 项通过，1 项既有跳过。
- `pnpm --dir ui check`、`pnpm --dir ui build`、`pnpm architecture:boundaries`（含 contracts）、
  `pnpm architecture:trends` 与 `git diff --check` 通过；trends 为 47 项既有/非阻断告警，无新增
  Task 20 warning。

## Fix round 4 — 2026-08-31

修复提交：`38a6a8f`

- 根因是来源确认面板直接从通用 transport 层导入 `ReviewAnchor`，违反 review component 的单向
  依赖边界。移除该导入，`originalAnchor` 改由允许的 `ReviewSourceBindingDecision['anchor']`
  派生；运行时行为与协议保持不变。

### Fix round 4 verification

- RED：`node --test --test-name-pattern='manual review transport and dependency boundaries remain one-way'`
  稳定复现 `ReviewSourceConfirmation.tsx imports transport DTOs`。
- GREEN：同一精确 policy test 通过；`ReviewSourceConfirmation` focused 为 1 文件/5 测试通过，
  Task 20 五文件 focused 为 5 文件/56 测试通过。
- `pnpm --dir ui check`、`pnpm --dir ui build`、`pnpm test:policy`（33 项通过，48 项 scope
  requirements 精确映射）通过。
- `pnpm architecture:boundaries`（含 contracts）与 `pnpm architecture:trends` 通过；trends 为
  47 项既有/非阻断告警，无新增 Task 20 warning。`git diff --check` 通过。
- 完整 `pnpm verify:clean` 通过：review protocol 54 项、manual loop 7 项、视频 packaging 30
  项、UI 全仓 145 文件/1290 通过/1 跳过、Rust workspace 全量测试、security、npm license
  与 video license 均通过；验证过程无工作区漂移。仅有既有 Biome/canvas/chunk-size/Cargo
  dependency duplicate warnings。

持续评审生产入口仍未启用；本轮仅修改 UI 类型导入，未修改 Rust、协议、`useImageReviewWorkbench`
或生产激活。
