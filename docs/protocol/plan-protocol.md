# 生成计划协议 `viewer.plan/1`

> Status: Active（0.3.1 开发中）
>
> 决策记录：[ADR 0012](../adr/0012-generation-plan-protocol-and-mcp-boundary.md)
>
> 设计依据：[生成计划与节点图设计](../superpowers/specs/2026-11-02-viewer-generation-plan-graph-design.md)

本文是 Agent 与第三方工具的集成契约。Viewer 只读地显示计划；用户只打标记，
不编辑计划。是否开始生成由用户在对话里决定，Viewer 不保存批准状态。

## 1. 存储布局

```text
.viewer/
├── plans/
│   ├── incoming/<planId>/<revision>.json   # 生产者写，不可变
│   ├── runs/<planId>/<runId>.json          # 生产者写，一次生成记录
│   └── index.json                          # Viewer 写，派生缓存
├── plan-marks/<planId>/current.json        # Viewer 写，生产者只读
└── refs/<sha256>.<ext>                     # 经 MCP 导入，不可变
```

`index.json` 是缓存，删除可重建。读取器不依赖它，也不依赖 `.viewer/project.json`。

## 2. 两层结构

| 层 | 内容 | 读者 |
| --- | --- | --- |
| 第一层 | `groups[]`、`declaredCount`、`brief`、`rules[]` | 人，全量读 |
| 第二层 | `shots[]`、`dimensions[].values[]`、`references`、`spec`、`model` | 机器读，人抽样 |

`groups[].id` 与 `shots[].id` 共享一个命名空间，必须唯一。
`dimensions[].id`、`values[].id`、`rules[].id` 各自独立。

## 3. 数量语义

`declaredCount` 是声明值；实际分镜数由程序统计。不一致时返回 `count_mismatch`
**警告**，计划仍可发布，界面常驻提示。

## 4. 拼装规则

```text
prompt = group.brief
       + Σ dimension.value.fragment（按 dimensions 声明顺序，跳过空值）
       + shot.delta
```

`shot.prompt` 非空时整条覆盖，忽略上述三段。

参考图 = group 与 shot 的引用合并，按 `assetId` 去重；每张保留 `role` 与来源。
拼装结果**不落盘**，由读取器与界面按需计算。

## 5. 校验与错误

| 代码 | 含义 | 严重度 |
| --- | --- | --- |
| `unsupported_version` | 协议版本高于本机支持 | 错误 |
| `invalid_schema` | 未知字段、类型错、超限 | 错误 |
| `duplicate_id` | id 在同一命名空间重复 | 错误 |
| `unknown_group` | 分镜引用了不存在的组 | 错误 |
| `unknown_value` | 属性引用了未声明的维度取值 | 错误 |
| `asset_missing` | 参考图不在 `.viewer/refs/` | 错误 |
| `relocation_conflict` | 同一来源被重定位两次或目标相同 | 错误 |
| `count_mismatch` | 声明数量与实际不符 | **警告** |

每条错误带路径，例如 `shots[3].references[1]`。

## 6. 版本与尝试

- `revision` 从 1 递增，文件不可变；同名文件已存在时写入被拒绝（`revision_conflict`）。
- 只改素材（换模型、改采样）不升 revision；重试写 `runs/`。
- `shotHash` 是拼装后提示词与素材集合的 blake3；hash 未变的分镜不必重新生成。

## 7. MCP 服务

`viewer-plan-mcp` 是 stdio 子进程：一行一个 JSON-RPC 2.0 消息，由 Agent 拉起，用完即退。

| 工具 | 参数 | 返回 |
| --- | --- | --- |
| `plan_rules` | — | 节点规则与上限 |
| `plan_validate` | `projectRoot`, `plan` | `{ok, errors[], warnings[]}` |
| `plan_write` | `projectRoot`, `plan` | 写入新 revision；有错误时 `isError` |
| `plan_current` | `projectRoot`, `planId` | 当前有效 revision + 拼装结果 + hash |
| `plan_list` | `projectRoot` | 每个计划的有效 revision |
| `marks_read` | `projectRoot`, `planId` | 用户标记（无标记时为 `null`） |
| `assets_import` | `projectRoot`, `path` | `{assetId, deduped}` |

约束：不监听端口、不常驻、只能写 `.viewer/plans/`、`.viewer/plans/runs/`、`.viewer/refs/`。

## 8. 标记

```json
{
  "protocolVersion": "viewer.plan-marks/1",
  "planId": "autumn-hero",
  "marks": [
    { "id": "mk_1", "revision": 4,
      "target": { "kind": "shot", "shotId": "s01" },
      "category": "wrong", "field": "references",
      "note": "这张参考图用错了", "observedHash": "…", "createdAt": "…" }
  ],
  "updatedAt": "…"
}
```

**没有批准字段。** 空标记集不等于通过；Agent 不得把"没有标记"解释为"用户认可"。

标记状态由当前计划与 `observedHash` 比较得出：待处理 / 已改（参数变化）/
已换（拓扑变化）/ 已失效。

## 9. 读取器 CLI

```bash
node scripts/plan-protocol/read-plan.mjs  --project <root> [--plan <planId>]
node scripts/plan-protocol/read-marks.mjs --project <root> --plan <planId>
```

成功向 stdout 写一行 JSON，失败写 stderr 并返回非零退出码。
需要先构建：`pnpm build:plan-reader`（或 `pnpm build:plan-mcp`）。
