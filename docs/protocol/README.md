# Viewer 开放评审协议

## 状态与范围

本文档是 **Active 的协议与集成边界**。Viewer 当前持续评审同时支持 `viewer.review/3` 和
`viewer.review/4`：图片点／箭头／画笔／矩形／椭圆标注与自然语言意见在成功保存后形成完整 current，用户手动把上一轮要求存档，
外部工具通过独立 current/history 读取器消费。旧 `viewer.review/1`、`viewer.review/2` 的固定范围
Completed Round 只保留为显式迁移和历史兼容，不再是新建项目的主流程。Production Manifest
执行、自动 lineage 判断、视频时间段标注和具体 Agent 集成仍属于未来阶段。

Viewer 目前仍处于开发初期。本协议基础工作不产生签名、公证、正式安装包、发售或销售任务；后续普通功能开发也不应默认列出这些发布阶段事项，除非项目明确进入相应阶段并由用户另行授权。

## 所有权边界

- Viewer 独占写入 `.viewer/reviews/`。UI、Agent 和第三方工具都不得直接拼装或改写协议 JSON。
- 当前产品在项目中维护一个无 Production Scope 的手动 Review Stream；它不会读取或执行
  `viewer-production.json`，也不会主动启动 Agent。
- 生产工具可选择在项目内提供 `viewer.review.usage/1` 返工依据声明。只有用户明确选择、核验并
  采用后才进入 Viewer 记录；声明不证明执行、产出来源或素材 lineage。
- 外部读取方以一个精确 Stream 的 current 作为当前待办，以显式 history 选择器读取背景；
  recovery、临时文件、目录顺序、mtime 和未索引记录都不是指令来源。

```text
project/
├── optional-review-usage.json             # Producer-owned optional declaration, user selects explicitly
└── .viewer/reviews/                       # Viewer owns
    ├── index.json                         # Exact Streams and current/history references
    ├── write.lock                         # One-writer advisory lease
    ├── states/{snapshotId}.json            # Immutable complete current-state snapshots
    ├── archives/{archiveId}.json           # Immutable manual archive checkpoints
    ├── evidence/{blake3}.png               # Immutable verified image evidence
    ├── usage/{declarationId}.json           # Adopted verified declaration copies, when used
    └── recovery/                           # Bounded private recovery input / legacy-index backups
```

## 当前 v3/v4 持续评审语义

- `current` 是某一 Review Stream 现在仍有效的**完整自然语言要求集合**。成功保存先在本地
  authoring 控制面持久化完整逻辑状态，再由后台任务生成证据并原子发布公开索引；没有“完成本轮”
  门槛，也不生成默认 `pass`。
- 用户没有评价的素材不产生意见记录。批量网格和大图预览都不记录“已浏览”；曝光、停留、
  滚动或播放行为不能被读取方解释为通过。
- Feedback 文字是返工意图的权威表达；版本化 Target/Anchor 只把文字关联到精确素材版本和
  几何。v3 支持整图、图片矩形和图片画笔；v4 还支持图片点、箭头和椭圆。所有图片坐标都相对
  方向校正后的完整图片归一化到 `[0, 1]`；画笔为 2–2048 个有效点。`imageArrow.tail` 是箭尾，
  `imageArrow.head` 是箭头指向、需要关注的目标位置，读取方不得把两端对调。原图永不被标注写回。
- 文字编辑、重绘、删除和后补意见形成新 revision/state。withdrawal 是类型化移除事实，不会
  自动生成“把改蓝色改回去”之类反向自然语言。
- 手动 archive 记录精确 Feedback/Text/Target revisions 及其依据。部分存档移走所选目标，同时
  保留未选目标和 basis 之后的编辑／新增；archive 不证明 Agent 已读取、执行或修复，也不证明通过。
- history 没有 `actionable`。继续提出会创建新身份并保留 `HistoryRef`；恢复会显式追加新当前
  状态。两者都不修改旧 history。素材换版时必须确认已核验候选和位置，不按路径猜测绑定。
- 每次读取都会重新形成 `sourceChecks`。`match` 只证明检查时刻；`changed`、`missing`、
  `unreadable` 或待确认不会删除原文，执行方在返工前仍须核对目标版本。
- recovery 仅用于 Viewer 恢复未发布输入或处理已提交但回执不确定的操作；它不是 current，
  也不能授权重放。当前实现不自动清理历史、证据或恢复输入，容量到限时失败关闭。

## Review Stream 选择

current 与 history 都属于一个 Review Stream。协议没有、也不得推导项目级全局 latest。读取方必须使用以下一种方式选择恰好一个 Stream：

- 精确 `reviewStreamId`；
- 精确的 `taskId` 与 `batchId` 组合；
- 仅当项目中恰好有一个 Stream 时省略选择器。

选择结果为零或多个时，读取方必须报错，不能根据文件时间、Snapshot/Archive ID、恢复输入或目录顺序猜测。

Viewer 当前在一个项目中维护至多一个无 Production Scope 的手动 Stream，后续保存和存档都
属于该 Stream。带 `taskId`／`batchId` 的 Production Stream 仍由未来阶段生产，不能被当前
手动入口误认或改写。

## 版本与兼容性

协议字符串的斜杠后数字是主版本。读取方遇到不支持的主版本时必须保持只读并返回 unsupported；禁止用旧版本重写、降级或“修复”较新版本的数据。公开 Schema 包括：

- [`viewer-production-v1.schema.json`](viewer-production-v1.schema.json)
- [`viewer-review-index-v1.schema.json`](viewer-review-index-v1.schema.json)
- [`viewer-review-draft-v1.schema.json`](viewer-review-draft-v1.schema.json)
- [`viewer-review-round-v1.schema.json`](viewer-review-round-v1.schema.json)
- [`viewer-review-index-v2.schema.json`](viewer-review-index-v2.schema.json)
- [`viewer-review-draft-v2.schema.json`](viewer-review-draft-v2.schema.json)
- [`viewer-review-round-v2.schema.json`](viewer-review-round-v2.schema.json)
- [`viewer-review-index-v3.schema.json`](viewer-review-index-v3.schema.json)
- [`viewer-review-state-v3.schema.json`](viewer-review-state-v3.schema.json)
- [`viewer-review-archive-v3.schema.json`](viewer-review-archive-v3.schema.json)
- [`viewer-review-read-result-v3.schema.json`](viewer-review-read-result-v3.schema.json)
- [`viewer-review-index-v4.schema.json`](viewer-review-index-v4.schema.json)
- [`viewer-review-state-v4.schema.json`](viewer-review-state-v4.schema.json)
- [`viewer-review-archive-v4.schema.json`](viewer-review-archive-v4.schema.json)
- [`viewer-review-read-result-v4.schema.json`](viewer-review-read-result-v4.schema.json)
- [`viewer-review-usage-v1.schema.json`](viewer-review-usage-v1.schema.json)

v1 索引保留 `completedRoundIds`。v2 索引以 `completedRounds` 保存每个历史 Round 的
`reviewRoundId`、`protocolVersion`、相对 `location` 和文件字节的 `blake3`，因此同一 Stream
可以包含 v1 文件与 v2 bundle。索引的主版本不等于所有历史 Round 的版本；读取方必须按
记录声明分派。v3 Stream 在首次保存点、箭头或椭圆时单向提升为 v4；一旦提升，后来删除扩展
几何也不会降回 v3。v3 状态和历史仍按 v3 原样读取，v3 解码器拒绝 v4 文档，v4 解码器也不把
v3 文档冒充 v4。v1 `imageRegion` 仅按 v1 规则读取，不得当作 v2/v3/v4 Anchor 静默改写。含旧数据
的项目必须通过 Viewer 的显式迁移检查；旧字节和历史证据保持不变，旧 Completed 不自动成为持续评审 current。

## Agent 无关参考读取器

[`read-current.mjs`](../../scripts/review-protocol/read-current.mjs) 和
[`read-history.mjs`](../../scripts/review-protocol/read-history.mjs) 的 Node 入口只使用内置模块，
文件访问与协议验证由本地 Rust 只读核心完成，不依赖 Codex、Claude、OpenCode 或其他特定
Agent。它们不联网、不启动 Viewer、不启动 Agent、不写项目文件，也不把 recovery 或 history
伪装成 current。

开发者先显式执行 `pnpm build:review-reader`。默认使用本工作树 `target/debug/viewer-review-reader`；
也可用调用者设置的绝对路径 `VIEWER_REVIEW_READER` 指定可信二进制。读取时不自动构建／下载，
不从受审项目、项目配置或 PATH 查找程序。缺少核心时明确报错，不退回未验证的 Node 路径读取。
内部是有界的一次性 stdin/stdout JSON 调用，不是常驻服务；没有安装包或签名要求。

```bash
node scripts/review-protocol/read-current.mjs \
  --project /absolute/test-project \
  --stream 00000000-0000-4000-8000-000000000102

node scripts/review-protocol/read-history.mjs \
  --project /absolute/test-project \
  --stream 00000000-0000-4000-8000-000000000102 \
  --archive 00000000-0000-4000-8000-000000000601

node scripts/review-protocol/read-current.mjs \
  --project /absolute/test-project \
  --list
```

上面的绝对路径和 UUID 是命令语法示例，不声称该目录或记录真实存在。实际调用必须使用 Viewer
返回或 `--list` 发现的精确 Stream ID；历史还必须在 `--snapshot`、`--archive`、
`--legacy-round` 中恰选一个。current 支持 `--since SNAPSHOT_UUID` 取得可选净变化；也支持
成对 `--task`／`--batch`。任何选择结果为零或多个都返回错误，没有隐式 latest。

成功的 current 输出包括完整 `feedback`、`assets`、`evidence`、`actionable`、`sourceChecks`、
`snapshotRef` 和可选 `delta`。调用方必须用这份完整 current 替换自己的旧待办，不能与之前
读取或 history 不断累加。空 current 不回退历史，也不表示全部通过；`actionable` 只是该次
source check 下可执行的目标集合，执行前仍要核对目标素材版本。

保存确认到公开证据生成完成之间，authoring head 会暂时领先 published head。`current` 和
`--list` 在同一个只读事务中固定这两个 head；只要它们不相等，就只返回 typed error
`publication_pending`，不附带旧 `feedback`、`actionable` 或任何“最新”成功载荷。Agent 集成应
使用有上限的退避重新读取；不得退回缓存的 latest、旧 current、mtime 或目录顺序来猜测新意见。
明确指定的已发布 history 在此期间仍可读取，因为它不会被解释成当前待办。

history 输出原始自然语言、精确目标/revision、选择器和可用证据，但没有 `actionable`。它仅供
背景、继续提出或显式恢复；存档不证明 Agent 已读、已执行、已修复或用户已验收。撤回记录只
说明要求从 current 移除，不是要求 Agent 执行一条反向自然语言。

每次读取固定一版 index。并发保存不会让一次调用混合新旧 head；delta 只描述已提交净变化，
不携带旧原文，无法证明起点或超过 10,000 节点时返回 unavailable，完整 current 仍可使用。
CLI 成功 JSON 写 stdout；typed error JSON 写 stderr 并 exit 1。库函数接受独立第二参数
`{ signal: AbortSignal }`；取消结束本次子进程读取，不返回部分成功，也不触发项目写入。

所有索引、state、archive、usage、legacy 与 evidence 位置都经描述符边界、普通文件身份、
大小、摘要、媒体和引用关系检查。成功结果不返回绝对素材／缓存路径。摘要证明内容完整性，
不认证是谁写下意见或谁执行了返工。读取器永远不自动执行意见、不修改项目。

### Legacy v1/v2 读取兼容

[`read-latest.mjs`](../../scripts/review-protocol/read-latest.mjs) 仅服务尚未迁移的 v1/v2
Completed 数据。它不会读取 v3/v4 current；持续评审入口遇到旧协议也要求显式迁移。旧记录按其原协议
验证并保持字节不变，不能被时间戳选择、降级重写或自动复活为当前要求。
