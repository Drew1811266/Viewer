# 持续评审阶段 B：暂停检查点

> Status: Development evidence
>
> 日期：2026-08-27。用户要求「先暂停，记录当前进度」。
>
> 执行已暂停；任务 1–6 完成，任务 7 处于 RED 测试阶段。阶段 B 尚未完成。

后续：用户已要求「继续」，正在同一工作区从任务 7 恢复。本文保留暂停时事实；
最新执行状态以实施计划为准。

## 续接位置与工作区

- 工作区：`/Users/abc/Project/Viewer/.worktrees/continuous-review-domain`。
- 分支：`codex/continuous-review-domain`；当前 HEAD：`3af28d1`。
- 原工作区 `/Users/abc/Project/Viewer` 仍为 `codex/image-annotation-review-workbench`，
  HEAD `d7abb9961f377ae257c3283ef7893218ec0c53f9`；暂停时检查为干净，未合并、未推送。
- 按[已批准实施计划](../superpowers/plans/2026-08-27-viewer-continuous-review-and-agent-handoff.md)
  继续阶段 B（任务 6–10），不重新开始产品方向讨论，也不跳到 UI 接入。
- 本暂停记录、索引／计划状态更新和任务 7 测试保留为未提交变更；没有为了暂停另建实现提交。
- 无仍在执行的子代理实现任务；先前只读审阅均已结束。等待用户再次要求继续，不自动恢复开发。

## 已完成

阶段 A（任务 1–5）已完成纯规则、全仓验证和独立复审，详见
[阶段 A 记录](../reviews/2026-08-27-continuous-review-domain-phase-a.md)。

阶段 B 的任务 6 已提交：`3af28d1 feat(review): define versioned continuous review wire contracts`。

- 新增 v3 状态、索引、存档、读取结果及独立 usage/1 声明的 Schema 与 Rust 编解码。
- 保持 Domain／Application 与 wire DTO 的依赖隔离；旧 v1/v2 接口不静默接收或转换 v3。
- 闭合字段、显式 nullable、规范 UUID／摘要、JavaScript 精确整数、路径和资源限制均有校验。
- 当前与历史读取角色分离；历史按依据快照分组，legacy 保留原身份与能力缺失说明。
- 证据清单与目标版本／编号对应；JSON 编码过程中限制缓冲增长。
- 新增黄金 JSON、六种隔离临时工程夹具及 Rust／Node 契约测试。
- 已同步 ADR 0006、API 基线、计划与文档索引；未将这些协议能力宣称为当前产品行为。

## 暂停前的验证结果

以下是本轮已运行结果，不表示暂停时重新运行了全部验证：

| 命令 | 结果 |
| --- | --- |
| `cargo test --locked -p viewer-infrastructure --test review_protocol_v3_contract` | 19 项通过 |
| `cargo test --locked -p viewer-infrastructure --test review_protocol_contract` | 旧协议 14 项通过 |
| `cargo test --locked -p viewer-infrastructure --lib review::protocol::v3` | 缓冲边界 1 项通过 |
| `pnpm test:review-protocol` | 30 项通过 |
| `pnpm test:policy` | 33 项通过 |
| `cargo clippy --locked -p viewer-infrastructure --all-targets -- -D warnings` | 任务 6 提交前通过 |
| `cargo test --locked -p viewer-infrastructure --test review_repository` | 旧仓储基线 49 项通过 |

任务 6 提交前已执行格式化和 diff 检查。本轮尚未执行阶段 B 的完整 `pnpm verify:clean`
及独立复审；阶段 A 的历史全量通过不能替代阶段 B 验证。

## 未提交的任务 7 工作

新增且尚未跟踪的测试文件：
`crates/viewer-infrastructure/tests/continuous_review_repository.rs`。

其中六项测试覆盖：只读打开不创建目录；首次提交可见；陈旧 CAS 不覆盖当前；新旧写入口
共用租约及只读权限；克隆 writer 并发 CAS 与多 Stream 保留；历史可达性及摘要；符号链接隔离。
这些是六个测试函数，部分函数覆盖多个断言场景。

已运行以下命令并确认 RED：

```sh
cargo test --locked -p viewer-infrastructure --test continuous_review_repository
```

失败原因是计划中的 `viewer_application::review_workspace` 模块以及 provider 的
`continuous_reader()`／`continuous_writer()` 尚未实现；并非已完成任务 6 的行为回归。
**当前工作副本包含预期编译失败的测试，不能宣称全量测试通过。** 测试保留原样，未禁用、
未删除，也没有补写生产代码来掩盖暂停位置。

任务 7 的应用端口、continuous 仓储、提交和历史模块尚未创建。现有 `atomic.rs`、`lease.rs`
与旧仓储只进行了阅读及基线测试，尚未修改。

## 续接时的技术注意点

1. 先核对工作区和上述未提交变更，再从任务 7 的 RED 测试继续；不要在原工作区实现。
2. 测试暂按 `ReviewCommitRequest.production: Option<ProductionScope>` 表达 Stream 作用域。
   这是待核对的接口细化：Domain state 只有 Stream ID，而 v3 索引需要 task／batch；
   续接时先核对并同步计划／ADR，再实现。不得猜测生产作用域或改写现有 Stream 的归属。
3. 复用现有 `ProjectReviewLease` 和同一个 `write.lock`，复用／扩展 atomic 基础设施；
   不新建并行锁或第二套事务引擎。写锁需覆盖 writer 生命周期，克隆共享串行提交守卫。
4. 在写不可变文件前检查 expected，发布索引前再次核对；只读端不创建目录；
   不把未被已提交引用链指向的孤立文件认作历史或成功命令。
5. 路径保护需覆盖祖先目录替换／符号链接及普通文件校验，不能只做一次路径字符串检查。
   是否扩展为目录描述符相对 IO 仍待实现取舍，不是已完成能力。
6. 跨历史的身份不可变、真实摘要、父链／存档依据可达性尚属任务 7–8；
   wire 解码成功不能代替这些检查。命令查找不可用与明确不存在必须区分。
7. 任务 8 再补幂等、恢复草稿和各持久化边界的故障注入；任务 9 实现不可变底图捕获／渲染；
   任务 10 实现增量素材核验。上述任务均未开始实现。
8. 任务 10 必须明确独立实时定位映射的归属：历史 AssetVersion 的路径／mtime 不原地改写；
   已确认同内容的移动／改名也不能伪装成新产出版本。
9. 完成任务 7–10 后才做阶段 B 全量门禁、独立复审和阶段记录；不能仅以任务 6 通过结束阶段 B。

## 保持不变的边界

- 不切换 UI、旧生产入口或外部 Agent 读取入口，不迁移真实工程。
- 不修改用户原始素材；“下载/测试图”没有在本轮被写入，测试仅用隔离临时工程／静态夹具副本。
- 存档与使用声明不推断 Agent 已执行或问题已解决；当前空意见不推断全部通过。
- 用户选择在本任务内逐项实施；未经新的明确授权不并行分派实现。
- Viewer 仍处于开发初期。签名、Apple 公证、正式安装包、上架、公开发布和发售不在本轮范围，
  也不是恢复开发、验收或收尾的默认任务。
