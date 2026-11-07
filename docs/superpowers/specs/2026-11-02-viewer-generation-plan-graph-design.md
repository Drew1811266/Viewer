# Viewer 生成计划与节点图设计（v2）

> Status: Draft for approval
>
> 日期：2026-11-02（v2 重写：2026-11-03）
>
> 版本标签：0.3.1（功能收口目标）；0.3.2 是随后的优化与修复轮次；开发期间代码版本保持 0.3.0
>
> 类型：新领域对象（生成计划）+ 反向协议（Agent → Viewer，经 MCP）+ 新只读工作台
>
> 约束：Viewer 不生成图片、不调用生图 API、不运行 Agent、不提供节点编辑器
>
> v1 已被本版取代。v1 的单层节点、`variants[].count`、文件直写模型，均不再适用。

## 1. 结论

Viewer 在评审**之前**补一段：Agent 把「用户要生成什么」写成一份分两层的计划，
Viewer 把它渲染成节点图（第一层）与素材抽屉（第二层）。用户只做一件事——**指**：
在节点上打标记，Agent 通过 MCP 读回标记继续迭代。是否开始生成由用户在对话里决定，
Viewer 不保存、也不推断任何批准状态。

**两层不是分组，是两种注意力：**

| | 第一层 · 需求 | 第二层 · 素材 |
| --- | --- | --- |
| 回答 | 我要什么、要几张 | 这张图具体怎么生成 |
| 内容 | 组、声明数量、意图描述、跨组规则 | 拼装后的提示词、参考图、规格、模型 |
| 谁读 | **人，全量读** | 机器读；人只抽样 |
| 规模 | 3–10 个组 | 50–500 张分镜 |
| 变化频率 | 低（改需求才动） | 高（换一句话、换模型都动） |

这张划分解决一个具体问题：**没有人能审 50 条生成请求，但所有人都能读 3 个组和几个数字。**
所以第一层必须小到能全量阅读，第二层必须大到能交给机器。

## 2. 这次重写改了什么

| 主题 | v1 | v2 | 原因 |
| --- | --- | --- | --- |
| 节点 | 单层：参考图 → 组 → 分镜 | 两层：需求（画布）+ 素材（抽屉） | 注意力分配 |
| 数量 | `variants[].count` 派生 | 第一层**声明**数量，系统核对实际 | 用户要"感知生成数量" |
| 共享内容 | 值携带片段与参考图 | 保留，并在第二层展示拼装来源 | 共性不复制 |
| 素材 | 计划里存路径 | **内容寻址素材库**，计划只存 assetId | 去重与可复现 |
| 写入 | Agent 直接写文件 | **MCP 工具**（校验后写入同一份文件） | 规则可发现、错误可自纠 |
| 版本 | 每次改动 +1 | 版本跟随**需求**；重试记在 run 里 | 避免重试淹没版本 |
| 图形 | 横向 | **自上而下**，画布只画第一层 | 规模与可读性 |

## 3. 领域模型

### 3.1 三种节点 + 一种材料

| 概念 | 层 | 说明 |
| --- | --- | --- |
| Plan | L1 根 | 标题、声明总量、规则 |
| Group | L1 | 用户口中的"一套/一类"，带声明数量与意图描述 |
| Shot | L1 叶子 / L2 主体 | 一张要生成的图；第一层是叶子，第二层是素材集合 |
| Value | 材料 | 维度取值，携带共享片段与共享参考图；不是画布节点 |

画布：`Plan → Group → Shot`，自上而下，逐层下钻。
取值不出现在画布上，只出现在第二层的"这段文字/这张图从哪来"里。

### 3.2 关系与身份

- `Group` 与 `Shot` 共享一个全局命名空间，id 必须唯一。
- `dimensions[].id` / `values[].id` / `rules[].id` 各自独立命名空间。
- 每条边都有类型：`belongs`（shot → group）、`uses`（shot/group → value）、`references`（→ asset）。
- 重命名必须通过 `relocations` 声明；未声明的改名按"删除 + 新增"处理。

### 3.3 素材（asset）

参考图不是路径，是**内容**。所有参考图进入 `.viewer/refs/<sha256>.<ext>`，
计划里只写 `assetId`（即 sha256）。同一份字节在项目里永远只有一个文件。

## 4. 协议 `viewer.plan/1`

### 4.1 文件布局与所有权

```text
project/
└── .viewer/
    ├── plans/
    │   ├── incoming/<planId>/<revision>.json   # Agent 写（经 MCP），不可变
    │   ├── runs/<planId>/<runId>.json          # Agent 写（经 MCP），一次生成记录
    │   └── index.json                          # Viewer 写，派生缓存，可删可重建
    ├── plan-marks/<planId>/current.json        # Viewer 写，Agent 只读
    └── refs/
        ├── <sha256>.<ext>                      # 经 MCP 导入，不可变
        └── index.json                          # MCP 写，追加式
```

**为什么放 `.viewer/refs/` 而不是项目根 `refs/`**：参考资料是"打包进工程文件的资源"
（类似 .blend 内嵌贴图），不是用户的产出素材。放外面会让它混进"全部素材"、搜索和评审网格。
用户要看参考图，去计划里看——那里本来就有缩略图。

### 4.2 文档结构

```json
{
  "protocolVersion": "viewer.plan/1",
  "planId": "autumn-hero",
  "revision": 4,
  "title": "秋季产品图批量生成",
  "createdAt": "2026-11-03T09:00:00Z",
  "producer": { "name": "deepseek-harness", "sessionId": "…" },

  "dimensions": [
    { "id": "series", "label": "套系", "values": [
        { "id": "summer", "label": "夏",
          "fragment": "明亮清爽，夏季自然光",
          "references": ["9f2c…"] } ] },
    { "id": "subject", "label": "人物", "values": [
        { "id": "boy", "label": "男孩", "fragment": "10 岁男孩，短发" } ] }
  ],

  "rules": [ { "id": "bg", "text": "所有图片纯白背景，不出现桌面与手部" } ],

  "groups": [
    { "id": "g_summer", "label": "夏", "attributes": { "series": "summer" },
      "declaredCount": 7, "brief": "轻薄夏装，户外自然光" }
  ],

  "shots": [
    { "id": "s01", "groupId": "g_summer",
      "attributes": { "subject": "boy" },
      "delta": "海边逆光，跑动",
      "references": [ { "assetId": "3ab7…", "role": "style" } ],
      "prompt": null,
      "spec": { "aspect": "4:3", "size": "1600x1200", "seed": null },
      "model": { "id": "flux-dev", "version": "1.2", "params": {} } }
  ],

  "relocations": [ { "kind": "shot", "from": "s1", "to": "s01" } ]
}
```

### 4.3 第一层字段

| 字段 | 必填 | 约束 |
| --- | --- | --- |
| `planId` | 是 | `^[A-Za-z0-9][A-Za-z0-9._-]*$`，≤ 64，项目内唯一 |
| `revision` | 是 | ≥ 1，必须与文件名一致 |
| `dimensions[]` | 否 | ≤ 20；每维取值 ≤ 50；id 稳定不可改 |
| `groups[]` | 是 | 1–50；`declaredCount` 0–500；`brief` ≤ 1000 字 |
| `groups[].attributes` | 否 | 必须引用已声明的维度取值 |
| `rules[]` | 否 | ≤ 50；`text` ≤ 2000 |

**数量规则**：`declaredCount` 是 Agent 对用户口述数量的**声明**。系统统计该组实际分镜数，
两者并列显示。不一致产生 `count_mismatch` 警告，界面顶部常驻提示。

这是唯一能抓住"用户说 7 张、Agent 写了 6 条"的地方，所以数量必须被声明一次，
不能纯靠数出来。

### 4.4 第二层字段

| 字段 | 必填 | 约束 |
| --- | --- | --- |
| `shots[]` | 是 | 1–500；id 与 group 共享命名空间 |
| `shots[].groupId` | 是 | 必须存在 |
| `shots[].attributes` | 否 | 引用已声明取值；与所属 group 的 attributes 合并 |
| `shots[].delta` | 否 | ≤ 2000 字，本张独有描述 |
| `shots[].prompt` | 否 | 非空时**整条覆盖**，跳过拼装（逃生口） |
| `shots[].references[]` | 否 | ≤ 20；`assetId` 必须存在于 refs 索引；`role` ∈ style / subject / composition / other |
| `shots[].spec` | 否 | `aspect` / `size` / `seed`，边界校验 |
| `shots[].model` | 否 | `id` + `version` + 自由 `params`（≤ 32 键） |

### 4.5 拼装规则（不落盘）

分镜最终提示词按顺序拼接：

1. 所属 group 的 `brief`
2. 各维度取值 `fragment`（按 `dimensions` 声明顺序，跳过空值）
3. 本张的 `delta`

`shots[].prompt` 非空时，1–3 全部忽略。

参考图 = group 与分镜自身 `references` 合并、按 `assetId` 去重，保留 role 与来源。

界面必须显示**拼装结果**，并给每一段标出来源（组 / 哪个维度取值 / 本张）。
用户看到的不该是一个字符串，而是一段带出处的话。

### 4.6 校验与发布

校验是确定性纯函数，桌面应用、MCP 服务与读取器共用同一份实现
（位于 application/infrastructure，不在 `src-tauri`，也不在 `scripts/`）。

1. 版本：更高版本返回 `unsupported_version`，不得报"损坏"。
2. 严格反序列化：未知字段拒绝。
3. 结构：id 唯一性、引用完整性、数量上限、文件体积 ≤ 8 MiB。
4. 语义：`count_mismatch`（警告）、`asset_missing`（错误）、`unknown_value`（错误）、
   `relocation_conflict`（错误）。
5. 通过后写 `index.json` 并通知 UI。

**两处例外**：`index.json` 是派生缓存，可删可重建；读取器不依赖它、也不依赖
`.viewer/project.json`，自己扫描 `incoming/` 选出最高有效版本。

### 4.7 版本与尝试

- `revision` **只跟随需求变化**：组、数量、意图、维度取值、分镜内容。
- 重试（换模型、改采样、同一条需求再画一次）**不升版本**，记在 run 里。
- 每次写入都必须提交完整计划，不能只交增量——这是"静态可枚举"的代价与保证。

```json
// .viewer/plans/runs/<planId>/<runId>.json
{
  "protocolVersion": "viewer.plan.run/1",
  "runId": "run-20261103-1",
  "planId": "autumn-hero",
  "planRevision": 4,
  "startedAt": "…", "finishedAt": "…",
  "model": { "id": "flux-dev", "version": "1.2" },
  "outputs": [
    { "shotId": "s01", "shotHash": "…", "paths": ["out/s01.png"], "status": "ok" }
  ]
}
```

`shotHash` 是拼装后提示词与素材集合的 blake3。**hash 没变的分镜不必重新生成**——
这是"只重算受影响部分"的跨工具契约。

## 5. 素材库

### 5.1 导入

只有一条路径：MCP 的 `assets.import`。接受本地路径（推荐）或 base64（有上限）。

- 计算 sha256 → 目标 `.viewer/refs/<sha256>.<ext>`
- 已存在则**不复制**，直接返回既有 assetId
- 校验：可解码、受支持格式、边长上限（与现有图片管线一致）
- 写 `refs/index.json`：`{ sha256: { name, ext, width, height, importedAt } }`

### 5.2 性质

- **不可变**：改一张参考图 = 导入一张新的。旧计划仍能复现。
- **去重是结构结果，不是规则**：同样的字节必然落到同一路径，Agent 想重复都做不到。
- **同图不同用途**：文件一份，role 两条。

## 6. MCP 通道

### 6.1 形态

**stdio 子进程，由 Agent 自己拉起；不开端口、不常驻、不监听。**

与现有 `viewer-review-reader` 同类：一次性进程、JSON-RPC over stdio、用完即退。
App 完全不知道 MCP 存在，照旧靠文件监听刷新界面。

这条边界是刻意的：一旦做成"App 内开服务端口"，就等于给一个声明"无网络、无本地 API"
的应用增加常驻监听面，与现有安全模型冲突。

### 6.2 工具

| 工具 | 作用 | 备注 |
| --- | --- | --- |
| `plan.rules` | 返回节点规则与 schema（也可作 resource 暴露） | Agent 写数据前先读 |
| `plan.validate` | 校验候选计划 | 返回 errors[] / warnings[]，不落盘 |
| `plan.write` | 校验通过后写入新 revision | 校验不过拒绝写入 |
| `plan.current` / `plan.list` | 读当前有效版本 / 列出全部计划 | 无桌面应用时也可用 |
| `marks.read` | 读回用户标记（含 revision、observedHash、状态） | 无批准字段 |
| `assets.import` | 导入参考图，返回 assetId | 去重入口 |
| `run.record` | 记录一次生成与产物 | 供评审与追溯 |

### 6.3 错误模型

结构化、可自纠：`invalid_schema` / `unknown_value` / `asset_missing` / `count_mismatch`（警告）/
`revision_conflict` / `unsupported_version` / `unsafe_path` / `too_large` / `io`。

每条错误带定位（`shots[3].references[1]`）与一句话原因。Agent 应读完错误后自己重试，
而不是把坏文件留在磁盘上等人发现。

### 6.4 与文件的关系

MCP 是**写入通道 + 校验器**，文件仍是唯一事实来源。删除 MCP 后手写 JSON 依然可用；
删除文件后 MCP 也读不到东西。

## 7. 标记

```json
{
  "protocolVersion": "viewer.plan-marks/1",
  "planId": "autumn-hero",
  "marks": [
    { "id": "mk_1", "revision": 4,
      "target": { "kind": "shot", "shotId": "s01" },
      "category": "wrong", "field": "references",
      "note": "这张参考图用错了", "observedHash": "…",
      "createdAt": "…" }
  ],
  "updatedAt": "…"
}
```

- `target.kind` ∈ `plan` | `group` | `shot` | `value` —— 与两层对应：
  需求层标 `plan`/`group`，素材层标 `shot`/`value`。
- 每条标记自带 `revision` 与 `observedHash`，判定不依赖任何旧 revision 文件。
- **不保留标记历史**：标记是工作态，不是证据。
- **没有批准字段。** 空标记集 ≠ 通过。

状态由当前计划与 `observedHash` 比较得出：

| 状态 | 判定 |
| --- | --- |
| 待处理 | 目标存在且内容未变 |
| 已改 | 目标存在，**参数**变化（片段、delta、数量声明） |
| 已换 | 目标存在，但**拓扑**变化（换了组、改名、重新绑定） |
| 已失效 | 目标不存在 |

"已改"与"已换"分开，因为参数变化说明"要求改了"，拓扑变化说明"这不是原来那张了"。
混为一谈会导致 Agent 一挪动节点，就把用户意见静默关掉。

## 8. 变更影响与脏状态

- 每个分镜计算 `shotHash`；每次 revision 与上一有效版本比较，得到受影响集合。
- diff 分层显示：**需求层变了什么 / 素材层变了什么**。
- 分镜状态是持久的，不是一次性通知：`未生成` / `已生成·一致` / `已生成·计划已变` / `已评审`。
- 界面常驻"还有 N 张是旧的"。

## 9. 界面

### 9.1 结构

```text
┌──────────────────────────────────────────────────────────────┐
│ 计划切换 ▾ · r4 · 3 组 / 18 张（声明 18 ✓）· 6 张待重做       │
├──────────────────────────────────────────────┬───────────────┤
│ 画布（第一层）                                │ 素材抽屉       │
│   Plan                                        │ 拼装后的提示词 │
│    ├─ Group 夏 · 7 张 ⚠声明 8                 │  段段带来源    │
│    │    ├─ Shot s01 旧                        │ 参考图 + 引用数│
│    │    └─ …                                  │ 规格 / 模型    │
│    ├─ Group 秋 · 5 张                         │ 标记           │
│    └─ Group 冬 · 6 张（3 未生成）             │               │
├──────────────────────────────────────────────┴───────────────┤
│ 本版改动：需求未变 / 素材改 18 条                             │
└──────────────────────────────────────────────────────────────┘
```

- **画布只画第一层**：3–10 个组，下钻才展开分镜。它永远小到能一眼读完。
- 节点状态用描边：粗=待重做、细=最新、灰=未生成。
- 声明数量与实际不一致时，组节点直接显示 `⚠ 声明 8 · 实际 7`。

### 9.2 用户能做的事（全部）

切换计划、平移缩放、下钻、选中节点、**打标记**、删除标记、看变更。没有编辑。

### 9.3 素材抽屉

显示拼装结果，每段带来源标签，点击跳到所属维度取值。参考图显示缩略图与
**被引用次数**（`× 12`）——一张图坏了，一眼看到它扇出到多少张。

## 10. 模块与边界

| 层 | 新增 |
| --- | --- |
| domain | `plan` 模块：Plan / Group / Shot / Value / AssetId / Mark / Relocation，校验错误 |
| application | `plan` 用例与端口；**唯一校验函数** |
| infrastructure | 计划读写、素材库（哈希与存储）、索引、run 记录；`viewer-plan-reader` 与 `viewer-plan-mcp` 两个二进制 |
| src-tauri | `commands/plan.rs`、DTO、`state/plan.rs`；`watcher_runtime` 按 `.viewer/plans|refs` 前缀分流 |
| ui | `api/viewer.ts` 增加计划桥接；`components/plan/*`（画布用 `@xyflow/react`，布局用 `@dagrejs/dagre`）；抽屉与状态 |

依赖方向不变。`src-tauri` 不做协议解析；`scripts/` 不实现校验。

## 11. 安全与产品边界

需要重写 `PRODUCT_SPEC` §3、§9 与 `docs/protocol/README.md`：

- 现文写的是"不提供 MCP、本地 API 或 Agent 插件"。本设计**新增了一个 MCP 服务**，
  但它是 **stdio 子进程**，不开端口、不常驻、不接收外部连接——性质与已有的
  `viewer-review-reader` 相同，不改变"无网络访问"的承诺。
- 新增约束：MCP 只能写 `.viewer/plans/`、`.viewer/plans/runs/`、`.viewer/refs/`；
  路径必须落在项目根内；单文件与总量都有上限；不执行任何生成命令。
- 需要一篇 ADR 记录这条边界，并说明为什么不是"本地 API 服务"。

## 12. 验收

1. Agent 通过 MCP `plan.write` 写入合法计划，2 秒内出现在界面；写入非法计划被拒绝，错误定位到字段。
2. 同一张参考图导入两次，`refs/` 只有一个文件，两次 `assets.import` 返回同一 assetId。
3. 计划写 `declaredCount: 8` 而实际 7 条分镜时，组节点显示 `⚠ 声明 8 · 实际 7`。
4. 打 3 个标记后 `marks.read` 返回 3 条，含 `revision`、`observedHash`、派生状态。
5. 只改第二层（换模型）重试后，版本号不变，run 记录新增一条。
6. 改一句共享片段，界面显示受影响的 12 张分镜；已生成的 5 张转为"计划已变"。
7. Agent 改名并声明 `relocations` 后，原有标记显示"目标已改名"，不变成"已失效"。
8. 不启动桌面应用、单独运行 MCP 服务，能完成 `plan.write` → `plan.current` → `marks.read`。
9. 界面不存在任何"确认/批准"控件；读取器输出中没有批准字段。
10. `pnpm verify:clean` 全绿；新增文档在 `docs/README.md` 登记。

## 13. 文档落地

| 产物 | 位置 |
| --- | --- |
| 本设计 | `docs/superpowers/specs/2026-11-02-viewer-generation-plan-graph-design.md` |
| 研究依据 | `docs/quality/2026-11-02-procedural-graph-and-usd-research.md` |
| ADR 0012 | 生成计划协议、素材库与 MCP 边界 |
| 协议文档与 schema | `docs/protocol/plan-protocol.md`、`viewer-plan-v1`、`viewer-plan-marks-v1`、`viewer-plan-run-v1`、`viewer-plan-read-result-v1` |
| 产品边界改写 | `PRODUCT_SPEC` §3 §9；新增 `docs/product/0.3.1-DELTA.md` |
| 目标需求 | `docs/milestones/viewer-0.3.1-target-requirements.md` |
| 验收 | 不进入 147 状态视觉目录；组件测试 + 手工真机走查 |

**实施顺序**：schema 与校验器 → 素材库 → 读取器 → MCP → 只读画布 → 标记与状态 → 变更影响 → 抽屉。

**0.3.1 收口定义（功能完成）**：本设计的两部分（无界面核心 + 画布与交互）全部落地，且：

1. `pnpm verify:clean` 与 `pnpm security` 全绿，工作树零漂移；
2. 设计 §12 的十条验收逐条有证据；
3. 版本号在收口时统一提升为 `0.3.1`——`Cargo.toml` 的 `[workspace.package].version`、
   `src-tauri/tauri.conf.json` 的 `version`、`ui/package.json` 的 `version` 三处同步，
   并同步 `scripts/repository-policy.test.mjs` 中的版本断言（当前硬编码 `0.3.0`）；
4. 补 `CHANGELOG` 的 `0.3.1` 条目与 `docs/product/0.3.1-DELTA.md`；
5. 开发期间代码版本保持 `0.3.0`，不在实现过程中提前改号。

**0.3.2（优化与修复轮次）**：0.3.1 功能收口之后的独立轮次，处理性能、可用性反馈与缺陷修复。
其范围在 0.3.1 收口后单独定义，本设计不预先承诺内容。

**图渲染选型**：画布采用 `@xyflow/react` 12.12.1（MIT）+ `@dagrejs/dagre` 3.1.3（MIT）。
视口、节点容器、选中与增量更新由库负责；布局参数、节点组件、确定性与可访问性补偿由我们控制。
许可证、集成义务与风险见[选型记录](../quality/2026-11-03-node-graph-rendering-research.md)。

## 14. 留给实施者

- 画布的间距、连线路由与下钻动效。
- 抽屉宽度与折叠策略。
- 大计划（>50 组 / >500 分镜）时的聚合呈现。
- `refs/index.json` 的修复策略（文件被外部移动后如何重建）。
- 是否需要 `plan.list` 之外的批量操作。

## 15. 未决

1. 生成产物落在项目哪个目录：沿用用户目录，还是固定 `out/<planId>/<shotId>/`？
2. `run.record` 是每条分镜一次，还是整批一次？（影响断点续跑）
3. 素材库是否需要"删除未引用素材"的维护入口。
