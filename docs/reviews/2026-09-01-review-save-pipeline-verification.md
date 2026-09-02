# 快速评审保存与后台发布验证报告

> Status: Development evidence
>
> Date: 2026-09-01
>
> Scope: 评审作者态提交、后台证据生成与 v3 发布、Agent 读取门禁、恢复与回归验证

## 结论

本轮把“保存评审意见”和“生成证据并发布给外部 Agent”拆成两个有明确一致性边界的阶段。
前台保存只提交作者态命令、逻辑快照、双头状态和 outbox；后台 materializer 再生成证据并
原子发布既有 `viewer.review/3` 数据。外部 Agent 只有在 `authoring_head == published_head` 时
才能读取当前意见，发布中返回明确的 `publication_pending`，不会把旧发布误报成最新数据。

这次改动没有增加产品功能，没有改变评审、存档、恢复或 Agent 数据的业务语义，也没有
引入网络服务或新的运行时依赖。

## 架构验收

- 正常保存唯一入口是作者态短事务，返回持久化 receipt 与确定性 UI patch，不再同步生成
  PNG、扫描证据目录、发布完整 v3 数据或重载整个 workspace。
- SQLite 作者态以命令 ID 和 payload digest 保证幂等；丢失回执后的同命令重试返回原回执，
  不重复执行。取消只阻止尚未提交的新命令，不会把已提交重试误报为未保存。
- 后台任务通过 durable outbox、租约和可恢复状态推进 `published_head`；失败不会形成半发布
  的 Agent 可读状态。
- 显式旧版迁移仍走完整校验路径；普通可写项目使用作者态视图，真正只读项目继续读取不可变
  已发布视图，且不会尝试创建 SQLite 状态。
- 桌面桥接不再保留旧的“保存后返回完整 workspace”DTO，UI 通过增量 patch 原地更新。

## 前台保存性能

门禁命令：

```text
cargo test --locked --release -p viewer-desktop --test review_save_performance -- --ignored --nocapture
```

测试在每次样本中包含命令准备、`synchronous=FULL` 的 SQLite 作者态提交和真实桌面 DTO
序列化。覆盖 asset-only、首个图片区域、追加区域、纯文本修改，以及 1/10/100/1000 个不变
的 64 KiB 证据占位文件；共 1000 个样本。慢速 materializer 用于证明后台工作不会进入前台
计时，spy 同时断言前台源文件读取、证据渲染和 v3 完整仓库打开次数均为 0。

| 指标 | 实测 | 门限 |
| --- | ---: | ---: |
| p50 | 0.874 ms | <= 50 ms |
| p95 | 1.116 ms | <= 150 ms |
| p99 | 2.155 ms | <= 300 ms |

按证据文件数量统计的 p95 分别为：1 个 4.253 ms、10 个 1.485 ms、100 个 1.343 ms、
1000 个 1.943 ms。结果证明前台延迟不随既有证据数量线性增长。

## 真实素材验证

命令：

```text
cargo run --locked --release -p viewer-desktop --example review_save_latency -- --project "/Users/abc/Downloads/测试图" --samples 30
```

工具从真实目录递归选择首个受支持图片
`/Users/abc/Downloads/测试图/衣服/A01/2C2A7413.JPG`，复制到隔离临时项目后运行，未修改
原测试素材。作者态和后台物化分别计时：

| 阶段 | 场景 | p50 | p95 | p99 |
| --- | --- | ---: | ---: | ---: |
| 作者态提交 | 整图意见 | 1.872 ms | 2.357 ms | 2.393 ms |
| 作者态提交 | 区域意见 | 1.439 ms | 1.777 ms | 1.851 ms |
| 后台物化 | 整图意见 | 27.989 ms | 31.065 ms | 664.041 ms |
| 后台物化 | 区域意见 | 317.080 ms | 333.412 ms | 809.406 ms |

后台物化的长尾不阻塞保存按钮；它只延长“正在准备给 Agent”的状态，并继续受双头门禁保护。

## 故障与兼容矩阵

下列 47 项基础设施测试全部通过：恢复 14 项、物化 7 项、读取器 12 项、存档完整性 1 项、
迁移 13 项。

```text
cargo test --locked -p viewer-infrastructure \
  --test continuous_review_recovery \
  --test continuous_review_materialization \
  --test continuous_review_reader \
  --test continuous_review_archive_integrity \
  --test continuous_review_migration
```

额外通过桌面 materializer 生命周期 2 项、workspace command 6 项（含显式旧版迁移）、桌面
lib 74 项，以及评审协议 54 项和持续评审端到端链路 15 项。完整 Rust workspace 测试退出码
为 0；要求真实已暂存视频运行时的原生验收用例按原设计保持 ignored，独立 release 性能门已运行。

## 完整质量门

最终执行 `pnpm quality` 并以退出码 0 完成。该门禁覆盖文档与范围策略、评审协议、持续评审
端到端链路、架构边界、安全边界、视频打包策略、clean-verifier 测试、UI 静态检查、UI 全量
测试、UI 生产构建、Rust 格式检查、全 workspace Clippy `-D warnings` 和全 workspace 测试。
其中 UI 为 151 个测试文件，1381 项通过、1 项按设计跳过。

## 验证环境

- Apple Silicon `arm64`
- macOS 26.5.2（25F84）
- 内置固态硬盘，APFS
- device/allocation block size 均为 4096 bytes
- Rust 工具链和依赖均使用仓库锁定版本

## 当前开发边界

Viewer 仍处于开发初期。本报告只证明源码开发和内部开发验证范围；代码签名、Apple 公证、
正式安装包或 DMG、应用商店上架、公开发布和发售均不属于本轮任务，也不是本轮完成条件。
