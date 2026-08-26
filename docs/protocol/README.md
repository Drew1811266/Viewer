# Viewer 开放评审协议

## 状态与范围

本文档是 **Active 的协议与集成边界**。Viewer 0.1.6 当前已经提供手动创建、恢复和完成评审
轮次的界面，并使用本协议保存结果；Production Manifest 执行、返工版本关系和具体 Agent
集成仍属于未来阶段。

Viewer 目前仍处于开发初期。本协议基础工作不产生签名、公证、正式安装包、发售或销售任务；后续普通功能开发也不应默认列出这些发布阶段事项，除非项目明确进入相应阶段并由用户另行授权。

## 所有权边界

- 生产工具或 AI Agent 拥有项目中的 `viewer-production.json`，使用 `viewer.production/1` 描述一次生产任务及其素材清单。
- Viewer 独占写入 `.viewer/reviews/`，使用 `viewer.review/1` 保存目录索引、可恢复 Draft 和不可变 Completed Round。
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
    └── rounds/{reviewRoundId}.json         # Immutable Completed records
```

## 评审语义

每个 Round 冻结一组可评审的素材版本。用户主要以自然语言记录需要返工的例外；显式完成 Round 时，没有反馈且能够评审的素材得到 `pass`，有反馈的素材得到 `revise`，无法渲染或读取的素材得到 `unreviewable`。自然语言 Feedback 是返工意图的权威表达；结构化 Target 和 Anchor 只负责把文字关联到素材、图片区域或视频时间位置。

Completed Round 的 Outcome 只有三种：

- `pass`：本轮没有要求返工；
- `revise`：引用一条或多条自然语言 Feedback；
- `unreviewable`：素材无法完成审查，并记录稳定失败类别。

Draft 不包含 Outcome，也不能推进 Completed head。Completed Round 文件只创建一次；Viewer 先持久化 Round，再原子更新 `index.json`。中断恢复只接受 `previousCompletedRoundId` 构成的唯一线性链，遇到分叉或无法连接的孤儿记录时停止并要求显式恢复。

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

协议字符串的斜杠后数字是主版本。读取方遇到不支持的主版本时必须保持只读并返回 unsupported；禁止用旧版本重写、降级或“修复”较新版本的数据。四份 JSON Schema 是公开的 v1 形状约束：

- [`viewer-production-v1.schema.json`](viewer-production-v1.schema.json)
- [`viewer-review-index-v1.schema.json`](viewer-review-index-v1.schema.json)
- [`viewer-review-draft-v1.schema.json`](viewer-review-draft-v1.schema.json)
- [`viewer-review-round-v1.schema.json`](viewer-review-round-v1.schema.json)

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

读取器只打开固定的 `.viewer/reviews/index.json` 和所选 head 对应的 `rounds/{reviewRoundId}.json`。它拒绝符号链接、超出大小上限的文件、不支持的协议版本以及 Project、Stream 或 Round 身份不一致；成功时把已验证的 Completed JSON 输出到 stdout，失败时写入 stderr 并以状态码 `1` 退出。
