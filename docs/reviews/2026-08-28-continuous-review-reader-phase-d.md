# 持续评审阶段 D：原生只读读取器候选验证

> Status: Development evidence
>
> 2026-08-28；Task 14–15 候选实现通过两轮全仓门禁，独立复审待完成。
> 累计已验收仍为 13 / 23；Task 16 桌面桥接及阶段 E/F 尚未开始。

## 范围和架构

工作树 `/Users/abc/Project/Viewer/.worktrees/continuous-review-domain`，分支
`codex/continuous-review-domain`。起点 `1dccf78`；获批架构修订提交 `7387a6b`。
本次执行用户已确认的 Node 入口 + Rust 只读核心方案，不改变产品 UI，不迁移真实工程。

- 新程序 `viewer-review-reader` 位于现有 infrastructure crate；没有新第三方依赖。
- Node 只处理参数与有界子进程传输。Rust 复用目录句柄、v1/v2/v3 编解码、证据验证、
  当前投影、存档计划和 Domain 净增量规则，没有第二份 JS v3 业务算法。
- 读取前显式 `pnpm build:review-reader`，默认可信脚本工作树的 `target/debug` 程序，
  或调用者显式绝对路径 `VIEWER_REVIEW_READER`；不自动构建／下载／扫描项目可执行文件。
- 64 KiB 闭合请求，64 MiB 有界响应；只读、无网络、无常驻进程，不启动 Viewer 或 Agent。
- index 只固定一次，正常原子替换后仍读已打开的版本；不可变对象保持更严格的身份检查。
  源按 64 KiB 缓冲完整摘要，缺失／换版／不可读均不得进入 actionable。
- 当前和显式历史入口分离；没有当前状态不回退 Completed；历史不含 actionable。
  delta 只给净变化，保留撤回与存档区别，不带旧原文；查找上限为 10,000 个节点。
- 旧 `safe-read.mjs` 及其插桩测试移除，安全覆盖迁至 Rust 真实 TempDir／文件句柄测试，
  旧文件可从 Git 检查点恢复。BLAKE3 黄金向量及全部 legacy 正负用例保留。

## 回归中实际修正

1. CLI `--list` 只在 Node 分派，不作为未知字段透传闭合请求。
2. legacy 负例改为断言稳定错误码，不依赖某一解析器的诊断文案；未放宽无效输入接受范围。
3. 旧历史预览编号按创建时间／反馈身份验证，与 JSON 数组存储顺序无关；同条意见的多个
   区域不重复占编号，标记记录可以按合法 ordinal 显式映射。
4. 已打开但被替换／unlink 的不可变 PNG 不能套用 index 的例外。
5. 存档增量必须属于该 checkpoint 实际计算出的变化；仅引用一个真实 archive ID 不足以
   证明另一个目标被存档。损坏增量不伪造空结果，完整已验证 current 仍独立返回。
6. 显式读取迁移后的 v2 Draft 时，缺少旧证据是 limitation，不捏造 PNG 或误报已声明 PNG 损坏。
7. 无 index 但存在旧 Draft 不能报合法空工程；复用已有无索引检查，不重开另一版 index。
8. v3 库支持单独的 `{ signal }` 取消控制；取消结束本次进程读取，不产生部分成功清单。

## 已执行验证

| 验证 | 结果 |
| --- | --- |
| 恢复基线旧 Node 协议测试 | 41 项，40 通过、1 失败；未验收旧实现的大源核验缺陷 |
| 原生 IO／源／传输／当前历史入口 RED | 缺实现或预期断言失败；日志保留于 target |
| 索引原子替换、目录及祖先替换、链接／非普通文件等原生 IO | 7 通过 |
| 原生源检查与 32 MiB 文件固定缓冲 | 3 通过 |
| delta 日志顺序和有界追加 | 1 通过 |
| `pnpm test:review-protocol`（第二轮门禁） | 54 通过，exit 0 |
| Rust writer → Node reader、修改后改回／撤回、10,000 与 10,001 节点 | 3 通过 |
| `pnpm verify:clean` 第一轮 | exit 0；补查旧 Draft／无 index／取消后再次全跑 |
| `pnpm verify:clean` 第二轮 | exit 0；含 UI 132 文件、1142 通过、既有 1 跳过，workspace Rust、边界和依赖政策 |
| `git diff --check` | exit 0 |
| 架构趋势 | 非阻断告警保留，不调整基线掩盖；包含既有问题与新增读取边界复杂度 |

日志：`target/continuous-review-phase-d-verify-clean-first.log`、
`target/continuous-review-phase-d-verify-clean-second.log`；专项 RED/GREEN 日志同前缀。
Task 14–15 共用原生入口及 DTO，因此作为一个可构建候选提交；各独立边界仍保留 RED→GREEN。

## 尚未完成

独立只读复审和最终检查点；Task 16 桌面命令／会话安全证据授权；阶段 E UI 和阶段 F 实机闭环。
此记录不表示新流程已在当前软件启用，也不等于阶段 D 整体完成。
签名、公证、正式安装包、上架、公开发布和发售不是任务或验收项。

原工作树已复核干净，仍为 `d7abb9961f377ae257c3283ef7893218ec0c53f9`；
未操作下载/测试图及原始媒体，未合并、推送或清理历史／恢复资料。
