# 持续评审阶段 B：实施与验证记录

> Status: Development evidence
>
> 日期：2026-08-27。任务 6–10 实现与聚焦验证完成；阶段 B 全量检查点正在进行。

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
- 复用 atomic 模块，增加目录句柄相对的发布／同步；同名不可变文件须逐字节相等。
  检查点复审已将 create-once 的 link／unlink 改为排他 rename，见下文。
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

## 任务 9

任务 8 提交为 `9bfb338`。新增无 serde／IPC 构造的 BoundReviewImage：持有已验证 PNG
不可变内存与完整 AssetVersion，坐标为 EXIF 正向、左上角原点，方向已经烘焙进 PNG。
可信适配器在构造前验证摘要与来源绑定；渲染器再次验证摘要、格式和尺寸，仅接受 Base 角色。
源文件分块复制到私有 0700 临时目录，ImageIO 只读副本；复制／解码／编码前后验证文件身份。
整图意见也保留底图，多目标可共享一个 Feedback；沿用原生画笔／矩形与 marker 状态隔离。

ReviewEvidenceResult 的 staging lease 在提交复制完成前保持临时文件，最后一个所有者释放时
只清理自有 scratch；不会删除 repository evidence。仓储 Snapshot／Archive／Legacy 选择器
只读取已提交可达引用；不同依据有不同编号图时返回 AmbiguousEvidence，要求选择精确快照。
Legacy 只有已画标记的预览，不能冒充干净底图；EvidenceAbsent 与 Integrity 分开。
未接入真实旧索引迁移或 UI，Task 11 必须保证 PreparedReviewAsset 就是用户正在评审的版本。

验证：先复现缺少捕获／仓储接口的 RED；新增原生 5 项、仓储 22 项、恢复 13 项、
v3 协议 19 项通过；原生 `review_` 聚焦共 15 项通过（包括既有标记回归）。
Infrastructure／macOS 全目标严格 Clippy 通过，4 GiB 总量仍由协议／仓储逐文件预算约束。
阶段 B 全仓门禁与独立复审仍待任务 10 后执行。

## 任务 10

任务 9 提交 `d542928`。新增 ContinuousReviewAssetPort 与独立实现模块，保留 legacy 端口行为。
新增素材增量合并变化跟踪，不清空旧成员；源检查每次分块计算 BLAKE3，核验读取前后身份，
不依赖 size／mtime／watcher 快捷判断。检查结果只作当前投影，不修改保存的 AssetVersion。

已确认定位映射由 IndexedReviewAssetCatalog 会话持有，明确候选实体／路径、摘要验证及前一
定位 CAS 后更新。同实体相同内容与已确认定位复用历史版本；重复内容不自动合并业务身份。
映射不是跨进程协议：后续独立 reader 无映射时须安全核验历史路径或返回待确认，不能猜测。
原生捕获因此分别核验可信 PreparedReviewAsset 的实时实体与历史内容摘要，保留旧路径／mtime；
不是仅因元数据变化而把同一内容伪造成新版本。

显式 SourceBindingDecision 只改变选中目标并分配新 revision，旧资产、原文和其他目标保持不变。
ProducerVerifiedAndPositionConfirmed 是纯规则输入契约，Application 必须先核验声明并取得位置确认，
不能把构造该枚举等同于已验证外部声明。本阶段没有接入自动绑定或 Agent 执行。

聚焦验证：新素材 6 项、读中变化／取消单元 2 项、旧素材 9 项、仓储 22 项、
恢复 13 项、原生 `review_` 16 项及 Domain 112 项通过；严格 Clippy 通过。
补充 4 GiB 唯一 PNG 总量边界测试，仅构造引用，不分配／读取 4 GiB 内容。

已运行非阻塞 architecture:trends，报告 38 项警告（包括既有大文件和本阶段闭合验证函数分支复杂度）。
本轮仅在既有 review_session 错误转换中补充新端口错误分支，没有增加编排职责；
未修改趋势基线或用豁免隐藏警告。源读取与协议验证仍须以独立复审检查其边界和成本。

## 续接

### 检查点复核进展

首次 `pnpm verify:clean` 在 `a0e2192` 上通过：前端 132 文件、1,142 通过／1 跳过，
Rust 全量、格式化、严格 Clippy、架构／安全边界及离线依赖许可检查通过；门禁没有改变工作树。
日志为隔离工作区 `target/continuous-review-phase-b-verify.log`（生成日志，不纳入源码）。

随后主审补测复现两项遗漏：Ready 索引尺寸可能过时；探测期间同 inode、同 size／mtime
覆盖可能绕过准备末尾检查。仅在 continuous adapter 强制重新探测尺寸，并对整个
hash＋probe 区间核验含 ctime 的 MediaFileIdentity。先观察两项 RED，单独刷新探测后仍有
一项 RED，再补身份区间核验使两项 GREEN。新素材集成现为 8 项，旧素材 9 项仍通过，
Infrastructure 严格 Clippy 通过。旧端口正常行为不变。

上述修正提交为 `5cdc406`；其完整 `pnpm verify:clean` 再次 exit 0，前端计数不变，
Rust／静态／边界／离线许可检查通过。日志为 `target/continuous-review-phase-b-verify-final.log`。

### 独立复审与修正

只读复审范围为 `25f38cb..a0e2192`；未修改工作区，结论为三个 P2 问题，暂无 Critical／Minor。
复审亦确认主审补查的 Task 10 区间问题。按 receiving-code-review 逐项复现、修正：

- **恢复总量**：旧实现接受第 10,001 条／超过 64 MiB 的合法输入，随后整个列表无法读取。
  两项新增测试先观察 `Ok` 而非 `LimitExceeded` 的 RED；写前在同一守卫下校验替换后的
  数量和总字节。边界允许替换，扩容拒绝且保留旧输入；读写共用清单规则，最多 64 暂存
  残留另行有界忽略。另复现并修正满额草稿加暂存文件被错误计数的问题。
- **中断发布**：真实测试子进程在不可变对象发布后、同步前退出，无析构清理；旧实现残留
  `nlink == 2`，RED。改为 descriptor-relative 排他 rename 后保持单链接，已有字节仍不可覆盖；
  不放宽任意硬链接的安全拒绝。Darwin 本机执行验证，Linux 分支尚无本机运行证据。
- **存档时间顺序**：构造存档后恢复同一目标、将旧存档依据改为后来快照并重算合法摘要；
  旧实现错误接受，RED。改为从 checkpoint.before 向祖先追溯后拒绝，不改动当前或历史文件。

聚焦 GREEN：原子 4 项、新容量 2 项、存档完整性 1 项、恢复 13 项、仓储 22 项、
旧仓储 49 项通过，Infrastructure 全目标严格 Clippy 与 diff 检查通过。
修正后的全仓门禁和原复审者复验尚待记录，不能用此聚焦结果宣称检查点完成。

非阻塞性能后续项：大量存档时，每次保存对 archive／usage 的全量核验会重复追溯父链，
存在约 O(存档数 × 快照数) 的 IO 成本。Task 11 明确补量测；尚不声称大历史下延迟达标。
优化只能采用有界、单次操作内验证复用，不能降低完整性保证。

完成阶段 B 全仓门禁和只读独立复审后，下一阶段从任务 11 的 Application 用例接入开始。

Viewer 仍是开发初期；签名、公证、正式安装包、上架、公开发布和发售不属于本轮任务或验收。
