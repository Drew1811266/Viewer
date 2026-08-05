# Viewer Tiered Visual Acceptance Design

> 状态：方案 A 已批准，等待书面规格复核
>
> 日期：2026-08-04
>
> 适用分支：`codex/viewer-atlas-product-migration`

## 1. 决策摘要

Viewer 的完整视觉迁移不再要求 89 个图谱状态在两个视口中全部通过 Finder、macOS Accessibility 和真实文件扫描逐个进入。该做法把组件视觉验证、产品交互验证和操作系统集成验证绑定成一条重型链路，单个控件角色或文件选择器时序变化就会阻塞整批视觉验收。

本设计采用已经批准的方案 A：

1. 新增一个独立、非发布的视觉验收入口，直接渲染 Viewer 的正式 React 组件和正式 CSS；
2. 用确定性状态目录覆盖迁移台账中的全部 89 个状态；
3. 用 Playwright 在一次浏览器运行中批量生成 `1024 × 720` 与 `1440 × 900` 的产品截图；
4. 继续把当前图谱参考和正式产品截图组成同视口联合对照图；
5. 把 PID 绑定的 Tauri 原生验收缩减为 12–15 条真正依赖桌面环境的关键冒烟链路；
6. 发布构建不包含视觉验收页面、状态目录、夹具、Playwright 或验收命令。

这项新决策取代以下旧约束：

- `2026-08-03-viewer-native-acceptance-controller-design.md` 第 2.2、3.2 和 5.3 节关于“不得向产品注入测试状态”和“全部状态必须通过真实操作进入”的要求；
- `2026-08-03-viewer-native-acceptance-controller-plan.md` 中要求 89 个状态全部由原生控制器进入的范围；
- `2026-08-02-viewer-atlas-to-product-complete-migration.md` Task 15 中要求 89 × 2 份证据全部来自原生窗口的闭环条件。

旧原生控制器不会删除。它继续承担桌面边界冒烟和最终关键链路验证，但不再是每个视觉状态的唯一入口。

## 2. 目标与成功标准

### 2.1 目标

- 89 个台账 ID 都有唯一、可执行、确定性的正式组件渲染状态；
- 两个批准视口可以在同一个 Playwright 浏览器进程中连续采集；
- 单状态失败只影响该状态，不需要重新打开 Finder、创建项目或等待缩略图扫描；
- 每个产品截图都绑定 commit、状态 ID、视口、状态目录版本和正式样式哈希；
- 每个状态都能生成“当前图谱参考 + 当前正式组件”的联合对照图；
- 视觉检查和原生交互检查分层报告，不再互相伪装为对方；
- 日常受影响状态验证以分钟内完成为目标，完整 89 × 2 快速视觉批次以单次连续运行完成为目标。

### 2.2 完成条件

新的验收架构完成必须同时满足：

1. 状态目录与迁移台账的 89 个 ID 一一对应，无遗漏、重复或额外状态；
2. `1024 × 720` 与 `1440 × 900` 均能生成精确尺寸截图；
3. 截图页面只使用 `ui/src` 中的正式组件、正式图标和正式样式，不复制图谱 HTML 作为产品实现；
4. Release UI 构建只包含现有 `index.html` 产品入口，不包含视觉验收入口；
5. Playwright 及其浏览器只存在于开发依赖和本地验收命令中；
6. 关键原生冒烟能够在唯一 Viewer 开发实例和一个固定夹具项目中运行；
7. 参考和产品截图必须先合成联合输入，再由视觉检查裁决；像素相似度不得单独决定通过；
8. 当前重型 `--all` 原生 178 状态采集不再作为日常或最终完成门禁。

## 3. 非目标

- 不为验收重新设计另一套 Viewer 界面；
- 不在正式 `App` 中增加可由普通用户触发的隐藏路由、快捷键或设置项；
- 不用测试夹具替代真实产品组件、正式 CSS 或真实图标；
- 不用浏览器截图声明 macOS 或 Windows 原生窗口行为已通过；
- 不在本阶段开发 Windows 原生控制器；
- 不以全屏像素差百分比替代设计人员对排版、间距、层级、裁切和状态语义的检查；
- 不把历史 commit 的截图升级为当前 commit 的证据。

## 4. 方案比较

### 4.1 方案 A：独立正式组件视觉入口（采用）

独立 Vite 入口加载正式组件和正式样式，通过状态目录提供确定 props、桥接响应和可观察状态。Playwright 复用一个浏览器进程采集全部状态。优点是快、确定、跨 macOS/Windows 视觉实现复用；代价是需要维护状态目录和非发布夹具。

### 4.2 方案 B：优化现有原生控制器（不采用为主方案）

共享一次性项目和 Viewer 会话，减少 Finder 和扫描次数。它能降低部分成本，但仍会受到 Accessibility 角色、系统文件对话框、窗口焦点和异步扫描的影响，无法成为高频视觉回归入口。

### 4.3 方案 C：仅组件单元测试与人工截图（不采用）

实现最少、速度最快，但无法稳定形成全部状态的双视口视觉证据，也难以发现真实布局、字体和媒体加载造成的回归。

## 5. 分层验收模型

### 5.1 第一层：组件和状态契约

Vitest 检查：

- 89 个 ID 与迁移台账完全一致；
- 每个 ID 都解析为一种已知场景类型；
- 每个场景只引用正式组件；
- 需要的媒体、文件元数据和桥接响应完整；
- 状态准备完成时设置唯一的 ready 标记；
- 失败时设置结构化 error 标记并使批次非零退出；
- 发布入口和发布构建配置不引用任何验收模块。

这一层不截图，运行速度应接近普通 UI 单元测试。

### 5.2 第二层：快速视觉批次

独立入口按以下 URL 合约渲染一个状态：

```text
/visual-acceptance.html?id=PRE-01&viewport=1024x720
```

页面只包含待验收产品画面，不包含状态选择器、调试面板或图谱导航。根节点必须暴露：

```html
<main
  data-acceptance-id="PRE-01"
  data-acceptance-viewport="1024x720"
  data-acceptance-status="ready"
>
```

Playwright 启动一个 Chromium 浏览器进程，按视口分组复用页面：

1. 导航到状态 URL；
2. 等待 `data-acceptance-status="ready"`；
3. 等待 `document.fonts.ready` 和所有可见图片完成解码；
4. 关闭动画、过渡和闪烁光标；
5. 验证根节点尺寸、状态 ID 和无横向溢出；
6. 采集精确视口 PNG；
7. 记录 DOM 断言、控制台错误、截图 SHA-256 和运行耗时；
8. 复用现有无损 PNG 合成能力生成同视口联合对照图。

批次支持 `--id`、`--wave`、`--changed` 和 `--all`：

- `--id PRE-01`：开发时只验证一个状态；
- `--wave 2`：验证迁移台账中的一个波次；
- `--changed`：根据组件到状态的显式覆盖映射运行受影响状态；
- `--all`：里程碑或最终提交运行全部 89 × 2 状态。

### 5.3 第三层：Tauri 原生冒烟

原生控制器只保留真正不能由浏览器层证明的边界：

1. 单实例开发启动；
2. 未打开项目窗口；
3. 系统文件夹选择器打开项目；
4. 项目扫描完成并显示主工作区；
5. 文件夹栏折叠与窗口调整；
6. 缩略图选择边界与键盘焦点；
7. 原生右键进入圆盘菜单；
8. 圆盘菜单激活图片预览；
9. 多选后进入图片对比；
10. 文本文件进入预览；
11. 信息面板快捷键；
12. 重命名或目标位置对话框；
13. Finder 文件拖入的有效和无效路径；
14. 窗口在 `1024 × 720` 和 `1440 × 900` 的关键响应；
15. 关闭项目并回到初始状态。

冒烟批次使用一个固定的一次性夹具和同一个 Viewer 进程。只有文件选择器、Finder 拖放等必要边界经过 Finder；其它链路不为每个状态重新创建项目。单条失败保存现场并继续收集非破坏性诊断，但不把失败截图标为通过。

### 5.4 第四层：最终视觉裁决

最终裁决使用联合对照图，而不是独立截图。每个状态检查：

- 字体、字号、字重和文字截断；
- 控件对齐、留白、密度和响应式重排；
- 边框、圆角、阴影、遮罩不透明度和选中边界；
- 图片裁切、背景、加载、错误和禁用状态；
- 圆盘菜单、弹层、对话框和浮动控件的层级；
- `1024 × 720` 下是否重叠、越界或裁切；
- 图谱与产品差异是否源于真实功能约束，并有书面记录。

截图可以证明可见状态，但不能单独证明交互正确或完整可访问性合规。

## 6. 非发布视觉入口架构

### 6.1 文件边界

计划新增：

```text
ui/
  visual-acceptance.html
  vite.visual-acceptance.config.ts
  src/acceptance/
    main.tsx
    AcceptanceApp.tsx
    acceptanceStateCatalog.ts
    acceptanceStateCatalog.test.ts
    acceptanceFixtures.ts
    acceptanceBridge.ts
    scenes/
      workspaceScenes.tsx
      viewingScenes.tsx
      dialogScenes.tsx
      feedbackScenes.tsx

scripts/
  viewer-visual-acceptance.mjs
  viewer-visual-acceptance.test.mjs
  viewer-acceptance-evidence.mjs

docs/reviews/
  viewer-native-smoke-matrix.md
```

`scenes/` 按产品表面拆分，避免一个 89 分支的巨型文件。它们可以构造 props、桥接响应和用户可见状态，但不能复制正式组件内部 JSX 或 CSS。

### 6.2 发布隔离

- 正式 `ui/vite.config.ts` 和 `ui/index.html` 不引用视觉入口；
- `pnpm build:ui` 继续只构建正式产品；
- 视觉入口使用独立 `vite.visual-acceptance.config.ts`，输出只允许位于 `target/viewer-visual-acceptance/site/`；
- `src/acceptance/` 不被正式入口导入；
- 仓库策略测试读取生产构建 manifest，若出现 `visual-acceptance`、`acceptanceStateCatalog` 或 Playwright 资源则失败；
- Playwright 放在根项目开发依赖，不进入 Tauri `resources`、Rust crate 或应用包；
- 正式应用不接受 `id=PRE-01`、环境变量或 localStorage 开关来进入测试状态。

### 6.3 正式组件复用规则

- 通用工作区状态优先渲染 `<App bridge={acceptanceBridge}>`，让正式状态协调器和布局共同参与；
- 全屏预览、对比、文档和明确的独立对话框允许直接渲染对应正式组件，以避免通过文件系统制造纯视觉边界状态；
- 场景只注入组件本来就公开接受的 props 或正式 `ViewerBridge` 接口结果；
- 若某状态无法通过稳定公开接口构造，应先提取正式、可测试的产品状态模型，而不是在场景中访问 React 私有状态；
- 所有文字、图标、文件元数据和媒体使用与图谱相同语义的确定夹具；禁止占位框、emoji、手绘 SVG 或复制图谱 CSS。

## 7. 状态目录与覆盖映射

每个目录项包含：

```ts
interface AcceptanceStateDefinition {
  id: AcceptanceId
  wave: 1 | 2 | 3 | 4
  referenceState: string
  scene: AcceptanceScene
  components: readonly string[]
  assertions: readonly AcceptanceAssertion[]
}
```

`components` 是 `--changed` 的显式反向覆盖来源。修改 `ImagePreview.tsx` 或预览样式时，命令解析到 PRE-01–PRE-07；修改通用 `ViewerButton` 或 token 时，解析到全部受影响场景。无法安全判断影响范围时自动退化为 `--all`，不能静默漏测。

目录由迁移台账生成或与台账做严格相等校验。目录中的 `referenceState` 必须等于台账对应行，防止产品截图和错误图谱状态配对。

## 8. 证据与结果模型

快速视觉证据写入：

```text
target/viewer-visual-acceptance/<commit>/<viewport>/<ID>/
  product.png
  reference.png
  combined.png
  manifest.json
  console.json
```

`manifest.json` 至少记录：

- schema 版本；
- commit、branch 和 dirty 状态；
- ID、wave、reference state 和 viewport；
- 状态目录文件哈希、正式 CSS 入口哈希和 atlas HTML 哈希；
- Playwright、浏览器和 Node 版本；
- ready 时间、截图时间和耗时；
- 产品、参考和联合图片路径、尺寸与 SHA-256；
- DOM 断言结果、控制台错误数和视觉裁决状态。

自动批次的初始视觉裁决为 `pending-visual-review`。只有联合对照图被检查且 P0/P1/P2 为零，台账才能记录 `pass`。

## 9. 错误处理与防卡死策略

- 每个浏览器状态有独立、较短的 ready 超时；失败后记录当前 DOM、控制台和截图，随后进入下一个非破坏性状态；
- 全批次有总时间预算；超过预算停止新增状态并输出未运行列表；
- 禁止无界轮询、递归重试和固定长 sleep；
- 图片解码、字体准备和异步组件状态都用可观察 Promise；
- 相同 ID、commit、viewport 的证据目录默认不可覆盖；调试运行使用独立 run ID，最终证据使用干净 commit；
- Playwright 或 Vite 子进程在 `finally` 中关闭，并由测试确认没有遗留端口或浏览器进程；
- 原生冒烟失败不会触发 89 状态浏览器批次重跑；浏览器视觉失败也不会触发 Finder 重开。

## 10. 测试策略

实现遵循测试驱动：先写目录、发布隔离、状态 ready、批次选择和证据清单的失败测试，再实现最小行为。

必须覆盖：

- 台账 89 ID 与状态目录精确相等；
- 缺状态、重复状态、错误 reference state 和未知场景类型均失败；
- 正式构建 manifest 不含视觉验收模块；
- `--id`、`--wave`、`--changed`、`--all` 选择正确且稳定排序；
- 一个浏览器进程复用多个状态，不为每张图重启浏览器；
- ready、error、字体和图片等待都有有界超时；
- 精确截图尺寸、路径边界、哈希和 manifest 原子写入；
- 某状态失败时其它非破坏性状态仍可运行并在汇总中明确失败；
- Playwright、Vite 和临时目录在成功与失败时均被清理；
- 原生冒烟矩阵只包含批准的桌面边界，不再自动扩展为 89 状态。

## 11. 执行顺序

1. 建立状态目录契约和发布隔离门禁；
2. 建立独立 Vite 入口与一个代表性状态 `PRE-01`；
3. 建立 Playwright 单状态截图、ready 协议和证据 manifest；
4. 批量覆盖 PRE、DOC、RAD 三组当前最易受原生入口影响的状态；
5. 扩展到全部 89 状态并启用 `--changed` 覆盖映射；
6. 抽取联合 PNG 证据共用模块，保持旧原生证据可读；
7. 把原生控制器收敛为批准的关键冒烟矩阵；
8. 运行完整快速视觉批次，逐张检查联合对照图并修正正式产品；
9. 最终只对批准的原生关键链路做双视口验证；
10. 更新迁移台账与验证文档，明确浏览器视觉证据和原生冒烟证据的不同含义。

## 12. 风险与控制

### 浏览器与 Tauri 渲染存在差异

字体栅格、窗口标题栏和系统对话框不能由浏览器视觉层证明，因此保留关键原生冒烟。产品内部 WebView 的组件布局、颜色、间距和响应式状态由快速视觉层高频覆盖。

### 状态入口与真实产品分叉

验收场景不得复制正式 JSX 或 CSS；状态目录测试记录正式组件覆盖关系。组件公开接口变化会让场景编译失败，而不是继续展示旧样机。

### 测试代码误入发布物

独立 Vite 配置、生产 manifest 门禁和 Tauri 包内容检查形成三重隔离。任何验收标识进入正式 bundle 都是阻塞性失败。

### 截图很多但审查不足

批量截图只是收集证据。最终仍必须打开联合对照图，按状态记录可见差异和裁决；不把自动生成数量当作视觉通过。

## 13. 验收结论格式

最终报告必须分别陈述：

- 快速视觉状态：通过数量、失败数量、未审数量及双视口证据根；
- 原生冒烟：通过链路、失败链路、使用的 Viewer commit 和窗口尺寸；
- 视觉裁决：P0/P1/P2 数量及对应状态；
- 自动测试：Vitest、Playwright 编排器、发布隔离、UI build 和原生控制器测试结果；
- 限制：当前只完成 macOS 原生冒烟，Windows 原生行为待 Windows 版本阶段验证。

只有快速视觉 89 × 2 无未解决 P0/P1/P2、批准的原生冒烟通过、发布隔离通过且证据属于同一最终产品 commit，才能声明本轮 Viewer 完整视觉迁移完成。
