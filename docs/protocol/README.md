# Viewer 开放评审协议

## 状态与范围

本文档是 **Active 的协议与集成边界**。Viewer 当前提供手动评审轮次、图片矩形／画笔标注、
自然语言意见、Draft 恢复及 Completed 发布，并使用本协议保存结果；Production Manifest
执行、返工版本关系和具体 Agent 集成仍属于未来阶段。

Viewer 目前仍处于开发初期。本协议基础工作不产生签名、公证、正式安装包、发售或销售任务；后续普通功能开发也不应默认列出这些发布阶段事项，除非项目明确进入相应阶段并由用户另行授权。

## 所有权边界

- 生产工具或 AI Agent 拥有项目中的 `viewer-production.json`，使用 `viewer.production/1` 描述一次生产任务及其素材清单。
- Viewer 独占写入 `.viewer/reviews/`，新建轮次使用 `viewer.review/2`；已有
  `viewer.review/1` 记录保持可读，不为迁移而重写历史 Completed 文件。
- 当前手动入口由 Viewer 自身根据用户明确选择或当前浏览范围固定素材，写入一个无
  Production Scope 的手动 Review Stream；它不会读取或执行 `viewer-production.json`。
- Agent、自动化脚本和第三方集成只能把已被 `index.json` 收录的 Completed Round 当作返工指令来源。
- `.viewer/reviews/drafts/` 中的数据用于 Viewer 自身恢复编辑状态，**永远不是有效的 Agent 指令**，即使 Draft 的时间或 Round ID 更新。

```text
project/
├── viewer-production.json                 # Producer / Agent owns
└── .viewer/reviews/                       # Viewer owns
    ├── index.json                         # Per-Stream completed history
    ├── write.lock                         # One-writer advisory lease
    ├── drafts/{reviewRoundId}.json         # Recoverable, never instructions
    └── rounds/
        ├── {legacyRoundId}.json           # Immutable v1 Completed record
        └── {reviewRoundId}/               # Immutable v2 Completed bundle
            ├── round.json                # Feedback, anchors, outcomes, artifact records
            └── artifacts/
                └── {assetVersionId}-annotation.png
```

## 评审语义

每个 Round 冻结一组可评审的素材版本。用户主要以自然语言记录需要返工的例外；显式完成 Round 时，没有反馈且能够评审的素材得到 `pass`，有反馈的素材得到 `revise`，无法渲染或读取的素材得到 `unreviewable`。自然语言 Feedback 是返工意图的权威表达；结构化 Target 和 Anchor 只负责把文字关联到素材、图片区域或视频时间位置。

Completed Round 的 Outcome 只有三种：

- `pass`：本轮没有要求返工；
- `revise`：引用一条或多条自然语言 Feedback；
- `unreviewable`：素材无法完成审查，并记录稳定失败类别。

Draft 不包含 Outcome，也不能推进 Completed head。Completed Round 只创建一次。v2 发布先在
临时目录中写入并核验 `round.json` 与标注预览，持久化后提交整个 bundle，再原子更新
`index.json`。索引未提交前，Agent 不得将 bundle 或 Draft 当作返工指令。中断恢复只接受
`previousCompletedRoundId` 构成的唯一线性链，遇到分叉或无法连接的孤儿记录时停止并要求显式恢复。

v2 图片局部意见使用 `imageRect` 或 `imageStroke`，坐标相对经过方向校正后的完整图片归一化到
`[0, 1]`。矩形必须具有正宽高且不越界；画笔为 2–2048 个有效点，横纵范围均须非零。
自然语言文字原样保留。每张带局部意见的图片只生成一张 PNG 标注预览，原图不被修改；
`artifacts[].annotations` 用从 1 开始的连续编号关联 `feedbackId`。编号按意见创建时间、再按
Feedback ID 稳定排序。预览用于辅助定位，文字和结构化 Anchor 仍是评审事实。
一个 Round 的全部画笔点合计不得超过 200,000，避免大量局部意见绕过单个画笔上限。

当前手动工作流主要记录需要返工的例外。批量网格曝光和打开图片／视频预览具有同等资格；
协议不记录浏览次数、停留时间、滚动位置或播放行为。完成前如果固定素材被替换、移动、删除
或内容身份改变，Viewer 会阻止发布，避免把未经核验的新内容默认为通过。稳定、已确认的技术
失败可以成为 `unreviewable`；尚未完成探测或普通缩略图失败不能被静默归入该结果。

手动素材可以携带可选 `sourceEntityId`，用于把固定版本关联到 Viewer 已验证的本地实体身份。
旧版 v1 文档没有该字段时仍可读取。可评审图片继续保存完整宽高；仅在图片已确认不可评审时，
宽高可以成对省略，禁止只提供其中一个值。

## Review Stream 选择

“最新完成结果”只存在于一个 Review Stream 内，协议没有、也不得推导项目级全局 latest。读取方必须使用以下一种方式选择恰好一个 Stream：

- 精确 `reviewStreamId`；
- 精确的 `taskId` 与 `batchId` 组合；
- 仅当项目中恰好有一个 Stream 时省略选择器。

选择结果为零或多个时，读取方必须报错，不能根据文件时间、Round ID、Draft 或目录顺序猜测。

Viewer 当前在一个项目中维护至多一个无 Production Scope 的手动 Stream，后续手动 Round
追加到该 Stream。带 `taskId`／`batchId` 的 Production Stream 仍由未来阶段生产，不能被当前
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

v1 索引保留 `completedRoundIds`。v2 索引以 `completedRounds` 保存每个历史 Round 的
`reviewRoundId`、`protocolVersion`、相对 `location` 和文件字节的 `blake3`，因此同一 Stream
可以包含 v1 文件与 v2 bundle。索引的主版本不等于所有历史 Round 的版本；读取方必须按
记录声明分派。v1 `imageRegion` 仅按 v1 规则读取，不得当作 v2 Anchor 静默改写。

## Agent 无关参考读取器

[`scripts/review-protocol/read-latest.mjs`](../../scripts/review-protocol/read-latest.mjs) 只使用 Node.js 内置模块，不依赖 Codex、Claude、OpenCode 或其他特定 Agent。它不联网、不启动 Viewer、不启动 Agent、不写项目文件，也不枚举 Draft 作为结果。

```bash
node scripts/review-protocol/read-latest.mjs \
  --project /absolute/project \
  --stream 00000000-0000-4000-8000-000000000102

node scripts/review-protocol/read-latest.mjs \
  --project /absolute/project \
  --task task-b \
  --batch batch-b

node scripts/review-protocol/read-latest.mjs \
  --project /absolute/project \
  --list
```

读取器先读取固定的 `.viewer/reviews/index.json`，再读取所选 Stream 的 Completed head。
对于 v2 索引，它在解析 Round JSON **之前**核验索引声明的 BLAKE3；这同样适用于索引中的
v1 历史记录。旧 v1 索引没有摘要字段，继续按原契约验证身份和内容。

所有 Round／artifact 位置逐级检查，拒绝越界路径、符号链接和非普通文件。v2 bundle 还须
具有完整且无额外文件的 artifact 清单；每个 PNG 均核验摘要、PNG 签名、宽高、媒体类型及
编号到意见的完整映射。索引最多 16 MiB，Round JSON 和单个 PNG 最多各 64 MiB，单个 PNG
最多 16,777,216 像素，bundle 中 PNG 总量最多 4 GiB。读取过程本身有字节与目录项上限，
不是读完以后才判断大小。

成功时输出原始自然语言意见、Anchor 和已验证的相对 artifact 路径，不返回绝对素材路径或
缓存路径；失败时写入 stderr 并以状态码 `1` 退出。摘要提供内容完整性校验，不代表对评审
意见来源的身份认证。读取器不自动执行意见、不修改项目，也不猜测尚未完成的结果。
