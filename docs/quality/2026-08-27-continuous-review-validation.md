# Viewer 持续评审与 Agent 交接验证报告

> Status: Complete development evidence
>
> Runtime baseline: `b46e11b69522034718d4647ca6de1f3940376f32`
>
> Validation date: 2026-08-31
>
> Scope: Tasks 1–23, `viewer.review/3` continuous review and old-feature regression

## 1. 结论边界

本报告证明当前开发分支的已测范围；不代表正式发行、所有机器兼容或外部 Agent 实际执行。
Viewer 仍处于开发初期。签名、Apple 公证、正式安装包/DMG、商店上架、公开发布和发售未执行，
也不是本轮计划、门禁、阻塞或默认后续事项。

验证中的“存档成功”只表示精确旧要求与 current 分离；不表示 Agent 已读取、已执行、已修复，
也不表示用户验收通过。返工依据声明同样不证明执行或素材 lineage。

## 2. 持续评审自动化

Task 22 在提交 `b46e11b` 上完成以下通过结果：

| 范围 | 结果 |
| --- | --- |
| App/workbench/coordinator/history focused | 13 files, 238/238 passed |
| Application continuous service | 32/32 passed |
| Infrastructure continuous repository | 25/25 passed |
| Explicit legacy migration | 13/13 passed |
| Desktop review-workspace DTO | 7/7 passed |
| Rust→Node continuous scenarios | 8/8 passed |
| Review protocol | 54/54 passed |
| Combined legacy + continuous loop | 15/15 passed |
| Visual-acceptance CLI | 17/17 passed |
| UI acceptance | 14 files, 104/104 passed |
| Task 22 full `pnpm verify:clean` | exit 0; UI 1,329 passed / 1 skipped; Rust, clippy, security and license gates passed |

Rust→Node scenarios覆盖两次完整存档循环、部分共享意见、源替换、未知依据、恢复冲突、回执
丢失和旧数据迁移。Node 只负责受控临时工程与独立读取；实际写入由生产 Rust service/repository/
macOS evidence 路径完成。读取器不写回执、不修改工程，也不伪造 Agent 执行事实。

## 3. 受控原生多轮流程

输入源为用户指定的 `/Users/abc/Downloads/测试图`。为了避免把 4.1 GiB 原目录作为可写实验
工程，仅复制精确三张项目图片和一张替换图片到 Viewer 所有的
`/tmp/viewer-task22-native-v3.MZ9I52`；原目录从未作为 Viewer 项目打开，也未被产品代码自动扫描。

真实原生工作台完成了：

1. 在同一张图片保存 2 个矩形和 2 个画笔自然语言意见；
2. 关闭并冷启动后恢复 4 条意见与几何；
3. 仅在受控副本中替换同路径素材，UI 按精确 Asset Version 失败关闭并要求确认；
4. 手动存档精确 4 条旧意见，独立读取为 current=0/history=4；
5. 在返工素材上保存 1 条新矩形意见；
6. 再次冷启动后，网格显示 current=1，生产“历史”入口可用；
7. 唯一 archive 直接打开并显示旧 4 条为“仅供背景查看，不是当前执行要求”；
8. 重新打开图片只恢复新 current 的 1 条意见，没有把历史 4 条重新变成当前任务。

最终 current：snapshot `39efb64b-b62b-43c5-b1b5-fc2cea20c7f6`，1 条 actionable；旧源
`changed`，替换源 `match`。最终 history：archive
`d72b2786-c02f-435f-875e-ffd581a64b94`，4 条精确旧原文。证据：

- `/tmp/viewer-task22-native-v3.MZ9I52/evidence/08-history-enabled-and-open.png`
- `/tmp/viewer-task22-native-v3.MZ9I52/evidence/09-final-current-one-history-four.png`

原始四文件最终 SHA-256 与初始值一致：

- `2C2A7413.JPG`: `437b42605afb190d898baaeac440d4daccb0c5a95c88496289fbe9d3b57482ef`
- `2C2A7418.JPG`: `b5eb2c6b9ed4d3948980b679cbfd527be02829e952e32bdb6884eef5afd5df3c`
- `2C2A7426.JPG`: `f00eeb4fdfcfe6e8e53e286c920ddd5171bbbeea300683227818b44545c69735`
- `2C2A7433.JPG`: `2ed7d415716f05b160d32d86326fd60d0c081fe9d633ee0987136d43c16b8f1a`

临时 `.app` 只用于让辅助技术精确选择本工作树 debug 可执行文件；可执行文件哈希完全一致，
不是安装、签名、公证、打包、发布或发售产物。验收完成后 wrapper 与开发进程均已停止。

## 4. Task 23 旧功能专项回归

已执行并通过：

```sh
pnpm --dir ui test \
  src/App.test.tsx src/components/ContentBrowser.test.tsx \
  src/components/FolderOverview.test.tsx src/components/ImagePreview.test.tsx \
  src/components/imagePreview src/components/VideoPreview.test.tsx \
  src/components/videoPreview src/components/SearchToolbar.test.tsx \
  src/components/SearchResults.test.tsx src/components/RenameDialog.test.tsx \
  src/components/BatchRenameDialog.test.tsx src/components/DestinationDialog.test.tsx \
  src/components/OperationResults.test.tsx src/components/OrganizationDragPreview.test.tsx \
  src/app/workspace/useOrganizationCoordinator.test.tsx \
  src/state/useOrganizationPointerDrag.test.tsx src/components/ReadOnlyBanner.test.tsx \
  src/state/useReviewShortcuts.test.tsx
```

结果：36 个 UI 测试文件、411/411 通过。覆盖图片预览/缩放/导航、视频交互、搜索、网格和
文件夹投影、重命名/批量重命名、目的地/结果、组织拖动、快捷键与只读界面。

```sh
cargo test --locked -p viewer-application
cargo test --locked -p viewer-infrastructure
cargo test --locked -p viewer-platform-macos \
  --test video_surface_geometry --test video_theater_layout --test video_window_aspect
```

结果：Application 141/141 通过；Infrastructure 509/509 通过、1 个明确 ignored 的性能测量；
macOS 视频表面/剧场/窗口几何 21/21 通过。覆盖读写/只读项目、扫描/搜索、图片缓存、文件
事务/冲突/恢复/撤销、监听、视频探测/缩略图/缓存，以及持续评审仓储/迁移/读取。

提交 `e225f90` 后补充尝试了 `pnpm accept:native-smoke -- --viewport 1024x720`。验收器在任何
fixture 或 UI 操作前以 `PRECONDITION_WINDOW_COUNT` 停止：目标进程存在，但没有发现可控主窗口；
随后 Computer Use 明确报告 Mac 已锁定且无法自动解锁。因此该额外旧功能原生 smoke 为
0/15 journey 执行，不计作通过，也不以浏览器或单元测试替代。进程组随后正常停止。第 3 节
Task 22 的真实持续评审原生多轮流程是独立的既有通过证据，不受这次补充预检失败影响。

## 5. 发现并解决的问题

原生验收不是一次性通过：

- 第一次冷启动暴露同内容被重新分配 AssetVersion、导致 grid=4 但大图=0 的缺陷；通过只在
  严格 `SourceCheckStatus::Match` 后恢复持久身份，并让版本不一致的工作台失败关闭解决。
- 随后的混合版本复审发现 guard 错误查看所有保留 asset row；改为只连接当前 feedback target
  引用的同实体版本，既阻止旧目标误绑，又不让无引用孤儿阻塞新评审。
- archive 可独立读取但生产“历史”按钮禁用；根因是只有 acceptance 注入 selector。最终由
  repository/application/desktop typed catalog 提供 exact-stream selector，0/1/多条分别禁用、
  直接打开、显式选择，UI 不解析 `.viewer` 或按时间猜测。

每轮修复均先有 RED，再通过 focused/full gates 和独立复审。最终复审为 Critical 0、Important 0、
Minor 0；失败截图保留为历史诊断，不计入通过证据。

## 6. 最终门禁

2026-08-31 在最终文档状态上执行并通过：

| 门禁 | 结果 |
| --- | --- |
| `git diff --check` | exit 0 |
| 修改文档相对链接目标检查 | 16 个 Markdown 文件，全部目标存在 |
| `pnpm test:policy` | 33/33；48 项范围映射恰好一次 |
| `pnpm architecture:boundaries` | exit 0；UI 契约 8/8、桌面安全契约 12/12 |
| `pnpm architecture:trends` | exit 0；48 条分支级非阻断告警，未修改基线隐藏问题 |
| `pnpm verify:clean` | exit 0；完整 quality + security 流程通过 |

`verify:clean` 内再次通过评审协议 54/54、legacy + continuous 闭环 15/15、UI 全仓 148 个
测试文件（1,329 通过、1 个既有跳过）、UI check/build、Cargo fmt/clippy、完整 locked Rust
workspace、安全、Rust/npm 许可证及视频许可证门禁。Rust 中明确忽略的项目是需要打包视频运行时、
macOS 原生性能环境或显式 `--ignored` 的测量，不被写成已执行。

`test:video:packaging` 的 30/30 只验证开发/发行脚本与运行时安全契约；本报告不把它解释为已经
签名、公证、生成正式安装包或具备发布条件。最终整分支复核没有遗留 Critical、Important 或
Minor 发现。

## 7. 明确未执行

- 未让任何真实外部 Agent 自动读取、执行或生成返工素材；协议验证只证明可独立消费。
- 未验证视频时间段标注，因为产品尚未提供该功能。
- Task 23 补充的 15 项旧功能原生 smoke 因 Mac 锁定停在安全预检，0 项实际执行；对应能力只
  声明自动化专项回归通过，不声明本次额外原生 journey 通过。
- 未执行签名、公证、正式安装包/DMG、商店上架、公开发布或发售。
- 未把原始“测试图”目录作为可写工程，未修改或整目录复制该 4.1 GiB 数据。

这些项目不会被换写成“通过”。前两项是产品/集成边界，后两项是明确的阶段与安全边界。
