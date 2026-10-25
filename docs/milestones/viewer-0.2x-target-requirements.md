# Viewer 0.2.x 功能扩展规划（规划稿）

> Status: Draft（2026-10-13 起草，2026-10-25 修订，待评审）
>
> 基线：v0.2.0（main = 2297902）
>
> 2026-10-25 修订：三大功能目标收拢为 0.2.x 切片，本轮收口定版 0.2.5，不占用 0.3.0 版本号；
> 0.3.0 的主题目标届时另行规划，不在本文范围。
>
> 本文是 0.2.x 扩展切片的规划与技术选型依据。按项目约定，跨模块改动在开工前仍需出对应设计文档；
> 涉及 IPC/DTO/契约变更的切片需 ADR。

## 1. 目标与范围

本轮（0.2.1–0.2.5，定版 0.2.5）的三大功能目标：

1. **广泛图像格式兼容**：JPG、PNG、GIF、TIFF、HEIC、PSD、RAW、AI、SVG、AVIF 等市面主流格式的
   浏览、缩略图与预览支持。
2. **Markdown 阅读 + 编辑**：完整支持 Markdown 文档阅读与编辑，新增软件专属的文档阅览/编辑界面。
3. **图片批量处理**：在已有批量重命名基础上，新增批量格式转换、尺寸修改、压缩。

不在本轮（0.2.x）范围：签名/公证/打包/发布工程；图片编辑器（调色/修图）；动画播放
（留待 0.3 周期另行规划）。

## 2. 现状（v0.2.0）

- 图片完整解码管线仅 JPEG/PNG；GIF/WebP/AVIF/HEIC/HEIF/BMP/TIFF/SVG/ICO/PSD/RAW 等全部归类为
  `unsupported_image`（只列出、不解码，见 `crates/viewer-infrastructure/src/scan/file_classifier.rs`
  与 docs/product/SUPPORTED_FORMATS.md）。
- Markdown 仅只读预览（表格/删除线，HTML 净化，10 MiB 截断），无编辑、无写回。
- 批量重命名、内部拖拽、会话撤销已在 0.2.0 实现（BatchRenameDialog、useOrganizationPointerDrag、
  undo_last_operation）。
- 批量格式转换/尺寸修改/压缩：无。

## 3. 技术调研结论（开源库/系统能力选型）

### 3.1 图像解码：macOS ImageIO 原生覆盖（零新依赖）

`CGImageSource` 在 macOS 13（本项目最低支持版本）上原生支持的读取格式包括：
JPEG、PNG、GIF、TIFF、BMP、ICO、WebP（macOS 11+）、AVIF（macOS 13+，`public.avif`）、
HEIC/HEIF、PSD（`com.adobe.photoshop-image`）、主流相机 RAW（CR2/CR3/NEF/NRW/ARW/SR2/RAF/ORF/
RW2/PEF/DNG 等，均为 CGImageSource 支持类型）。

项目现有 `viewer-platform-macos/src/image/` 已经基于 ImageIO 构建 JPEG/PNG 管线（缩略图、预览、
viewer-image:// scheme）。**扩展格式主要是放开分类 + 将这些格式接入现有解码管线 + 逐格式验证**，
不需要引入新的解码库。

排除项及理由：
- Rust `image` crate：解码格式少于 ImageIO（无 HEIC/RAW/PSD），无收益。
- libraw/libheif 等 C 库：与 ImageIO 能力重叠，且引入 C 依赖会加重现有许可证审计/打包负担
  （项目对第三方依赖有 deny.toml + THIRD_PARTY_NOTICES + 锁哈希的严格约束）。

### 3.2 SVG：resvg（需引入，纯 Rust）

ImageIO 不支持 SVG。选型 **resvg**（Linebender 维护）：

- 纯 Rust（usvg 解析 + tiny-skia 栅格化），无 C 依赖；许可证 Apache-2.0 OR MIT，与本项目兼容。
- 活跃维护（2026-09 仍发版，v0.48.x），约 1600 个 SVG→PNG 回归测试，防递归/栈溢出保护
  （对处理不可信来源素材重要）。
- 静态 SVG 1.1 子集，不支持动画/脚本——审阅场景够用（不支持项明确标注）。
- 接入点：`viewer-platform-macos`（或新建 port 适配）将 SVG 栅格化为位图后走现有位图管线。

### 3.3 AI（Adobe Illustrator）：CoreGraphics PDF 渲染（零新依赖）

现代 `.ai` 文件本质是 PDF 兼容流。方案：`CGPDFDocument` 渲染 PDF 兼容 `.ai` 文件首页为位图，
走现有位图管线。旧的纯 PostScript `.ai`（AI 9 之前）不支持，明确标注不支持状态。

### 3.4 Markdown 编辑器：CodeMirror 6（React 侧引入）

选型 **CodeMirror 6 + @uiw/react-codemirror**（均 MIT）：

- Markdown 字符串进出一致（无 AST 转换层、无格式劫持），与"文件系统是唯一数据真实来源"原则吻合。
- 模块化、大文档性能好、可访问性（键盘/读屏）为一级目标；React 19 兼容。
- 排除 Monaco（体积大、定位是 IDE）与 Tiptap/Slate/Lexical（富文本 AST 模型，Markdown 往返有损）。
- 界面形态采用**源码 + 实时预览双栏**，复用现有只读 Markdown 预览渲染管线，与当前双栏阅读布局一致；
  inline WYSIWYG 类库（Atomic Editor、live-markdown 等）均太年轻，暂不采用，可作 0.3 周期观察项。

### 3.5 图片批量转换/压缩/缩放：ImageIO 编码（零新依赖）

`CGImageDestination` 原生写 JPEG/PNG/GIF/TIFF/HEIC，macOS 11+ 起 WebP、macOS 14+ 起 AVIF
（运行时用 `CGImageDestinationCopyTypeIdentifiers` 探测，不探测到即禁用该输出格式）。
质量经 `kCGImageDestinationLossyCompressionQuality` 控制；缩放经
`CGImageSourceCreateThumbnailAtIndex` + `kCGImageSourceThumbnailMaxPixelSize`（与现有缩略图同源，
色彩管理一致）。

排除项：mozjpeg/oxipng/ravif 等优化编码器（更好的压缩比但均为 C 依赖，且压缩比收益对本场景
非关键；若 0.3 后确有需求再评估）。

## 4. 架构落点（按六边形架构）

| 改动 | 位置 |
| --- | --- |
| 文件分类器扩展（unsupported_image → 各格式图片类） | viewer-infrastructure/scan/file_classifier.rs + viewer-domain/file.rs |
| ImageIO 解码格式矩阵（GIF/TIFF/HEIC/PSD/RAW/AVIF/WebP…） | viewer-platform-macos/src/image/ |
| SVG 栅格化适配（resvg） | viewer-platform-macos（新模块）+ viewer-application Port 扩展 |
| AI(PDF) 渲染适配 | viewer-platform-macos/src/image/ |
| Markdown 写回命令 + 脏状态 + 外部变化冲突检测 | viewer-application/text.rs、viewer-infrastructure/text/、src-tauri IPC、ui |
| 文档阅览/编辑界面（CodeMirror 6） | ui/src/components/（新） |
| 批量转换/缩放/压缩用例 + 任务化 | viewer-application（新用例）、viewer-platform-macos（编码）、src-tauri IPC、ui（批量面板扩展） |

契约影响：新增 IPC 命令（Markdown 保存、批量图像处理 preview/execute）；文件分类结果枚举变化。
按约定出 ADR（合并为一份"0.3 格式与批量处理契约"ADR 或随首切片出）。

## 5. 版本切片（0.2.x 系列 → 0.2.5 收口）

每个切片独立验收，完成后按 `type(scope): outcome` 提交，走 `pnpm verify:clean` 门禁。

### 0.2.1 — 图像格式兼容 I：ImageIO 矩阵

- 分类器接入：GIF、TIFF、BMP、ICO、WebP、AVIF、HEIC/HEIF、PSD、DNG/主流 RAW 从
  `unsupported_image` 移入完整图片管线（缩略图 + 预览 + 对比）。
- 逐格式解码验证矩阵（正常/损坏/截断/超大像素防炸弹）；色彩配置文件与方向处理核对。
- 交付物：SUPPORTED_FORMATS.md 更新为分级承诺（完整解码 / 仅列出）。
- 依赖变化：无。

### 0.2.2 — 图像格式兼容 II：SVG 与 AI

- 引入 resvg（workspace 依赖，锁版本）；SVG/SVGZ 栅格化接入缩略图与预览。
- AI（PDF 兼容流）经 CGPDFDocument 渲染；纯 PostScript `.ai` 明确标注不支持。
- 验证：不可信 SVG 的防护测试（递归/超大/外部实体）；PDF 渲染内存预算。
- 依赖变化：+resvg（Apache-2.0 OR MIT，纯 Rust）→ THIRD_PARTY_NOTICES/deny 审计。

### 0.2.3 — Markdown 编辑与文档阅览界面

- 新增软件专属文档阅览/编辑界面（替换/升级当前 TextPreview 双栏）：CodeMirror 6 编辑 + 实时预览。
- 保存：Cmd+S / 工具栏；原子写回（临时文件 + rename，复用文件事务协议思路）。
- 冲突检测：文件自加载后被外部修改（FSEvents + 内容指纹）时阻止盲目覆盖，提示重载或另存。
- 只读模式/权限失败路径；10 MiB 截断策略对编辑的一致处理（超限只读）。
- 依赖变化：ui +@codemirror/* + @uiw/react-codemirror（MIT）；npm 许可证审计。

### 0.2.4 — 图片批量处理

- 批量面板（与批量重命名同一任务模型）：格式转换（JPEG/PNG/HEIC/TIFF/WebP，AVIF 按运行时探测）、
  尺寸修改（最长边/百分比/精确像素，仅缩小时默认不放大）、压缩（质量 0-100）。
- 执行前预览（逐文件前后大小/尺寸估计）+ 冲突检查（目标已存在时跳过/覆盖/后缀策略）。
- 输出策略默认"新文件"（原目录加后缀或输出到子目录），避免覆盖原始素材；覆盖需显式勾选。
- 批量任务为单一撤销步骤？——**否**：转换产生新文件，遵循"复制不由 Viewer 撤销"的既有约定；
  覆盖模式下同样不提供撤销，执行前预览即安全网。
- 验证：色彩配置保真、HEIC 输出的质量档位、RAW→JPEG 直转矩阵。

### 0.2.5 — 收口（三大功能定版）

- 三大功能整体回归 + 性能基线复测（新解码格式纳入滚动场景，p95 恶化 >10% 需说明并更新基线）。
- PRODUCT_SPEC / USER_GUIDE / FEATURE_REFERENCE / SUPPORTED_FORMATS / CHANGELOG 同步。
- ADR/契约文档定稿。
- 定版 0.2.5；0.2.5 → 0.3.0 的新增主题目标届时另行规划，不在本文范围。

## 6. 风险与待决策问题

1. **RAW 解码质量**：ImageIO 的 RAW 解码是"快速预览"级（无相机配置文件、降噪弱）。对"审阅 AI
   素材"的主场景无影响；若将来需要专业 RAW 冲图，再评估 libraw（C 依赖）。
2. **AVIF 编码可用性**：依赖 macOS 14+；macOS 13 上该输出项自动禁用（运行时探测）。
3. **超大图像防炸弹**：新格式放开后必须保留像素上限闸门（沿用现有解码限制机制），验证矩阵要含
   声明尺寸与实际数据不符的恶意样本。
4. **动图（GIF/APNG/AVIFS）**：0.2.1 先支持首帧静态展示；动画播放不进本轮 0.2.x，
   留待 0.3 周期另行规划。
5. **Markdown 编辑的并发写**：外部 Agent 与用户同时写同一文件的处理策略（检测→提示为主，
   不做自动合并）。
