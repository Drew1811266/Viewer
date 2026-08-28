# 持续评审阶段 B：实施与验证记录

> Status: Development evidence
>
> 日期：2026-08-27。任务 6–8 完成；任务 9–10 与阶段 B 全量检查点尚未完成。

## 范围

继续用户批准的[六阶段实施计划](../superpowers/plans/2026-08-27-viewer-continuous-review-and-agent-handoff.md)。
工作区为 `/Users/abc/Project/Viewer/.worktrees/continuous-review-domain`，分支为
`codex/continuous-review-domain`。原工作区、真实项目和原始素材未改动，未合并或推送。

任务 6 提交 `3af28d1`；此前暂停事实见
[暂停检查点](../progress/2026-08-27-continuous-review-phase-b-paused-checkpoint.md)。

## 任务 7

- 新增 Application 独立仓储端口／数据模型与 Infrastructure adapter；不让应用层引用 wire DTO。
- 提交明确携带 production 作用域，已有 Stream 归属不可改写；多个 Stream 分别保存当前引用。
- 新旧 writer 共用原 write.lock，克隆 writer 串行提交；固定目录和锁文件身份，防止路径替换。
- 复用 atomic 模块，增加目录句柄相对的 link／rename／同步；同名不可变文件须逐字节相等。
- 先验证输入与期望，再安装证据／状态／存档／声明，核验引用后再次检查索引，最后单点发布。
- 存档核验真实依据和精确移出结果；历史只认已提交可达引用。不存在的存档或声明不能成为操作依据。
- 跨空快照保留身份不变量，同素材版本底图不能被替换；PNG 安装／读取逐文件流式验证。
- 拆分 prepare、commit、history、archives、references、evidence、usage、mapping、owned_io 模块。
  为复用纯规则，Domain 新增窄 `validate_shared_identities` 入口，不把文件验证挪入 Domain。

本任务不包含原生捕获／渲染、SourceCheck 实现或实际产品入口接入。旧索引不自动迁移，
legacy 来源的连续状态仍需后续显式迁移适配，不能以仓储端口存在宣称迁移已完成。

## 聚焦验证

本轮先重现任务 7 缺失接口的 RED；新增存档、证据、来源引用、租约替换与身份负例均先失败再实现。

- 新仓储集成测试：20 项通过。
- 旧仓储回归：49 项通过。
- 原子写入目录句柄回归：2 项通过。
- Domain：111 项通过。
- Infrastructure 全目标严格 Clippy 通过；此前测试夹具的一项冗余 clone 已修正，未放宽规则。
- `pnpm architecture:boundaries` 通过，包括前端 8 项契约与 Desktop 8＋4 项安全边界测试。

阶段 B 的完整 `pnpm verify:clean` 和独立复审仍须在任务 8–10 完成后执行。
不能以这些聚焦结果替代阶段 B 完整验收。

## 任务 8

任务 7 提交为 `5b4337a`。本任务先查已提交命令再判断 CAS：同命令／同摘要返回原回执，
即使当前已前进；异摘要报冲突，损坏或超限报无法确认，不把不可达残留文件当成功。
六个故障点包含索引 rename 后、目录 fsync 前的不确定窗口。恢复草稿仅供 Viewer 恢复输入，
具有显式 stream_id 与内部闭合协议 `viewer.review.recovery/1`，不会进入当前／历史意见。
列表最多 10,000 文件／合计 64 MiB；不清理提交历史和共享证据。

已有文件按句柄验证真实大小写，避免每读一节点扫描整个目录。历史单次追溯上限 10,000，
超限不掩盖可独立核验的当前快照。存档覆盖区分每个 checkpoint：有效覆盖不能重复存档，
显式恢复解除覆盖后可以再次存档，旧记录不改写。

聚焦验证：恢复 13 项、仓储 20 项、旧仓储 49 项、原子边界 3 项均通过；
Infrastructure 全目标严格 Clippy 通过。包括真实文件写失败、第二次 CAS、并发重试、
64 MiB／16 MiB 文档限制与 10,000／10,001 节点边界。全阶段门禁仍待任务 9–10。

## 续接

接下来实施任务 9 的不可变底图捕获和原生编号渲染，
以及任务 10 的动态素材核验与独立实时定位映射。

Viewer 仍是开发初期；签名、公证、正式安装包、上架、公开发布和发售不属于本轮任务或验收。
