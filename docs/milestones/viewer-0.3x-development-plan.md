# Viewer 0.3.x 开发方案（0.3.0）

> Status: Draft for approval（2026-10-28，基线 `v0.2.5` / `main@e5ae15e`）
>
> 需求依据：[viewer-0.3x-target-requirements.md](viewer-0.3x-target-requirements.md)
>
> 视觉依据：[0.3.0 视觉规格](../design/0.3.0/README.md)（23 屏浅色 M3 稿）。
>
> 本文把 0.3.0 的三条主线（评审工具、离线语音、Material Design 3）展开为可执行任务。
> 任务编号 `T-03A/B/C/D-xx`；每个任务给出代码落点、测试与完成定义。所有落点均已对照
> v0.2.5 源码核实（见 §1）。
>
> 本文仍是**方案草案**：§2 的五个决策确认后再进入实现，避免在协议、模型和 UI 路线上返工。

## 0. 结论摘要

0.3.0 不是“再画几个工具 + 换皮”，而是三次基础能力升级：

1. **标注成为一等数据**：在现有 5 种几何 anchor 上增加持久化 `AnnotationStyle`，新增工具复用
   既有几何形状，避免协议膨胀；同时把工具栏、选择编辑、只读解释和输入体验一起重做。
2. **语音是本地适配器，不是网页 API**：应用层定义 `SpeechRecognizer` 端口，macOS 适配器用
   `cpal` + `sherpa-onnx`（回退 `whisper-rs`）在本地推理；模型随 `.app` 分发、校验和固定、默认不落音频。
3. **M3 是设计系统迁移，不是组件库替换**：用官方 M3 token（color/type/shape/elevation/state/
   motion）重建现有 15 个 UI primitives 与全部页面；不引入 MUI，也不把工作台塞进通用 Web 组件。

建议实施顺序：

```text
S0 决策 + 三个 spike（模型/协议/M3 视觉方向）
   ├─ S1 M3 token + 基础 primitives（先给后续 UI 提供地基）
   ├─ S2 标注协议 v5 + 渲染/工具注册表（数据先行）
   ├─ S3 标注 UX + 语音输入（可并行）
   ├─ S4 全站页面迁移（shell → 浏览 → 预览 → 评审 → 弹窗）
   └─ S5 真机验收、性能、可访问性、打包（0.3.0 门禁）
```

## 1. 已核实现状与改动面

| 环节 | 位置 | v0.2.5 现状 / 对 0.3.0 的约束 |
| --- | --- | --- |
| 工具注册表 | `ui/src/app/review/annotationToolRegistry.ts` | 只有 `point/arrow/brush/rectangle/ellipse` 5 个工具；`MarkupTool`、`shortcut`、`icon` 都是封闭联合类型 |
| 工具入口 | `ui/src/components/review/AnnotationToolMenu.tsx`、`AnnotationToolbar.tsx` | “标记”按钮打开 popover；`dirty` 或只读时整组工具禁用，提示只有一句“当前素材暂时不可修改标记” |
| 画布绘制 | `ui/src/components/review/AnnotationCanvas.tsx`、`annotationScene.ts` | 颜色固定红、线宽固定 2；点/箭头/画笔/矩形/椭圆直接 canvas 绘制，无样式数据 |
| 选择编辑 | `AnnotationCanvas.tsx`、`annotationHitTest.ts` | 浏览态可移动/缩放矩形、椭圆、箭头；画笔只能整体选中或删除；无多边形/文字编辑 |
| 反馈输入 | `InlineFeedbackEditor.tsx`、`ReviewInspector.tsx`、`ReviewFeedbackRail.tsx` | 三处重复 textarea；文字为空不能保存；每条标注后自动弹出并抢焦点；无语音 |
| 领域 anchor | `crates/viewer-domain/src/review/feedback.rs` | `FeedbackAnchor` 只有 `ImagePoint/Arrow/Stroke/Rect/Ellipse`；`FeedbackTarget` 没有样式字段 |
| 持续评审 wire | `crates/viewer-infrastructure/src/review/protocol/v3/wire/feedback_types.rs` | `Target` 与 `Anchor` 都是 `deny_unknown_fields`；当前写 v3/v4，新增字段必须提升协议版本 |
| 旧版评审 | `crates/viewer-infrastructure/src/review/protocol/v1.rs` | v1 只保留 `imageRect`，新工具无法无损回写；0.3.0 新工具必须限定在持续评审 |
| 评审协议入口 | `crates/viewer-infrastructure/src/review/continuous/reader/project.rs` | 通过 `viewer.review/3|4` 检测协议；v5 要加入检测、迁移与“需要更新版本”错误 |
| UI 基础 | `ui/src/styles/tokens.css`、`app.css`、`review.css`、`primitives.css` | 已有自定义 token 与 15 个 UI primitives，但仍是自有视觉；CSS 约 6.6k 行，需分层迁移 |
| 视觉验收 | `ui/src/acceptance/acceptanceStateCatalog.json` | 141 个状态：workspace 42、viewing 28、review 37、feedback 15、video 12、dialog 7 |
| 语音能力 | 全仓库 | 没有音频采集、模型、麦克风权限或 `NSMicrophoneUsageDescription` |
| 打包 | `src-tauri/tauri.conf.json` | macOS 13+；已有视频 runtime 资源 staging 模式，可复用为模型 staging；CSP 当前只允许本机 |
| 质量门禁 | `package.json` | `pnpm verify:clean`、`architecture:boundaries`、`gate:m1/m2/m3`、`verify:perf`、review protocol 测试 |

## 2. 需要先确认的决策

| 编号 | 决策 | 推荐 | 影响 |
| --- | --- | --- | --- |
| D1 | 语音模型内置还是首次下载 | **内置**默认模型，断网即可用；不做下载器 | 打包体积、构建流程、隐私承诺 |
| D2 | 默认模型 | **S0 四模型 bake-off**；推荐 `Qwen3-ASR-0.6B` int8 + sherpa-onnx（约 937 MB），未达标回退 Whisper turbo（547 MB）或 small（181 MB）。详见 [ASR 调研](../quality/2026-10-28-asr-model-research.md) | 体积、内存、延迟、准确率 |
| D3 | UI 实现 | **自建 M3 token + primitives**，不引入 MUI / Material Web | 迁移成本、视觉一致性、依赖治理 |
| D4 | 主题范围 | **只做浅色**；深色留到后续版本；画布周围中性 | 设计/验收工作量减少，深浅双主题不进入 0.3.0 |
| D5 | 新工具范围 | 文字、直线、荧光笔、多边形、标尺；聚焦遮罩、模糊/马赛克延后 | 协议/交互/验收复杂度 |

若 D1 改为下载，需新增下载器、断点续传、校验、代理与离线错误处理，0.3.0 至少增加 2–3 周；
若 D2 选 Qwen3-ASR-0.6B，安装包比 small 增加约 750 MB；若选 turbo，比 small 增加约 366 MB。
S0 必须先测安装包、冷启动、峰值内存与 10s p95，再决定是否值得。

## 3. 总体设计

### 3.1 标注：复用几何，扩展样式

不为每个新工具新增 anchor 变体，而是把“工具语义 + 外观”放进 `AnnotationStyle`：

| 工具 | 复用的 anchor | `style.shape` | 交互 |
| --- | --- | --- | --- |
| 点 | `ImagePoint` | `pin` | 单击；显示序号 |
| 箭头 | `ImageArrow` | `arrow` | 拖拽；调两端 |
| 直线 | `ImageArrow` | `line` | 拖拽；不画箭头头部 |
| 画笔 | `ImageStroke` | `brush` | 自由笔画 |
| 荧光笔 | `ImageStroke` | `highlighter` | 宽笔触、半透明、multiply |
| 矩形 | `ImageRect` | `rectangle` | 拖拽；四角缩放 |
| 椭圆 | `ImageEllipse` | `ellipse` | 拖拽；四角缩放 |
| 多边形 | `ImageStroke` | `polygon` | 逐点单击，双击/Enter 闭合；闭合填充 |
| 文字 | `ImagePoint` | `text` | 落点 + 输入；文字来自 `Feedback.text` |
| 标尺 | `ImageArrow` | `ruler` | 拖出测量线；显示 px 距离，并吸附水平/垂直 |

这样做的收益：

- 现有 `FeedbackAnchor`、命中测试、投影、归档证据链路全部复用；
- 只需给 `FeedbackTarget` 增加一个可选 `style`，协议从 v4 升到 v5；
- 老评审没有 style 时按 anchor 推导默认样式，不丢数据；
- 文字用 DOM overlay 渲染，保证可访问性与清晰度，不把文本画进 canvas。

### 3.2 语音：应用端口 + macOS 适配器

```text
React FeedbackComposer
  → Tauri commands (speech_start / speech_stop / speech_cancel / speech_status)
  → viewer-application::speech  (状态机、限制、错误、取消)
  → viewer-platform-macos::speech
       ├─ AudioCapture (cpal / AVFoundation 权限)
       ├─ Resampler (48k stereo → 16k mono)
       └─ AsrRecognizer (sherpa-onnx 或 whisper-rs + Metal/Accelerate)
  → 返回 transcript，不落盘
```

- 不在 `viewer-domain` 引入音频概念；语音只是反馈文本的一种输入方式。
- 模型路径由 `src-tauri` 组合根从 Tauri resource dir 解析，平台适配器只接收 `PathBuf`。
- 推理在 `spawn_blocking` 中执行；一次只允许一个会话；支持取消与超时。
- 默认模型 lazy load，首次使用加载；启动不增加明显开销。

### 3.3 M3：token 先行，primitives 其次，页面最后

- 用官方 `@material/material-color-utilities`（Apache-2.0，仅构建期）从种子色生成
  **light-only** 调色板，输出静态 CSS；运行时不依赖 JS 配色库。深色不进入 0.3.0。
- 建立 `ui/src/styles/m3/tokens.css`、`typescale.css`、`shape.css`、`motion.css`、`state.css`；
  现有 `--viewer-*` 作为迁移期别名，最终删除。
- 重建 `ui/src/components/ui/*` 为 M3 primitives，保留现有对外 props/测试，减少页面改动。
- 字体：Latin 用自托管 Roboto Variable（Apache-2.0），CJK 回落系统 PingFang SC；
  图标用 Material Symbols Rounded 子集（Apache-2.0），替换当前 Lucide 映射。
- 桌面密度：基于 M3 density -1/-2 调整，不把移动端 48dp 全部照搬。

## 4. 工作流 A：评审工具

### 4.1 领域与协议

**T-03A-01 — ADR-0009：标注样式与协议 v5**

- 新增 `docs/adr/0009-annotation-style-and-review-protocol-v5.md`，记录：
  - 为什么复用 anchor、只加 style；
  - v5 的兼容承诺：0.3 读 v3/v4，写 v5；0.2.5 不能读 v5；
  - 新工具限定持续评审，旧版评审只读；
  - 样式参与 revision/hash/归档。
- 更新 `docs/README.md` 的 ADR 索引（ADR 入库时）。
- 完成定义：ADR 评审通过；协议字段表与迁移矩阵固定。

**T-03A-02 — `AnnotationStyle` 领域类型**

- 落点：`crates/viewer-domain/src/review/style.rs`（新）、`review/mod.rs` 导出。
- 类型：

```rust
pub enum AnnotationShape {
    Pin, Arrow, Line, Brush, Highlighter, Rectangle, Ellipse, Polygon, Text, Ruler,
    Spotlight, // P1 预留；0.3.0 不创建
}
pub enum AnnotationColor { Red, Orange, Amber, Green, Blue, Violet, Ink, Paper }
pub enum AnnotationLineStyle { Solid, Dashed }
pub enum TextSize { Small, Medium, Large }
pub struct AnnotationStyle {
    shape: AnnotationShape,
    color: AnnotationColor,
    width: u8,          // 1..=32 CSS px
    opacity: f32,       // 0.2..=1.0
    line_style: AnnotationLineStyle,
    text_size: Option<TextSize>, // 仅 text 使用
}
```

- 校验：枚举合法、`width` 范围、`opacity` 非 NaN 且在范围、`text_size` 只允许 text；
  `Default` 按 anchor 推导（点=pin 红 2px，箭头=arrow 红 2px，笔画=brush 红 2px……）。
- 测试：`annotation_style.rs` 边界/非法值/默认推导；属性测试保证 serde 往返稳定。
- 完成定义：domain 单测绿；无 UI/协议依赖。

**T-03A-03 — `FeedbackTarget` 增加 style**

- 落点：`crates/viewer-domain/src/review/feedback.rs`、所有构造点。
- `FeedbackTarget { asset_version_id, anchor, style: Option<AnnotationStyle> }`；
  `#[serde(default)]` 兼容旧 JSON；图片目标写入时应有 `Some`，视频/Asset 目标保持 `None`。
- 构造时校验 `style.shape` 与 `anchor` 兼容：point→pin/text，arrow→arrow/line/ruler，
  stroke→brush/highlighter/polygon，rect→rectangle，ellipse→ellipse；不兼容的值返回错误。
- 重复目标检测按 `(asset_version_id, anchor)`，忽略 style；同一位置不同颜色仍算重复。
- 测试：旧 target 反序列化为 `None`，渲染时按 anchor 推导默认；新 target 往返；
  多 target 各自样式；shape/anchor 不兼容拒绝；同 anchor 不同 style 判重。
- 完成定义：domain + application 编译；现有 review round 测试不回归。

**T-03A-04 — 协议 v5 wire**

- 落点：`crates/viewer-infrastructure/src/review/protocol/v5/`（新）、`protocol/mod.rs`、
  `detect_continuous_review_protocol`、`ReviewPublicationProtocol`。
- `viewer.review/5`；`Target.style` 可选：写入时图片目标必填，视频/Asset 目标省略；
  `Anchor` 保持 v3/v4 的 8 种（不新增 anchor 变体）。
- 读取：v3/v4 正常解码，style 为 `None`，渲染时按 anchor 取默认；v5 解码 style；未知更高版本返回
  `NewerProtocol` 错误，不得报“文件损坏”。
- 写入：所有 0.3 mutation 写 v5；v4 文件首次修改时升级。
- 测试：
  - v3/v4 fixture 读 → 默认样式；
  - v5 roundtrip（每种 shape/color/width/opacity/line_style）；
  - v5 未知字段/越界拒绝；
  - `viewer.review/6` 返回“需要更新 Viewer”；
  - 归档 hash 包含 style，同内容不同样式不同 hash。
- 完成定义：`test:review-protocol` 全绿；v5 fixture 入库。

**T-03A-05 — 持续评审 delta/归档/投影**

- 落点：`crates/viewer-application/src/review_workspace/*`、
  `crates/viewer-infrastructure/src/review/continuous/*`、`review/bundle.rs`、`protocol/v3/history_result.rs`。
- style 进入 target revision、delta、archive、preview、recovery；所有穷举补 `shape`。
- 测试：修改样式产生新 revision；删除/恢复保留样式；归档预览显示正确样式；
  v4→v5 迁移后旧意见样式=默认。
- 完成定义：`cargo test --workspace`；review-loop E2E 绿。

**T-03A-06 — 旧版评审闸门**

- 落点：`ui/src/app/review/imageReviewWorkbenchAdapter.ts`、`reviewModel.ts`。
- 非 continuous 协议或 v1 项目：新工具/样式入口禁用，横幅说明“此评审为旧版，迁移后可编辑”，
  提供现有迁移 dialog；不允许静默降级。
- 测试：legacy adapter 的 readOnly 原因、CTA、无新工具菜单；迁移后可用。
- 完成定义：legacy 路径不在 UI 中暴露不可持久化的样式。

### 4.2 渲染与交互

**T-03A-07 — 场景渲染器支持样式**

- 落点：`annotationScene.ts`、`AnnotationCanvas.tsx`。
- `AnnotationSceneItem` 增加 `style`；`paintAnnotationScene` 按 item 取样式；默认选项仅作 fallback。
- 荧光笔：`globalCompositeOperation='multiply'` + alpha；线帽圆；暗图可读性验收。
- 多边形：闭合路径 + 填充（低透明度）；标尺：刻度、px 距离/水平/垂直标签；遮罩/聚光灯不进入 0.3.0。
- 文字/标尺标签用 DOM overlay，不画 canvas 文本；canvas 只画几何与样式。
- 测试：每种 shape 的几何投影、颜色/宽度、缩放后位置；200 条场景性能。
- 完成定义：组件测试 + 视觉验收截图；无 canvas 文本。

**T-03A-08 — 工具注册表扩展与快捷键**

- 落点：`annotationToolRegistry.ts`、`AnnotationToolMenu.test.tsx`、`SHORTCUTS.md`。
- 11 个入口（10 种标记 + 选择）；保留 `P/A/B/R/O`，新增 `L/H/G/T/M`；`V` 浏览；Space 临时平移。
- 注册表增加 `group`（基础/标注/强调/测量）、`defaultStyle`、`anchorKind`、`cursor`。
- 测试：顺序、快捷键唯一性、输入框内不触发、只读不触发、每工具默认样式。
- 完成定义：工具栏/菜单/快捷键测试全绿。

**T-03A-09 — 顶部标记菜单 + 样式选项**

- 落点：`AnnotationToolbar.tsx`、`AnnotationToolMenu.tsx`、`AnnotationStyleBar.tsx`（新）、`review.css`。
- 保留当前“顶部工具栏 + 标记菜单 + 320px 意见栏”；不新增左侧常驻工具轨。
- 样式条：8 色、3 档线宽、透明度（荧光笔）、文字字号（文字）、虚线（直线/箭头）；每种工具记住上次样式。
- 测试：11 个入口可点、可键盘、active 状态、快捷键、样式持久化到本次会话。
- 完成定义：键盘可完成所有工具切换；不需要先打开二级模态。

**T-03A-10 — 新工具交互**

- 落点：`AnnotationCanvas.tsx`、`annotationGeometry.ts`、`annotationHitTest.ts`。
- 文字：落点 → 打开 composer；保存后 DOM callout；可拖动/方向键微调。
- 直线/标尺：拖拽端点；直线无箭头；标尺显示 px 距离并吸附水平/垂直。
- 荧光笔：自由笔画宽笔触；抽稀沿用 `simplifyNormalizedStroke`。
- 多边形：逐点添加、双击/Enter 闭合、Esc 取消、Backspace 删除最后一点；至少 3 点；闭合后可拖动顶点。
- 测试：每种工具的 pointer 流程、取消、越界 clamp、最小尺寸、撤销草稿。
- 完成定义：每工具至少 1 个组件测试 + 1 个真机验收状态。

**T-03A-11 — 选择、移动、缩放与键盘微调**

- 落点：`AnnotationCanvas.tsx`、`annotationHitTest.ts`、`AnnotationCanvas.test.tsx`。
- 所有 shape 可选中；矩形/椭圆四角缩放；箭头/直线/标尺调端点；文字/点/画笔/多边形整体移动；
  方向键 1px，Shift+方向键 10px；Delete 删除；Esc 取消选择。
- 画笔/多边形命中测试用最近点距离；文字用 DOM 边界。
- 测试：命中、拖动、键盘、只读禁止编辑、保存后 revision。
- 完成定义：不再出现“画笔只能删不能动”。

**T-03A-12 — 草稿撤销/重做**

- 落点：`useImageReviewWorkbench.ts`、新增 `annotationHistory.ts`。
- 栈最多 50 步；记录创建/移动/缩放/样式/文字；`Cmd+Z` / `Cmd+Shift+Z`；
  保存后清空草稿栈；已保存删除沿用现有恢复入口。
- 测试：每类操作 undo/redo、连续操作、保存后栈清空、只读禁用。
- 完成定义：与需求 §6.1 的撤销口径一致；不承诺跨保存完整历史。

**T-03A-13 — 反馈输入统一 + 快速继续**

- 落点：新增 `FeedbackComposer.tsx`，替换 `InlineFeedbackEditor.tsx`、`ReviewInspector.tsx`、
  `ReviewFeedbackRail.tsx` 中的重复 textarea。
- 能力：多行、字数上限、`Cmd+Enter` 保存、`Esc` 取消、保存并继续、错误/保存中状态、
  自动聚焦但保留用户已输入内容、空文本禁用保存。
- 位置：靠近标注但不遮挡；窄画面改为底部 sheet；记住上次位置偏好（可选）。
- 测试：三处入口复用、键盘、保存中、错误、取消、保存并继续保留工具。
- 完成定义：只有一个 textarea 实现；现有测试迁移后全绿。

**T-03A-14 — 只读原因与出口**

- 落点：`reviewModel.ts`、`AnnotationToolbar.tsx`、`ImageReviewWorkspace.tsx`。
- 映射 `loading/saving/source_confirmation/outside_scope/write_unavailable/recovery_required/
  migration_required` 到用户文案 + 主行动（确认来源、调整范围、恢复、迁移、重试）。
- 工具栏禁用时 tooltip 说明；页面顶部持久横幅，不只灰色菜单。
- 测试：每种 reason 的文案/CTA/禁用状态；无裸禁用。
- 完成定义：截图评审时能一眼知道为什么不能画、点哪里解决。

**T-03A-15 — 评审视觉/交互验收**

- 在 `acceptanceStateCatalog.json` 增加每个工具 × 选中/未选中 × 只读、样式条、文字 callout、
  标尺、撤销提示；更新 `design-qa.md` 证据。
- 真机在标准测试项目完成 10 种标记工具（11 个入口）、200 条标注、缩放/平移/重开。
- 完成定义：验收目录与证据入库；P0 缺陷清零。

## 5. 工作流 B：离线语音

### 5.1 模型与依赖决策

候选对比（S0 实测后定稿）：

| 方案 | 体积 | 语言 | 许可证 | 结论 |
| --- | --- | --- | --- | --- |
| Qwen3-ASR-0.6B int8 | ~937 MB | 中/英/粤/多方言 | 权重 Apache-2.0；sherpa-onnx Apache-2.0 | **首选候选**（S0 实测） |
| Whisper large-v3-turbo q5_0 | ~547 MB | 多语，中文好 | 模型 MIT；运行时 MIT | 次选；精度/成熟度基线 |
| Whisper small q5_1 | ~181 MB | 多语，中英可用 | 模型 Apache-2.0；whisper-rs Unlicense | 体积下限；需处理许可证例外 |
| SenseVoiceSmall int8 | ~226 MB | 中/粤/英/日/韩 | 权重 FunASR Model License（自定义） | 速度备选；许可证未确认前不发布 |
| Paraformer-zh | ~230 MB | 中/英 | Apache-2.0 | 轻量回退 |
| Parakeet TDT | ~600 MB | 英语/欧洲语 | CC-BY-4.0 | 不支持中文，排除 |
| Apple Speech | 0 | 多语 | 闭源 | 不满足“开源模型”，排除 |

**T-03B-01 — ADR-0010 + 模型卡**

- `docs/adr/0010-offline-speech-recognition.md`、`docs/product/ASR_MODEL.md`。
- 记录模型来源、版本、SHA256、许可证、推理库、打包路径、体积/内存/延迟、隐私边界、
  中文 CER/英文 WER 实测方法与结果。
- 完成定义：许可证与再分发条款明确；模型 hash 固定；无“未审计模型”。

**T-03B-02 — 模型 staging 与打包**

- 新增 `scripts/asr/stage-model.sh`：按 S0 选定的 manifest 从固定 URL 下载/校验/复制到
  `target/viewer-asr-model/<model-id>/`；支持 `VIEWER_ASR_MODEL_DIR` 离线缓存。默认 manifest
  是占位，S0 后改成最终模型。
- `src-tauri/tauri.conf.json` `bundle.resources` 增加模型映射；`.gitignore` 排除二进制；
  构建前检查存在与 SHA256；DMG 体积记录。
- 许可证/notice：`THIRD_PARTY_NOTICES.md`、`docs/quality/dependency-exceptions.json`、
  npm/cargo license 检查。
- 测试：`scripts/asr/stage-model.test.mjs`（校验失败/缓存/缺文件）；打包后资源存在。
- 完成定义：干净机器按文档可构建；无模型时构建失败并给出明确命令。

### 5.2 应用与平台

**T-03B-03 — 应用端口与状态机**

- 落点：`crates/viewer-application/src/speech.rs`、`speech/`。
- 端口：`AudioCapture`（start/stop/cancel/levels）、`SpeechRecognizer`（transcribe(samples, language)）。
- 状态：`Idle/RequestingPermission/Recording/Transcribing/Failed`；单会话；取消 token；上限 180s。
- 错误：`PermissionDenied/NoInputDevice/TooShort/TooLong/ModelMissing/ModelCorrupt/InferenceFailed/Cancelled`。
- 测试：状态迁移、并发开始拒绝、取消、超时、空音频、错误映射。
- 完成定义：纯应用层测试，无 Tauri/macOS 依赖。

**T-03B-04 — macOS 音频采集与权限**

- 落点：`crates/viewer-platform-macos/src/speech/capture.rs`、`permission.rs`。
- `cpal` 输入流；48k/stereo/任意设备格式 → 16k mono f32；电平事件 10 Hz；停止时返回 buffer。
- `AVCaptureDevice` 权限查询/请求；`NSMicrophoneUsageDescription` 写入 Tauri Info.plist；
  TCC 拒绝深链系统设置。
- 测试：重采样单测（正弦/静音/削波）、buffer 上限、设备缺失错误；权限手验矩阵。
- 完成定义：断开/切换设备不崩溃；拒绝权限有明确错误；不写音频文件。

**T-03B-05 — ASR 适配器（sherpa-onnx / whisper-rs）**

- 落点：`crates/viewer-platform-macos/src/speech/recognizer.rs`、workspace 依赖
  `sherpa-onnx`（首选）或 `whisper-rs`（回退）；两者通过同一 `SpeechRecognizer` 端口接入。
- 模型 lazy load；Metal/Core ML 优先，CPU 回退；线程数可配；初始 prompt/热词注入评审领域词
  （sherpa 用 `hotwords`，whisper 用 `initial_prompt`）；语言 auto/zh/en；返回
  text/language/duration/latency。
- 模型完整性：大小 + SHA256（缓存校验结果，避免每次启动全量哈希）；损坏返回 `ModelCorrupt`。
- 测试：mock recognizer 走应用测试；真实模型测试由 `VIEWER_ASR_MODEL` 环境变量启用；
  截断/静音/超长/取消内存边界。
- 完成定义：10s 音频 p95 ≤3s（M1/16GB）；内存峰值记录；无网络调用。

**T-03B-06 — 分段、清理与隐私**

- 落点：`speech/chunking.rs`、`transcript.rs`。
- 180s 上限；按 30s 窗口 + 1s overlap 分段；去除首尾静音；合并重复词；保留中文标点；
  不自动改写用户原意；空结果返回 `TooShort` 或空文本提示。
- 隐私：音频只在内存；不写日志文本；错误日志只记录时长/模型 ID。
- 测试：分段边界、重叠去重、静音、纯噪声、取消后 buffer 丢弃。
- 完成定义：隐私检查清单通过；无磁盘残留测试。

### 5.3 IPC 与 UI

**T-03B-07 — Tauri 命令与事件**

- 落点：`src-tauri/src/commands/speech.rs`、`src-tauri/src/lib.rs` invoke handler、
  `ui/src/api/speech.ts`、`ui/src/api/viewer.test.ts`。
- 命令：`speech_status`、`speech_start`、`speech_stop`、`speech_cancel`；
  事件：`viewer://speech-state`（state/elapsedMs/level）、`viewer://speech-result`（可选）。
- 错误 DTO 与 UI 错误码一一对应；命令不返回音频。
- 测试：IPC 契约（成功/权限/模型缺失/取消/并发）；CSP 不需要新增外部 connect-src。
- 完成定义：接口文档 + 契约测试；网络边界测试仍绿。

**T-03B-08 — UI hook 与麦克风按钮**

- 落点：`ui/src/app/speech/useSpeechToText.ts`、`ui/src/components/speech/VoiceInputButton.tsx`、
  `FeedbackComposer.tsx`。
- 状态：idle/permission/recording/transcribing/error；显示时长、电平、停止/取消；
  转写完成插入光标处，保留选区/草稿；支持撤销本次插入。
- 无设备/拒绝/模型损坏的错误面板；重试；打开系统设置。
- 测试：各状态渲染、插入位置、取消不插入、错误重试、键盘可达、aria-live。
- 完成定义：在标准测试项目用中文口述 20 条、英文 10 条，人工确认可用。

**T-03B-09 — 语音设置与模型状态**

- 落点：`ui/src/components/settings/SpeechSettings.tsx`（新）、设置入口。
- 显示模型名/大小/语言/许可证/“本地运行”说明；模型缺失/损坏的修复指引；
  语言 auto/zh/en；不提供云端选项。
- 测试：状态展示、语言切换、隐私文案、损坏模型提示。
- 完成定义：用户能自己判断模型是否可用，不需要看日志。

**T-03B-10 — 语音验收与基准**

- 本地真实语料（不提交仓库）50 条中文 + 20 条英文；记录 CER/WER、p50/p95 延迟、内存峰值。
- 手验矩阵：首次权限允许/拒绝、无麦克风、切设备、静音、超长、取消、模型缺失/损坏。
- 完成定义：结果写入 `docs/quality/asr-baseline.md`；不达标则按 D2 回退 turbo/small。

## 6. 工作流 C：Material Design 3

### 6.1 设计基础

**T-03C-01 — ADR-0011 + 视觉方向**

- `docs/adr/0011-material-design-3-design-system.md`。
- 方向：**克制的 M3**——中性表面、单一品牌种子、图片优先；使用标准 M3 组件与 motion，
  不采用实验性 Expressive API；桌面密度 -1。
- 产出 23 张 1440×900 全界面视觉稿：项目入口、分类/网格/搜索、图片/文本/视频/其他预览、
  评审（默认/选中/文字语音/权限/只读）、对比、存档/历史/迁移、设置、批量重命名/图片、
  文件操作、状态板、工具总览；全部为浅色。源文件 `/Users/abc/Project/9.22/design.pen`
  （pen.dev，不进入 Git）；总览 `docs/design/0.3.0/pen/overview.png`。
- 完成定义：用户逐屏确认后才进入全站迁移；确认记录写入 `design-qa.md`。

**T-03C-02 — token 生成与主题**

- 新增 `scripts/ui/generate-m3-tokens.mjs`，devDependency
  `@material/material-color-utilities`（Apache-2.0）；种子 `#00696D`。
- 输出 `ui/src/styles/m3/tokens.generated.css`：color roles（primary/secondary/tertiary/
  error/surface/surface-variant/outline/…）、light；不生成 dark。
- `typescale.css`：display/headline/title/body/label 全量；CJK 行高单独调。
- `shape.css`：none/xs/sm/md/lg/xl/full；`elevation.css`：0–5；`motion.css`：short/medium/long +
  emphasized/decelerate/accelerate；`state.css`：hover/focus/pressed/dragged 透明度。
- 不用 `color-mix()`（macOS 13 WebKit 兼容风险），预生成 rgba。
- 测试：token 完整性、浅色对比度、生成脚本幂等；不出现未使用的 dark 变量。
- 完成定义：`pnpm --dir ui m3:tokens` 可重现；无运行时配色依赖。

**T-03C-03 — 字体与图标**

- 自托管 Roboto Variable（Latin，Apache-2.0）；CJK 系统字体栈；`font-display: swap`。
- Material Symbols Rounded 子集（Apache-2.0），映射现有 42 个 `ViewerIconName`；
  保留缺失图标的 fallback 测试。
- 更新 NOTICES/许可证检查；删除 Lucide 资源前确认全部映射。
- 测试：图标名全覆盖、无 404、字体加载失败 fallback、CSP 允许本地字体。
- 完成定义：无外部字体/CDN 请求。

### 6.2 组件与页面

**T-03C-04 — M3 primitives 第一批**

- 落点：`ui/src/components/ui/ViewerButton.tsx`、`ViewerIconButton`（新）、`ViewerField.tsx`、
  `ViewerDialog.tsx`、`ViewerPopover.tsx`、`ViewerMenuRow.tsx`、`ViewerChoiceChip.tsx`。
- 变体：filled/tonal/outlined/text/elevated；icon button；filled/outlined field；
  basic dialog；menu；filter chip/segmented。
- 保留现有 props/aria；测试快照改为语义/角色断言，不锁具体 class。
- 完成定义：现有组件测试全绿；视觉样张通过。

**T-03C-05 — M3 primitives 第二批**

- 新增/升级：progress（linear/circular）、snackbar、card、list、
  top app bar、tabs、switch、slider、badge、tooltip。
- 所有交互组件支持键盘、焦点环、disabled/read-only、reduced motion。
- 完成定义：组件级 a11y 测试；无原生控件外观残留（除 checkbox/radio 语义）。

**T-03C-06 — App shell 与导航**

- 落点：`ui/src/App.tsx`、`Workspace*`、侧栏/顶部栏。
- top app bar + 文件夹栏（secondary pane）；不新增一级 navigation rail；
  responsive：窄窗口折叠文件夹栏；不改变现有信息架构。
- 完成定义：打开项目/浏览/切换视图的路径与 0.2.5 等价；键盘/读屏可用。

**T-03C-07 — 工作区/网格/空态**

- 落点：`ui/src/components/ContentBrowser*`、`EmptyProject*`、`ViewerEmptyState*`、
  `SearchToolbar*`、`TaskBar*`。
- M3 cards/lists/chips/progress；加载/扫描/空项目/错误/恢复状态重做。
- 完成定义：workspace 42 个验收状态更新；无布局跳动。

**T-03C-08 — 预览界面**

- 图片/文本/视频/其他文件预览 chrome、工具栏、状态条 M3 化；
  画布与 video surface 本身不改渲染，只改周边控件。
- 浅色下图片 matte 保持中性，不染色彩；画布区域不跟随界面强调色。
- 完成定义：viewing 28 + video 12 状态更新；画布性能不回归。

**T-03C-09 — 评审工作台**

- 落点：`ImageReviewWorkspace.tsx`、`ReviewContextBar`、`ReviewFeedbackRail`、
  `ReviewInspector`、`AnnotationToolbar`、`FeedbackComposer`。
- 顶部工具/标记菜单、样式条、意见栏、只读横幅、语音按钮统一 M3；桌面密度紧凑。
- 完成定义：review 37 + feedback 15 状态更新；与工作流 A/B 的真机验收合并。

**T-03C-10 — 对话框与流程**

- 项目打开/迁移/存档/历史/冲突/设置/批量/文件操作等全部 M3 dialog、
  menu、snackbar、progress。
- 完成定义：dialog 7 状态 + 新增语音设置状态；无嵌套 modal 死路。

**T-03C-11 — 高对比、减少动态**

- `prefers-contrast`、`prefers-reduced-motion`；0.3.0 固定浅色，不实现主题切换；
  图片画布不随界面强调色变化。
- 完成定义：两套偏好 × 关键界面手验；浅色对比度脚本通过。

**T-03C-12 — 旧样式清理与验收**

- 删除未引用的 `--viewer-*` 别名、旧 CSS、Lucide 资源；保留兼容层必须注释与移除计划。
- 更新 `acceptanceStateCatalog.json`、`design-qa.md`、`SHORTCUTS.md`、用户指南截图。
- 完成定义：`rg -- '--viewer-' ui/src` 只剩允许的别名（或为零）；验收证据完整。

## 7. 跨切面

**T-03D-01 — 依赖与许可证**

- Rust：`sherpa-onnx`（首选）或 `whisper-rs`（回退）、`cpal`、`rubato`（版本锁定）；
  `deny.toml`、NOTICES、exceptions。
- npm：`@material/material-color-utilities`（dev）、Roboto/Material Symbols 资产；
  `check-npm-licenses`。
- 模型：单独模型卡 + hash；不提交二进制。
- 完成定义：`pnpm security`、`dependencies:exceptions` 绿。

**T-03D-02 — 性能**

- 标注：200 条/4K 图，切换/拖动/缩放 p95；canvas 重绘不随反馈栏滚动抖动。
- 语音：冷启动加载、10s/30s 延迟、内存峰值。
- UI：CSS/字体/图标体积预算；首屏与页面切换不回归 >10%。
- 完成定义：`docs/quality/perf-baseline.md` 更新；`verify:perf` 绿。

**T-03D-03 — 文档**

- `docs/PRODUCT_SPEC.md`、`FEATURE_REFERENCE.md`、`USER_GUIDE.md`、`SHORTCUTS.md`、
  `DATA_PRIVACY.md`、`SUPPORTED_FORMATS.md`（如受影响）。
- 明确：标注样式/协议 v5、语音离线与权限、M3 主题、0.2.5 不兼容 v5。
- 完成定义：用户不需要读代码即可理解新功能与限制。

**T-03D-04 — 发布门禁**

- `pnpm verify:clean`、`pnpm architecture:boundaries`、`architecture:trends`、`gate:m1/m2/m3`、
  `test:review-protocol`、`test:review-loop`、`test:visual-acceptance`、`test:native-acceptance`。
- 打包 `.app`：模型资源存在、麦克风权限文案、浅色/高对比、语音全流程、重开项目。
- 完成定义：0.3.0 发布清单全部勾选；`v0.3.0` 标签只在用户确认后创建。

## 8. 兼容与迁移

| 场景 | 行为 |
| --- | --- |
| 0.2.5 持续评审 v3/v4 | 0.3 可读；style 缺失的目标按 anchor 推导默认样式；首次修改写 v5 |
| 0.3.0 打开 0.2.5 旧版 v1 | 只读；提示迁移到持续评审；迁移后可用新工具 |
| 0.3 写出的 v5 | 0.2.5 不能读；打开时提示“需要 0.3 或更新版本”，不得静默损坏 |
| 0.3 读 v5 | 正常；图片目标 style 缺失按默认，视频/Asset 目标忽略 style |
| 归档/恢复 | v5 归档包含 style；v4 归档恢复后默认样式；hash 校验包含 style |
| UI 迁移 | 旧 `--viewer-*` 别名逐步映射到 `--md-sys-*`；0.3.0 发布前清理 |
| 回滚 | 0.3 编辑过的 v4 会升级为 v5，回滚 0.2.5 不可读；升级前建议归档/备份 |

## 9. 阶段与门禁

| 阶段 | 内容 | 退出条件 |
| --- | --- | --- |
| S0 决策/Spike | D1–D8 确认；ASR 四模型 bake-off；v5 兼容 spike；23 屏视觉稿逐屏确认 | 三项 spike 有数据；ADR 0009/0010/0011 草案；用户确认路线 |
| S1 基础 | M3 tokens/primitives 第一批；`AnnotationStyle` + v5 协议；工具注册表 | token/协议单测绿；v4 fixture 可读；现有 UI 不回归 |
| S2 标注 | 渲染样式、顶部标记菜单、新工具、选择编辑、撤销、Composer、只读 | 11 个入口可用；200 条性能达标；A 验收目录更新 |
| S3 语音 | 端口/采集/ASR 适配器/IPC/UI/设置/模型打包 | 断网全流程；CER/WER 达标；七类错误手验 |
| S4 M3 页面 | shell → 工作区 → 预览 → 评审 → 弹窗 | 全站无旧视觉；浅色/高对比通过 |
| S5 收口 | 性能、a11y、打包、文档、真机验收 | 发布门槛 §10 全绿；用户确认 |

粗估：单人串行约 18–24 周；S2/S3 与 S4 可并行，压缩到约 12–16 周。估算不含返工与真机验收等待。

## 10. 0.3.0 发布验收清单

- [ ] 10 种标记工具（11 个入口）在标准测试项目全部可用，样式/位置/文字持久化，重开不丢。
- [ ] 只读场景全部有原因与出口；无“灰色菜单无解释”。
- [ ] 语音断网可用；中文 CER ≤12%、英文 WER ≤18%；10s p95 ≤3s；音频不落盘。
- [ ] 麦克风允许/拒绝/无设备/损坏模型/静音/超长/取消七类路径有证据。
- [ ] 全站 M3：浅色/高对比/减少动态；所有页面与弹窗；无旧 token 残留。
- [ ] 141 个既有状态 + 新增工具/语音/主题状态全部走查；验收证据入库。
- [ ] 键盘可完成核心闭环；WCAG 2.2 AA 对比度/焦点/目标尺寸通过。
- [ ] `pnpm verify:clean`、边界、趋势、M1/M2/M3、review protocol/loop、visual/native acceptance 全绿。
- [ ] 打包 `.app` 真机完成“标注 → 语音 → 保存 → 重开 → 归档 → 再打开”。
- [ ] 文档、ADR、模型卡、许可证、已知限制更新；用户明确确认可用。

## 11. 风险与缓解

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| ASR 中文准确率不达标 | 口述体验差 | S0 先实测；备选 turbo；领域热词；允许手动改字 |
| 937MB 模型导致打包/内存压力 | 分发困难、低配卡顿 | S0 先测安装包/内存；回退 small/turbo；加载/取消状态 |
| v5 协议迁移破坏旧评审 | 数据丢失/无法打开 | v4 fixture 回归；先读后写；升级前归档；明确错误 |
| 新工具增多导致交互复杂 | 学习成本高 | 10 种标记分组、快捷键、样式记忆、渐进披露；不做多选/图层 |
| M3 全站迁移范围失控 | 长期新旧混用 | token/primitives 先行；按页面切片；每切片验收；最后清理 |
| 浅色界面与素材边界不清 | 评审误判 | 画布中性 matte；界面强调色不进入画布；缩放/对比检查 |
| 麦克风 TCC 无法自动化 | 权限回归 | 手验矩阵 + 明确错误 + 设置深链；CI 只测 mock |
| 模型许可证/再分发 | 无法发布 | 只用 MIT/Apache 明确模型；模型卡 + hash；排除 SenseVoice 权重除非法律确认 |
| 语音推理阻塞 UI | 卡顿 | spawn_blocking、单会话、取消、超时、加载状态 |

## 12. 待确认问题

1. **安装包预算**：Qwen3-ASR-0.6B 约 937 MB 的增量是否可接受？若不可接受，优先 Whisper turbo/small 或 Paraformer。
2. **视觉稿确认**：23 张画面是否逐屏通过？特别是顶部标记菜单的 11 个入口、320px 意见栏、只读 banner 的密度。
3. **验收语料**：语音准确率用本地真实口述语料验收，是否接受“不提交仓库、只记录指标与日期”？
4. **设置范围**：语音设置是否暴露“更换模型”，还是只显示内置模型状态与许可证？
