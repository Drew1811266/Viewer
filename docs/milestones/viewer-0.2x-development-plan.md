# Viewer 0.2.x 开发方案（0.2.1–0.2.5）

> Status: Implemented baseline（0.2.5 开发基线，2026-10-27；残余项见
> [2026-10-27-viewer-0.2.5-closure.md](../quality/2026-10-27-viewer-0.2.5-closure.md)）
>
> 需求依据：[viewer-0.2x-target-requirements.md](viewer-0.2x-target-requirements.md)（2026-10-25
> 修订版，基线 v0.2.0）。
>
> 本文把需求稿的五个切片展开为任务级开发方案：每切片给出任务分解（含代码落点）、测试与验收
> 标准、契约与依赖治理动作。任务编号 `T-<切片><序号>`，供进度检查点与提交说明引用。所有代码
> 落点均已对照 v0.2.0 源码逐一核实（见 §1 现状索引）。

## 0. 总体约定

- **切片收口标准**：`pnpm verify:clean` 通过（quality + security，且验证零仓库漂移）+ 各切片
  专属验收（见各节"验收"）。提交遵循 `type(scope): outcome`。
- **ADR-0008《0.2.x 格式扩展与批量处理契约》**：在 T-021-01 立项（草案随 0.2.1 首个提交入库），
  记录 `FileKind::Image`、`ImageFormat` 扩展、分类器变化、SVG/AI 后端、Markdown 写回 IPC、
  批量处理 IPC 与"不提供批量撤销"的约定；随切片补充，0.2.5 定稿。
- **依赖新增只发生在两处**：0.2.2（workspace `+resvg +flate2`）与 0.2.3
  （ui `+@codemirror/* +@uiw/react-codemirror`）。每次新增同步：`deny.toml`、
  `THIRD_PARTY_NOTICES.md`、`docs/quality/dependency-exceptions.json`，走
  `scripts/check-locked-dependencies.sh` 锁哈希流程与 `pnpm security`。
- **性能不变量**：新格式不得使既有 JPEG/PNG 滚动场景 p95 恶化 >10%（0.2.5 复测并更新
  `docs/quality/perf-baseline.md`）。
- **视觉验收**：每个引入新 UI 形态的切片（0.2.3/0.2.4，以及 0.2.1 若支持格式的卡片视觉有变化）
  在 `ui/src/acceptance/acceptanceStateCatalog.json` 增加状态并按流程产出 `design-qa.md` 证据。

## 1. 已核实现状索引（改动面）

| 环节 | 位置 | v0.2.0 现状 |
| --- | --- | --- |
| 扩展名分类 | `crates/viewer-infrastructure/src/scan/file_classifier.rs:4` | `UNSUPPORTED_IMAGE_EXTENSIONS` 已枚举全部目标扩展名（gif…x3f，含 svg/svgz/jxl/jfif） |
| 领域文件类型 | `crates/viewer-domain/src/file.rs` | 工作树已有未提交 WIP：`FileKind::Image`（encode 8）+ `is_previewable_image()` 纳入 |
| 解码格式闸门 | `crates/viewer-platform-macos/src/image/image_io.rs:237` `image_format()` | UTI 白名单仅 `public.jpeg`/`public.png`，其余 `ImageError::Unsupported`——**这是全管线唯一的格式闸门** |
| 领域图像格式 | `crates/viewer-domain/src/image.rs` | `ImageFormat` 仅 `Jpeg`/`Png` |
| 工件编码 | `crates/viewer-platform-macos/src/image/encode.rs` | 一切表示（缩略图/fit/100%）经 `thumbnail_from_image_source` → `encode_png` 重编码为 PNG（Quick Look 后端同为 PNG） |
| 协议 MIME 闸门 | `src-tauri/src/image_protocol.rs:85` | 仅 `image/jpeg|image/png`。因工件恒为 PNG 重编码，**0.2.1 无需放宽**；测试 `protocol_returns_only_image_mime_types` 保持锁定 |
| 解码预算 | `crates/viewer-platform-macos/src/image/image_io.rs:23` | `DecodeBudget(700_000_000 bytes, 100_000_000 px)`，Original100Percent 强制校验——防炸弹闸门已存在 |
| 前端判定 | `ui/src/fileKinds.ts:6`；DTO 联合类型 `ui/src/api/types.ts:44` | `isPreviewableImage` 仅 `jpeg|png`；`'unsupported_image'` 在联合类型中 |
| 下游穷举（已随 WIP 补齐） | `review/assets.rs`（候选分类/媒体探测）、`src-tauri/src/state/scan_index.rs`（派生元数据探测）、`src-tauri/src/state/markers.rs`（标记目标校验） | `Image` 已并入 Jpeg/Png 分支（未提交） |
| 文本管线 | `viewer-application/src/text.rs`、`viewer-infrastructure/src/text/preview.rs`、`src-tauri/src/markdown.rs` | 只读：`TextPreviewReader`（BOM/UTF-8/GB18030 检测、10 MiB 截断）、pulldown-cmark + ammonia 净化、`ExternalUrl` 白名单 |
| 文件命令模式（0.2.4 参照） | `src-tauri/src/commands/operations.rs`、`viewer-application/src/file_commands.rs` | `preview_rename → preflight_file_command → execute_file_command`，`write_lane` 单写通道、`BatchProgress` 经 `viewer://operation-progress`、`OperationJournal` 崩溃恢复、undo 栈 |
| 外部变化 | `src-tauri/src/watcher_runtime.rs` + `scan/reconcile` | FSEvents → `viewer://project-changed`，`ExpectedChangeLedger` 协调自身操作与外部事件 |

## 2. 0.2.1 — 图像格式兼容 I：ImageIO 矩阵

**目标**：GIF/TIFF/BMP/ICO/WebP/AVIF/HEIC/HEIF/PSD/DNG 及主流 RAW 从 `unsupported_image`
移入完整图片管线（缩略图 + 预览 + 对比 + 派生元数据 + 评审资产）。
**关键结论（已核实）**：工件恒为 PNG 重编码，因此协议、缓存、评审证据渲染链路无需结构性改动；
改动集中在**分类器**与**`image_format()` UTI 白名单**两处，其余为配套投影/测试/文档。

| 任务 | 内容 | 主要落点 |
| --- | --- | --- |
| T-021-01 | ADR-0008 立项（契约清单见 §0），随本切片首个提交入库 | `docs/adr/0008-*.md` |
| T-021-02 | 领域层定稿：`ImageFormat` 扩展（`Gif/Tiff/Webp/Avif/Heic/Bmp/Ico/Psd/Raw`）；`FileKind::Image` WIP 收口（含 `file.rs` 既有单测） | `viewer-domain/src/image.rs`、`file.rs` |
| T-021-03 | 分类器：新增 `IMAGE_EXTENSIONS`（gif/webp/avif/heic/heif/bmp/tif/tiff/ico/apng/psd/dng/cr2/cr3/nef/nrw/arw/srf/sr2/raf/orf/rw2/pef/srw/x3f）→ `FileKind::Image`；`jfif` 并入 `jpg\|jpeg` 归 `FileKind::Jpeg`（D1）；svg/svgz/jxl 维持 `UnsupportedImage`；分类器单测扩展（含大小写惯例） | `scan/file_classifier.rs` |
| T-021-04 | 解码闸门放开：`image_format()` UTI 映射表（`public.gif`、`public.tiff`、`org.webmproject.webp`、`public.avif`、`public.heic`/`public.heif`、`public.bmp`、`public.ico`、`com.adobe.photoshop-image`、各相机 RAW UTI + `public.camera-raw-image` 兜底）；未知 UTI 仍拒绝（保持默认拒绝）。多页/动画格式（GIF/APNG/TIFF 多页）取第 0 帧，明确不动画 | `platform-macos/src/image/image_io.rs` |
| T-021-05 | 投影审计（编译器驱动）：`cargo check --workspace` + `pnpm --dir ui check` 清零剩余穷举；重点复核 `SessionIndex` 图像元数据投影、浏览卡片分流、评审资产（assets.rs 已补） | 全 workspace |
| T-021-06 | 前端：`api/types.ts` 联合类型加 `'image'`；`fileKinds.ts` 的 `isPreviewableImage`/`isImageFile` 纳入；确认 `ImageCell`/对比视图无 per-format 假设；`api/viewer.test.ts` 契约同步 | `ui/src/api/types.ts`、`ui/src/fileKinds.ts` |
| T-021-07 | fixture 与逐格式验证矩阵：扩展 `scripts/generate-g1-image-fixtures.swift` 合成 GIF/TIFF/BMP/ICO（CGImageDestination 可写）；WebP/HEIC/AVIF 按构建机运行时能力合成，否则提交固定 fixture；PSD 与 RAW 提交小型真实样本并在 fixture 目录记录来源。矩阵用例（每格式）：正常解码、截断/损坏、扩展名与内容不符、超大像素声明（防炸弹，含声明与实际不符样本）、多页取首帧、ICC（Display P3）与 EXIF 方向核对 | `scripts/generate-g1-image-fixtures.swift`、`tests/fixtures/`、新增 `tests/image_format_matrix.rs`（挂 viewer-platform-macos，`[[test]]` 引用） |
| T-021-08 | 原生渲染与 Quick Look 路径审计：wgpu 视口的资源解码（`platform-macos/src/image_render/decode.rs`）是否经过同一闸门；QL 缩略图对新格式的回退行为（QL 原生支持 HEIC/PSD 等，probe 失败时网格卡片应走 ImageIO 错误路径而非崩溃） | `image_render/decode.rs`、`quick_look.rs` |
| T-021-09 | 文档：`SUPPORTED_FORMATS.md` 分级重写（完整解码 / RAW 快速预览级 / 仅列出 svg/svgz/jxl）；ADR-0008 更新 | `docs/product/SUPPORTED_FORMATS.md` |

**验收**：`pnpm verify:clean`；`pnpm gate:m1`（含 G1 图像门重跑，fixture 重新生成后
`git diff --exit-code` 零漂移）；新矩阵测试进 `cargo test --workspace`；SUPPORTED_FORMATS 分级落盘。

**风险**：个别 RAW UTI 在 ImageIO 中缺失（如个别厂商新格式）→ 该扩展名按逐格式验证结果回退
保持 `UnsupportedImage`，不影响其余清单；PSD 合成 fixture 无系统路径 → 提交真实小样本解决。

## 3. 0.2.2 — 图像格式兼容 II：SVG 与 AI

| 任务 | 内容 | 主要落点 |
| --- | --- | --- |
| T-022-01 | 依赖引入 `resvg` + `flate2`（workspace 锁版本）；SVGZ 经 flate2 解压后进 usvg，解压设字节上限（防 gzip 炸弹）；同步 deny/NOTICES/exceptions + 锁哈希流程 | `Cargo.toml`、`deny.toml`、`docs/quality/dependency-exceptions.json` |
| T-022-02 | SVG 后端：platform 新模块 `image/svg.rs`——usvg 解析（外部引用默认禁用）→ tiny-skia 按请求像素尺寸栅格化 → 复用 `encode_png` 产工件；probe 返回 SVG 固有宽高（无固有尺寸时按策略给默认视口并在 SUPPORTED_FORMATS 标注）；`MacImagePort` 在 ImageIO 之前按扩展名分流 | `platform-macos/src/image/svg.rs`、`quick_look.rs`（分派） |
| T-022-03 | AI(PDF) 适配：`CGPDFDocument` 渲染 PDF 兼容流首页为位图（`objc2-core-graphics` 需开 `CGPDFDocument` feature）；纯 PostScript `.ai`（`%!PS` 魔数）走明确不支持状态 | `platform-macos/src/image/pdf.rs`（新） |
| T-022-04 | 分类器：svg/svgz → `FileKind::Image`（0.2.2 内完成）；`.ai` 从 `Other` 改归 `FileKind::Image`（D4），预览态区分 PDF 兼容流与纯 PostScript 不支持 | `scan/file_classifier.rs` |
| T-022-05 | 安全测试：不可信 SVG 语料（深嵌套、超大 viewBox、外部实体/脚本元素被 usvg 丢弃、递归炸弹防护）、PDF 巨页内存预算、SVGZ gzip 炸弹（解压上限） | 新增 `tests/svg_security_boundaries.rs` |
| T-022-06 | 文档与视觉验收：SUPPORTED_FORMATS 增 SVG/AI 行（静态 SVG 1.1 子集、不支持脚本/动画/外部引用；旧 .ai 不支持）；验收目录新增 SVG/AI 缩略与预览状态 | `docs/product/SUPPORTED_FORMATS.md`、`ui/src/acceptance/` |

**验收**：`pnpm verify:clean`（含 `pnpm security` 的 cargo-deny/licenses 全过）；恶意样本测试绿；
SVG/AI fixture 矩阵同 0.2.7 规格并入 `image_format_matrix.rs`。

## 4. 0.2.3 — Markdown 编辑与文档阅览界面

| 任务 | 内容 | 主要落点 |
| --- | --- | --- |
| T-023-01 | 前端依赖：`@codemirror/*` + `@uiw/react-codemirror`（锁定版本；npm 许可证审计 + exceptions 同步）；验证 React 19 + Biome 兼容（必要时 BIOME_BASELINE_EXCEPTIONS 记录） | `ui/package.json`、`ui/biome.json` |
| T-023-02 | 保存命令 `save_text`（命名随既有 snake_case）：校验 session/generation → 只读项目拒绝（复用 `project_read_only_operation` 模式）→ `RelativePath` 必须匹配已索引 Markdown/Text 节点 → symlink/alias 拒绝（复用 `markers.rs validated_marker_target` 模式）→ **原子写回**：临时文件 + rename（复用 `operation/copy` staged 原语）→ 写前 blake3 内容指纹与加载时指纹比对，不一致返回 `conflict` 错误变体 | `src-tauri/src/commands/preview.rs`、`state/preview.rs`、`viewer-application/src/text.rs`、`viewer-infrastructure/src/text/` |
| T-023-03 | 编码与截断策略（D3）：回写统一 UTF-8；非 UTF-8 源文件保存时 UI 明确提示将发生编码转换并需确认；>10 MiB 文件维持只读编辑（显示截断说明），不提供写回 | 同上 + `text/preview.rs` |
| T-023-04 | UI：升级现有 TextPreview 双栏为「CodeMirror 源码 + 实时预览」文档界面；脏状态标记、Cmd+S/工具栏保存、冲突对话框（重载 / 另存副本）、只读横幅、保存中/失败反馈；快捷键与 `SHORTCUTS.md` 同步 | `ui/src/components/TextPreview*.tsx`（升级或新组件）、`ui/src/app/workspace/` |
| T-023-05 | 契约与测试：`api/viewer.test.ts` 新命令契约；安全测试（越权路径/符号链接/只读/跨项目/写 `.viewer` 拒绝）；写回竞争测试（保存时文件被外部替换必须失败而非覆盖——参照 `file_transactions.rs` 的 TOCTOU 风格）；watcher `project-changed` 触发"文件已被外部修改"提示 | `tests/`（新增 `text_save_boundaries.rs`）、`ui/src/api/viewer.test.ts` |
| T-023-06 | 视觉验收：编辑态/脏态/冲突对话框/只读态新增验收状态与证据 | `ui/src/acceptance/` |

**验收**：`pnpm verify:clean`；原子写回与冲突竞争测试绿；手动验收（真机编辑-保存-外部修改-冲突
流程）留证据。

## 5. 0.2.4 — 图片批量处理

| 任务 | 内容 | 主要落点 |
| --- | --- | --- |
| T-024-01 | 领域词汇：批量图像操作计划项（目标格式/质量 0-100/缩放规则：最长边/百分比/精确像素，默认仅缩小/冲突策略：跳过/覆盖/后缀/输出策略：同目录后缀或子目录）与结果码 | `viewer-domain/src/operation.rs`（或新 `batch_image.rs`） |
| T-024-02 | 应用用例 `BatchImageService`：完整复用 `FileCommandService` 模式——plan/preview（逐文件前后尺寸与大小估计）→ preflight（冲突检查、只读/陈旧拒绝）→ `write_lane` 单写通道执行 → `BatchProgress` 经 `viewer://operation-progress` → 取消。**不提供撤销**（复制约定，预检即安全网） | `viewer-application/src/`（新模块）、复用 `scheduler.rs` 派生优先级 |
| T-024-03 | 平台编码器泛化：`encode.rs` 从 PNG 专用扩展为 `CGImageDestination` 多格式（JPEG/HEIC/TIFF/WebP/GIF/PNG；AVIF 与 WebP 写入按 `CGImageDestinationCopyTypeIdentifiers` 运行时探测，不可用即从输出选项禁用）；质量 `kCGImageDestinationLossyCompressionQuality`；缩放复用 `thumbnail_from_image_source` 的 `MaxPixelSize`（色彩管理与缩略图同源） | `platform-macos/src/image/encode.rs` |
| T-024-04 | 输出与冲突策略实现：默认"新文件"（原目录加后缀或输出子目录）；覆盖模式属破坏性操作，复用 staged copy + `OperationJournal` + Trash-before-Replace（ADR 0002 语义，崩溃可恢复）；执行前预览展示逐文件冲突处置 | `viewer-infrastructure/src/operation/`、`service/` |
| T-024-05 | IPC：`preview_batch_image` / `execute_batch_image`（命名随 `preflight_file_command`/`execute_file_command` 风格定案）+ DTO + `api/viewer.test.ts` 契约 | `src-tauri/src/commands/operations.rs`、`dto/` |
| T-024-06 | UI：批量面板扩展（与 `BatchRenameDialog` 同任务模型：预览→执行→进度→结果），格式/质量/缩放/输出选项，估计值标注"约" | `ui/src/components/`、`app/workspace/useOrganization*` |
| T-024-07 | 测试：色彩配置保真（Display P3 → HEIC/JPEG 往返）、HEIC 质量档位输出大小单调性、RAW→JPEG 直转矩阵、批量中途取消的部分结果一致性、覆盖模式崩溃恢复矩阵（复用 `m3_operation_journal` 规格） | `tests/`（新增 `batch_image_operations.rs`） |

**验收**：`pnpm verify:clean`；覆盖模式恢复测试绿；`pnpm gate:m3` 重跑（操作通道回归）。

**风险**：AVIF/WebP 编码能力随 macOS 版本差异（运行时探测兜底）；估计大小与实际编码偏差
（UI 标注为估计值）。

## 6. 0.2.5 — 收口

| 任务 | 内容 |
| --- | --- |
| T-025-01 | 全量回归：`pnpm verify` + `pnpm gate:m1` + `pnpm gate:m2` + `pnpm gate:m3`（门禁递归，实际以 m3 为准） |
| T-025-02 | 性能复测：混合格式语料（新格式占非 trivial 比例）纳入滚动与预览场景，复测 DecodeScheduler 与 image-render 基线；p95 恶化 >10% 逐项说明并更新 `docs/quality/perf-baseline.md` 与相关基线 JSON |
| T-025-03 | 文档同步：PRODUCT_SPEC（能力与边界）、USER_GUIDE/SHORTCUTS（编辑与批量操作）、FEATURE_REFERENCE、SUPPORTED_FORMATS（最终分级）、CHANGELOG（0.2.1–0.2.5 用户可见变化） |
| T-025-04 | 契约定稿：ADR-0008 定稿；`docs/quality/` 覆盖率与架构趋势基线刷新；workspace 版本号提升 0.2.5 |

## 7. 排序与依赖

- 严格顺序：**0.2.1 → 0.2.2**（同一条解码管线，闸门与分类器先后依赖）；**0.2.3 → 0.2.4**
  （0.2.4 的批量面板复用 0.2.3 已固化的任务模型与 UI 模式）。0.2.2 与 0.2.3 理论可并行
  （Rust 依赖 vs 前端依赖互不冲突），单人开发按编号顺序执行。
- 每切片独立验收提交；切片内按任务表顺序，T-021-07 的 fixture 矩阵先于 T-021-04 的白名单
  放开编写（先有测试再放开，保持"契约先行"）。
- **WIP 处理**：工作树现有未提交改动（`FileKind::Image` 域层 + 4 处下游 match 补齐 + 需求/方案
  文档）整体归入 T-021-01/T-021-02 的首个提交。

## 8. 已定案决策（2026-10-25 评审通过）

| # | 决策 | 结论 |
| --- | --- | --- |
| D1 | `.jfif` 归类 | 并入 `jpg\|jpeg` → `FileKind::Jpeg`（内容即 JPEG） |
| D2 | SVGZ 支持 | 支持：新增 `flate2`（纯 Rust、MIT/Apache-2.0），解压设字节上限；svgz 随 0.2.2 进入完整管线 |
| D3 | Markdown 回写编码 | 统一写回 UTF-8；非 UTF-8 源保存时 UI 明确提示编码转换并需确认 |
| D4 | `.ai` 扩展名分类 | 归 `FileKind::Image`；预览态区分 PDF 兼容流（可渲染）与纯 PostScript（不支持），SUPPORTED_FORMATS 标注 |

## 9. 风险登记册

| 风险 | 影响切片 | 缓解 |
| --- | --- | --- |
| ImageIO 对个别 RAW/新 UTI 缺失 | 0.2.1 | 逐格式矩阵先验证；缺失项回退 `UnsupportedImage`，清单以实测为准 |
| PSD/RAW 真实 fixture 的来源与体积 | 0.2.1 | 提交最小真实样本 + 目录内记录来源；RAW 承诺降级为"快速预览级" |
| 解压/像素炸弹绕过 ImageIO 预检 | 0.2.1/0.2.2 | `DecodeBudget` 硬限 + 矩阵含"声明尺寸与实际不符"样本；SVGZ 解压上限；PDF 渲染内存预算测试 |
| wgpu 原生视口与网格路径解码不一致 | 0.2.1 | T-021-08 专项审计；不一致则统一收敛到 `image_format()` 单一闸门 |
| CodeMirror 与 React 19/Biome 兼容 | 0.2.3 | T-023-01 先做隔离 PoC（单一组件 + 契约测试）再铺开 |
| 批量覆盖模式的数据安全 | 0.2.4 | 复用 ADR 0002 staged + journal + Trash-before-Replace；崩溃恢复矩阵强制 |
| 性能回退 | 全部 | 每切片跑 `verify:perf` 抽查；0.2.5 混合格式全面复测，p95 恶化 >10% 说明并更新基线 |
