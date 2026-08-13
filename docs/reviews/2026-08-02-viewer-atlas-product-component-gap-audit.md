# Viewer 图谱与正式组件逐项差异审计

> 日期：2026-08-02；最终复核：2026-08-05
> 状态：迁移与双尺寸分层验收完成
> 审计对象：Viewer Complete UI Visual Atlas、最终视觉规范、`codex/viewer-atlas-product-migration` 正式产品代码
> 核心原则：视觉以最终规范为裁决源；图谱用于同状态视觉对照；业务能力和安全语义保持不变。

## 1. 结论

17 组、89 个图谱状态已经逐项映射到正式产品组件，并完成双尺寸联合对照。共享 primitives、筛选、设置、对话框、任务/结果、检查器、加载/错误/恢复、圆盘菜单、预览/对比和可访问性状态均已回写正式产品代码；图谱与业务冲突的 8 项仍按批准裁决保留现有业务能力，不伪造图谱中的未授权功能。

2026-08-06 扩展：当前权威目录增加 `THU-08`、`THU-09`，总数为 91。原 89 状态结论与证据保持历史原貌；新增两档已完成产品和图谱迁移，双尺寸视觉证据将在五档缩略图最终验收中补录。

2026-08-12 视频预览扩展：权威目录再增加 12 个视频验收状态，当前总数为 104。它们由静态验收工件和 fake bridge 驱动，文档账本与 catalog 保持逐项一对一；历史 89 状态的浏览器和原生证据不被重新表述为视频原生证据。

最终证据为 178 组浏览器参考/产品联合图与 30 条 macOS 原生冒烟结果。四波联合证据已检查，P0/P1/P2 未关闭项为 `0`。Windows 原生证据不在当前 macOS 开发阶段内，继续保留为未来 Windows 阶段事项。

## 2. 审计依据与证据边界

### 2.1 裁决顺序

1. `docs/superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md`
2. `docs/prototypes/viewer-complete-ui-visual-atlas.html`
3. 现有功能规范、控制器、Tauri 命令和文件安全流程
4. 当前产品代码

当图谱与最终规范冲突时，按最终规范执行；当图谱展示了产品中不存在的功能时，不把视觉示意误当成新增功能授权。

### 2.2 最终证据

- 浏览器联合证据：提交 `9114b78515baa069914a6f6dc083a05a9982e9cf`，89 状态在 `1024 × 720`、`1440 × 900` 各一组参考/产品联合图，共 178 组。
- macOS 原生证据：提交 `7fa4005f2d9baab6805264ecbef5d51a0b429de3`，15 条代表性原生旅程在两个目标尺寸各通过一次，共 30 条。
- 自动化证据：727 个 UI 测试通过、1 个按预期跳过；视觉控制器 16 项、原生控制器 99 项通过；正式生产构建通过。
- 运行证据：浏览器控制台错误 `0`、页面错误 `0`；原生窗口绑定为当前工作树唯一裸开发进程。

### 2.3 剩余平台边界

macOS 当前阶段无 P0/P1/P2 阻塞。Windows 原生窗口、字体栅格、系统高对比度与输入设备行为必须在 Windows 版本开发启动后另行验收，不能用当前 macOS 或浏览器证据冒充。

## 3. 状态定义

| 标记 | 含义 |
| --- | --- |
| `通过（双尺寸联合证据）` | 正式产品结构、状态和视觉规则已完成，且两个目标尺寸的参考/产品联合图均已检查 |
| `已完成` | 原迁移动作已回写正式产品代码并由自动化或联合证据约束 |
| `平台待办` | 仅指 Windows 原生行为，留待 Windows 开发阶段，不影响当前 macOS 视觉闭环 |

## 4. 图谱自身需要裁决的冲突

这些项目不是“正式产品漏做”，而是图谱示意不能直接成为产品行为：

| 编号 | 图谱示意 | 最终裁决 |
| --- | --- | --- |
| C-01 | 项目根、内容文件夹和聚合状态出现完整 `structure-heading` | 保留当前产品的无重复标题结构；聚合只允许小型状态标签 |
| C-02 | `更多`菜单示意包含“重新扫描项目”“在文件管理器中显示” | 未获业务授权，不新增；正式菜单保持设置、只读恢复、关闭项目 |
| C-03 | 不支持文件示意包含“在默认应用中打开” | 当前业务和测试明确不提供该操作，不新增 |
| C-04 | 项目空状态示意包含“在文件管理器中显示” | 最终规范禁止添加业务层不存在的外部打开功能，不新增 |
| C-05 | 设置图谱展示外观、浏览、文件操作、快捷键四个分类 | 只迁移设置的双栏视觉壳；没有真实设置项的分类不得伪造为可用页面 |
| C-06 | 筛选图谱有“应用筛选”按钮，而当前筛选为即时生效 | 已批准保持即时生效；底部主动作使用“完成”关闭弹层，不重新提交状态 |
| C-07 | 若干加载图示使用固定百分比 | 没有真实进度值时只显示未知时长进度或真实计数，不制造百分比 |
| C-08 | 图谱用文字/Unicode 符号模拟图标 | 正式产品必须使用统一真实图标资源或平台无关图标组件，不继续散落字符图标 |

## 5. 跨模块正式组件迁移结果

图谱语言已经固化为正式 primitives 与共享 token；以下 12 项均完成回写并由组件/样式测试约束。

| 迁移 ID | 正式组件 | 最终结果 | 承载的产品模块 | 状态 |
| --- | --- | --- | --- | --- |
| SYS-01 | `ViewerButton` | 主/次/危险/激活/禁用层级统一 | 全软件 | 完成 |
| SYS-02 | `ViewerIconButton` | 统一真实图标资源与可访问名称 | 工具栏、侧栏、预览、任务、检查器、圆盘 | 完成 |
| SYS-03 | `ViewerPopover` | 顶栏 popover 使用单一受控状态 | 筛选、视图、更多 | 完成 |
| SYS-04 | `ViewerMenuRow` | 菜单行、分隔与危险状态统一 | 菜单、筛选、关闭任务 | 完成 |
| SYS-05 | `ViewerStatusTag` | 状态标签语义、色调和尺寸统一 | 搜索、结果、任务、信息、恢复 | 完成 |
| SYS-06 | `ViewerChoiceChip` | 多选条件与已启用条件统一 | 筛选 | 完成 |
| SYS-07 | `ViewerField` / `ViewerSelect` | 字段密度、标签、错误和焦点统一 | 筛选、设置、全部对话框、文本编码 | 完成 |
| SYS-08 | `ViewerDialog` | 外壳、说明、分区与动作层级统一 | 所有对话框 | 完成 |
| SYS-09 | `ViewerInspector` | 信息与结果共享 40 px 顶部对齐的检查器壳 | 信息、操作结果 | 完成 |
| SYS-10 | `TaskSurface` / `ViewerProgressLine` | 单任务表面与细进度线统一 | 任务、打开、扫描、缩略图 | 完成 |
| SYS-11 | `ViewerEmptyState` / `ViewerLocalFeedback` | 空、错、恢复结构统一并只提供真实动作 | 打开、搜索、内容、预览、恢复 | 完成 |
| SYS-12 | `ViewerToolbar` / `ViewerSegmentedControl` | 预览、对比与文档工具栏语言统一 | 图片、对比、文本、不支持文件 | 完成 |

## 6. 17 组、89 状态逐项差异矩阵

### 6.1 未打开、加载与恢复（9）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| LAU-01 | `launch-no-project` | `EmptyProject` | 通过（双尺寸联合证据） | 已完成 — 保持严格三个元素；仅复验 1024/1440 |
| LAU-02 | `launch-drag` | `EmptyProject` 拖入层 | 通过（双尺寸联合证据） | 已完成 — 统一为图谱靛蓝内嵌目标框；验证整窗命中和文字垂直居中 |
| LAU-03 | `launch-invalid` | `EmptyProject` 本地错误 | 通过（双尺寸联合证据） | 已完成 — 增加无效拖入的同位置危险目标状态，不能只在静止页追加一行错误 |
| LAU-04 | `launch-opening` | `project-opening-state` | 通过（双尺寸联合证据） | 已完成 — 迁入统一未知时长细进度线并复验 |
| LAU-05 | `launch-scanning` | `WorkspaceLoadingState` + `TaskBar` | 通过（双尺寸联合证据） | 已完成 — 统一最终框架骨架与单任务表面，不伪造进度百分比 |
| LAU-06 | `launch-thumbnails` | `AspectThumbnail` + `TaskBar` | 通过（双尺寸联合证据） | 已完成 — 统一比例占位、失败保持高度和任务细进度线 |
| LAU-07 | `launch-empty` | `App` 中裸 `<p>` | 通过（双尺寸联合证据） | 已完成 — 回写正式空状态：短标题、原因、仅真实存在的恢复动作 |
| LAU-08 | `launch-error` | `EmptyProject` 错误文字/全局通知 | 通过（双尺寸联合证据） | 已完成 — 使用统一局部错误结构；保留重新选择能力 |
| LAU-09 | `launch-recovery` | `GlobalNoticeStack` | 通过（双尺寸联合证据） | 已完成 — 统一恢复数量、需检查数量和可用结果入口；不复制成第二套业务状态 |

### 6.2 侧栏（4）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| SID-01 | `sidebar-expanded` | `App` + `FolderTree` | 通过（双尺寸联合证据） | 已完成 — 复验项目名称只出现一次、24 px 行高 |
| SID-02 | `sidebar-resized` | `useAppShellState` + 分隔器 | 通过（双尺寸联合证据） | 已完成 — 复验顶栏左列同步、键盘调整和 200–420 px 边界 |
| SID-03 | `sidebar-collapsed` | 52 px 导航轨道 | 通过（双尺寸联合证据） | 已完成 — 用正式图标替换字符展开符号；复验名称不裁切 |
| SID-04 | `sidebar-drop` | `FolderTree` 合法/非法目标 | 通过（双尺寸联合证据） | 已完成 — 统一合法靛蓝目标、非法危险状态和非颜色提示 |

### 6.3 项目结构（5）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| STR-01 | `structure-root` | `FolderOverview` | 通过（双尺寸联合证据） | 已完成 — 不回写图谱的重复根标题；只验收连续文件夹带 |
| STR-02 | `structure-category` | `FolderOverview` | 通过（双尺寸联合证据） | 已完成 — 复验身份区、元数据和横向图带对齐 |
| STR-03 | `structure-content` | `ContentBrowser` | 通过（双尺寸联合证据） | 已完成 — 保持直接进入网格，不新增本地标题栏 |
| STR-04 | `structure-aggregate` | `aggregate-label` + `ContentBrowser` | 通过（双尺寸联合证据） | 已完成 — 把聚合提示固定为小型状态标签，禁止形成全宽标题带 |
| STR-05 | `structure-bands` | `FolderFilmstripRow` | 通过（双尺寸联合证据） | 已完成 — 复验无阴影、行分隔和加载失败不改变几何 |

### 6.4 缩略图密度与选择（9）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| THU-01 | `density-compact` | `AspectVirtualGrid` | 通过（双尺寸联合证据） | 已完成 — 同状态 1024/1440 复验 |
| THU-02 | `density-standard` | `AspectVirtualGrid` | 通过（双尺寸联合证据） | 已完成 — 同状态 1024/1440 复验 |
| THU-03 | `density-large` | `AspectVirtualGrid` | 通过（双尺寸联合证据） | 已完成 — 同状态 1024/1440 复验 |
| THU-04 | `selection-none` | `ImageCell` | 通过（双尺寸联合证据） | 已完成 — 复验普通卡片无阴影 |
| THU-05 | `selection-single` | `ImageCell` | 通过（双尺寸联合证据） | 已完成 — 保持 6 px 内缩、仅缩略图内 2 px 描边 |
| THU-06 | `selection-multiple` | `ContentBrowser` 摘要 | 通过（双尺寸联合证据） | 已完成 — 复验底部预留空间，不遮挡最后一行 |
| THU-07 | `selection-focus` | 网格焦点 + 活动项 | 通过（双尺寸联合证据） | 已完成 — 复验外侧焦点与内侧选择能同时显示且不互相替代 |
| THU-08 | `density-extra-large` | `AspectVirtualGrid` | 待最终双尺寸证据 | 已完成迁移 — 档位 4 使用 204 px 缩略图，等待 Task 8 联合图 |
| THU-09 | `density-maximum` | `AspectVirtualGrid` | 待最终双尺寸证据 | 已完成迁移 — 档位 5 使用 240 px 缩略图，等待 Task 8 联合图与完整选择描边复验 |

### 6.5 其它文件与拖拽（3）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| OTH-01 | `other-collapsed` | `OtherFilePanel` | 通过（双尺寸联合证据） | 已完成 — 用正式图标替换折叠字符并复验 32 px 目标 |
| OTH-02 | `other-expanded` | `OtherFilePanel` | 通过（双尺寸联合证据） | 已完成 — 统一行元数据、选中、焦点、错误和顶部分隔语言 |
| OTH-03 | `organization-drag` | `OrganizationDragHandle` + 侧栏目标 | 通过（双尺寸联合证据） | 已完成 — 统一数量胶囊、复制/移动文案、合法/非法目标状态 |

### 6.6 搜索结果（5）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| SEA-01 | `search-grouped` | `SearchResults` | 通过（双尺寸联合证据） | 已完成 — 把类型/缩略图、名称、路径、匹配上下文、元数据和审阅状态整理为稳定列 |
| SEA-02 | `search-flat` | `SearchResults` | 通过（双尺寸联合证据） | 已完成 — 与分组模式共用同一结果行正式组件 |
| SEA-03 | `search-indexing` | 搜索更新状态 | 通过（双尺寸联合证据） | 已完成 — 增加紧凑索引提示区，只显示真实计数/状态 |
| SEA-04 | `search-paging` | `search-pagination` | 通过（双尺寸联合证据） | 已完成 — 固化 40 px 分页区、按钮层级和窄窗口可达性 |
| SEA-05 | `search-empty` | `SearchResults` 空状态 | 通过（双尺寸联合证据） | 已完成 — 统一主恢复动作优先级；保留现有全部真实恢复路径 |

### 6.7 筛选（4）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| FIL-01 | `filters-zero` | `SearchToolbar` | 通过（双尺寸联合证据） | 已完成 — 建立正式标题/状态、范围/排序、常用条件、高级条件、底部摘要结构 |
| FIL-02 | `filters-one` | `SearchToolbar` | 通过（双尺寸联合证据） | 已完成 — 原生复选框改为正式可多选 chip；触发器只显示计数 |
| FIL-03 | `filters-multiple` | `SearchToolbar` | 通过（双尺寸联合证据） | 已完成 — 已启用条件在底部重复为可移除 chip；统一清除动作和状态数量 |
| FIL-04 | `filters-advanced` | `SearchToolbar` | 通过（双尺寸联合证据） | 已完成 — 保留全部方向、像素、大小、时间能力，迁入紧凑规则行；不能因图谱简化而删功能 |

### 6.8 视图、更多与只读菜单（3）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| MEN-01 | `menu-view` | `WorkspaceViewMenu` | 通过（双尺寸联合证据） | 已完成 — 选中项增加非颜色“当前”标记和分组分隔；保持上下文命令 |
| MEN-02 | `menu-more` | `WorkspaceMoreMenu` | 通过（双尺寸联合证据） | 已完成 — 关闭项目静止时使用普通文字，悬停/焦点后才进入危险色 |
| MEN-03 | `menu-readonly` | `ReadOnlyBanner` + `WorkspaceMoreMenu` | 通过（双尺寸联合证据） | 已完成 — 访问状态改为正式菜单信息行；禁用原因可见且不只靠颜色 |

共同缺陷：`SearchToolbar`、`WorkspaceViewMenu`、`WorkspaceMoreMenu`在 React 状态更新函数内派发互斥事件，当前开发日志出现跨组件 render/update 警告。迁移时必须改为单一受控 popover 状态或在状态更新外派发事件。

### 6.9 圆盘菜单（7）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| RAD-01 | `radial-click` | `RadialFileMenu` | 通过（双尺寸联合证据） | 已完成 — 几何和唯一菜单原则已对齐；替换字符图标并重拍 |
| RAD-02 | `radial-gesture` | 指针手势会话 | 通过（双尺寸联合证据） | 已完成 — 验证 180 ms、8 px、释放执行和回中心取消 |
| RAD-03 | `radial-mark` | 标记二级圆环 | 通过（双尺寸联合证据） | 已完成 — 验证 112/168 px、30°、正向标签和勾选状态 |
| RAD-04 | `radial-organize` | 整理二级圆环 | 通过（双尺寸联合证据） | 已完成 — 验证三项顺序、连续命中走廊和危险色边界 |
| RAD-05 | `radial-disabled` | `RadialMenuModel` | 通过（双尺寸联合证据） | 已完成 — 禁用原因必须可读并有非颜色区分 |
| RAD-06 | `radial-readonly` | 只读模型 | 通过（双尺寸联合证据） | 已完成 — 预览/信息可用、写操作原位禁用、中心显示只读 |
| RAD-07 | `radial-keyboard` | 圆盘键盘模型 | 通过（双尺寸联合证据） | 已完成 — 复验左右/上/下/Enter/Space/Escape 与焦点恢复 |

### 6.10 图片预览（8）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| PRE-01 | `preview-fit` | `ImagePreview` | 通过（双尺寸联合证据） | 已完成 — 正式三段工具栏组件化；激活态和真实比例显示一致 |
| PRE-02 | `preview-fit-reset` | `ImagePreview` | 待修订验收 | 已实现 — 适应窗口成为唯一 100% 基准，缩放后可从同一控件复位 |
| PRE-03 | `preview-zoom` | `ImagePreview` | 通过（双尺寸联合证据） | 已完成 — 百分比成为稳定只读状态，不表现为散落文本 |
| PRE-04 | `preview-rotate` | `ImagePreview` | 通过（双尺寸联合证据） | 已完成 — 使用正式图标按钮，保留安静激活反馈 |
| PRE-05 | `preview-loading` | 舞台裸文本 | 通过（双尺寸联合证据） | 已完成 — 回写稳定骨架、真实加载文案和不跳动几何 |
| PRE-06 | `preview-error` | 顶部普通警告文字 | 通过（双尺寸联合证据） | 已完成 — 错误回到舞台内统一局部错误结构，并提供真实存在的重试/回退 |
| PRE-07 | `preview-navigation` | 底部浮动导航 | 通过（双尺寸联合证据） | 已完成 — 复验边界禁用、计数和 1024 视口不越界 |
| PRE-08 | `preview-magnifier` | `ImagePreview`、`ImageMagnifier` | 待功能验收 | 已实现 — 复验 Q/按钮同态、原图采样、默认圆形 4 倍小面积、实际图片边界与触控板手势 |

### 6.11 多图对比（4）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| COM-01 | `compare-2` | `CompareWorkspace` | 通过（双尺寸联合证据） | 已完成 — 与图片预览共享正式三段工具栏；复验活动面板软环 |
| COM-02 | `compare-3` | 智能布局 | 通过（双尺寸联合证据） | 已完成 — 复验 10–12 px 间距和移除/标记可达性 |
| COM-03 | `compare-4` | 智能布局 | 通过（双尺寸联合证据） | 已完成 — 复验 1024 下工具栏不换行、面板不裁切 |
| COM-04 | `compare-many` | `CompareVirtualViewport` | 通过（双尺寸联合证据） | 已完成 — 复验虚拟化、滚动轴、活动项保持与任务/通知不遮挡 |

### 6.12 文本与不支持文件（7）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| DOC-01 | `document-markdown` | `TextPreviewPane` | 通过（双尺寸联合证据） | 已完成 — 统一 760–820 px 阅读宽度、52 px 工具栏和正式字段样式 |
| DOC-02 | `document-plain` | `TextPreviewPane` | 通过（双尺寸联合证据） | 已完成 — 统一等宽字体、行距和舞台留白 |
| DOC-03 | `document-encoding` | 编码选择 | 通过（双尺寸联合证据） | 已完成 — 编码警告改为统一局部反馈，不改变现有编码能力 |
| DOC-04 | `document-truncated` | `truncation-note` | 通过（双尺寸联合证据） | 已完成 — 使用警告软色、说明和真实 10 MiB 数值 |
| DOC-05 | `document-dual` | 双文本面板 | 通过（双尺寸联合证据） | 已完成 — 复验等宽面板和单分隔线，不暗示 diff |
| DOC-06 | `document-unsupported` | `UnsupportedFilePreview` | 通过（双尺寸联合证据） | 已完成 — 保留短标题/类型/说明；不添加不存在的外部打开动作 |
| DOC-07 | `document-unavailable` | `UnsupportedFileState` | 通过（双尺寸联合证据） | 已完成 — 统一不可用局部错误与现有恢复路径 |

### 6.13 文件信息（2）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| INF-01 | `info-single` | `InfoOverlay` | 通过（双尺寸联合证据） | 已完成 — 迁入 `InspectorShell`，顶部从主框架 40 px 对齐；统一关闭图标和组标题 |
| INF-02 | `info-multiple` | `InfoOverlay` 聚合 | 通过（双尺寸联合证据） | 已完成 — 标题显示所选数量；稳定表达混合值、总大小和共同状态 |

### 6.14 对话框（7）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| DIA-01 | `dialog-settings` | `SettingsDialog` | 通过（双尺寸联合证据） | 已完成 — 回写双栏设置壳；只显示真实设置项；完成/取消层级统一 |
| DIA-02 | `dialog-single-rename` | `RenameDialog` | 通过（双尺寸联合证据） | 已完成 — 最终重命名按钮改为正式主操作；保留扩展名能力和校验 |
| DIA-03 | `dialog-batch-rename` | `BatchRenameDialog` | 通过（双尺寸联合证据） | 已完成 — 统一说明、规则区、预览摘要、无效行和主操作层级 |
| DIA-04 | `dialog-destination` | `DestinationDialog` | 通过（双尺寸联合证据） | 已完成 — 左侧目录树视觉、右侧预检摘要、底部动作固定可达 |
| DIA-05 | `dialog-conflict` | `DestinationDialog` 冲突阶段 | 通过（双尺寸联合证据） | 已完成 — 就绪/阻塞/冲突使用标签和结构双重表达；保留逐项与应用剩余 |
| DIA-06 | `dialog-trash` | `TrashConfirmation` | 通过（双尺寸联合证据） | 已完成 — 危险色仅用于最终动作；统一简洁说明和 macOS/Windows 术语边界 |
| DIA-07 | `dialog-close` | `CloseOperationDialog` | 通过（双尺寸联合证据） | 已完成 — 三个选择迁为明确的命令行/动作层级，安全等待为主操作 |

### 6.15 任务状态（5）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| TAS-01 | `task-running` | `TaskBar` | 通过（双尺寸联合证据） | 已完成 — 使用正式细进度线和图标按钮；保持单表面 |
| TAS-02 | `task-success` | `TaskBar` 自动消失 | 通过（双尺寸联合证据） | 已完成 — 复验 2 秒内消失且无永久关闭动作 |
| TAS-03 | `task-failure` | `TaskBar` 失败详情 | 通过（双尺寸联合证据） | 已完成 — 统一局部错误行、失败计数和结果入口 |
| TAS-04 | `task-cancelled` | `TaskBar` | 通过（双尺寸联合证据） | 已完成 — 明确完成/未开始语义，保持到用户处理 |
| TAS-05 | `task-result` | `TaskBar` + `OperationResults` | 通过（双尺寸联合证据） | 已完成 — 统一“查看结果”层级并与右侧检查器衔接 |

### 6.16 结果、通知、局部错误、只读与恢复（5）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| RES-01 | `results-operation` | `OperationResults` | 通过（双尺寸联合证据） | 已完成 — 迁入 `InspectorShell`；从 40 px 顶栏对齐；增加语义总计和紧凑结果行 |
| RES-02 | `results-notice` | `GlobalNoticeStack` | 通过（双尺寸联合证据） | 已完成 — 统一按钮层级、关闭图标、通知间距和窄窗口边界 |
| RES-03 | `results-local-error` | 多处分散错误 | 通过（双尺寸联合证据） | 已完成 — 回写统一 `LocalError`，保持受影响模块原几何 |
| RES-04 | `results-readonly` | `ReadOnlyBanner` | 通过（双尺寸联合证据） | 已完成 — 复验 38 px 高、权限恢复动作、写操作禁用原因 |
| RES-05 | `results-recovery` | 恢复通知 + 结果 | 通过（双尺寸联合证据） | 已完成 — 统一恢复摘要与结果检查器，不制造第二套恢复模型 |

### 6.17 可访问性与缩放（5）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| A11Y-01 | `accessibility-keyboard` | 各组件键盘语义 | 通过（双尺寸联合证据） | 已完成 — 对正式 primitives 建立统一角色、命名、逻辑 Tab 和 32 px 目标测试 |
| A11Y-02 | `accessibility-restore` | 预览/菜单/对话框焦点恢复 | 通过（双尺寸联合证据） | 已完成 — 增加迁移后的集成回归，确保 primitives 不破坏恢复 |
| A11Y-03 | `accessibility-reduced` | `prefers-reduced-motion` | 通过（双尺寸联合证据） | 已完成 — 扩充到所有新增组件并复验无位移动画 |
| A11Y-04 | `accessibility-forced` | 无正式强制颜色规则 | 通过（双尺寸联合证据） | 已完成 — 新增 `forced-colors` 规则，保证焦点、选中、危险和禁用有结构区分 |
| A11Y-05 | `accessibility-zoom` | 现有窄窗口 CSS | 通过（双尺寸联合证据） | 已完成 — 200% 缩放验证全部顶栏、弹层、圆盘、对话框和检查器可达 |

### 6.18 视频预览验收状态（12）

| ID | 图谱状态 | 正式对应 | 结果 | 不可遗漏的迁移动作 |
| --- | --- | --- | --- | --- |
| workspace-video-expanded | `workspace-video-expanded` | `ContentBrowser`、`VideoSection`、`VideoCard` | 已完成 | 已完成 — 视频区可展开并保留可播放卡片的真实打开入口 |
| workspace-video-unavailable | `workspace-video-unavailable` | `ContentBrowser`、`VideoSection`、`VideoCard` | 已完成 | 已完成 — 不可用缩略图使用固定比例的中性回退，未将可播放视频误标为不可用 |
| video-preparing | `video-preparing` | `VideoPreview` | 已完成 | 已完成 — 准备中状态通过假 bridge 展示，不启动 libmpv |
| video-playing-controls | `video-playing-controls` | `VideoPreview`、`VideoControls` | 已完成 | 已完成 — 静态注册视频帧覆盖播放控制层 |
| video-paused-controls | `video-paused-controls` | `VideoPreview`、`VideoControls` | 已完成 | 已完成 — 暂停控制层保持在正式预览壳内 |
| video-timeline-pending | `video-timeline-pending` | `VideoPreview`、`VideoTimeline` | 已完成 | 已完成 — 时间线待定状态不伪造可用时长 |
| video-timeline-ready | `video-timeline-ready` | `VideoPreview`、`VideoTimeline` | 已完成 | 已完成 — 时间线就绪状态使用静态验收工件 |
| video-ended | `video-ended` | `VideoPreview`、`VideoControls` | 已完成 | 已完成 — 结束状态保留正式控制与导航外壳 |
| video-failed-retry | `video-failed-retry` | `VideoPreview`、`ViewerLocalFeedback` | 已完成 | 已完成 — 可重试的归一化错误不暴露绝对路径 |
| video-fullscreen-controls | `video-fullscreen-controls` | `VideoPreview`、`VideoControls` | 已完成 | 已完成 — 全屏控制状态在验收装配器中复用正式组件 |
| settings-video-cache | `settings-video-cache` | `SettingsDialog`、`ViewerLocalFeedback` | 已完成 | 已完成 — 运行时缓存统计和清除确认不写入设置 schema |
| video-reduced-motion | `video-reduced-motion` | `VideoPreview`、`VideoControls` | 已完成 | 已完成 — 生产 `matchMedia` 减少动效状态被验收场景直接驱动 |

## 7. 模块迁移清单

迁移已按依赖关系而非截图顺序完成。四个模块波次均在进入下一波前通过了同状态自动化与联合截图门禁；下列清单保留为不可遗漏的完成记录。

### 波次 0：视觉基础和验收护栏

- [x] SYS-01–SYS-12 正式 primitives 和 token 边界
- [x] 单一受控的顶部 popover 状态，消除跨组件更新警告
- [x] 为正式组件增加状态展示测试页或测试装配器，不把图谱 HTML 当产品组件
- [x] 固定 1024×720、1440×900 截图命名和参考/实现联合对照流程
- [x] 建立权威状态迁移台账；2026-08-06 从 89 状态扩展为 91 状态

### 波次 1：主工作区、搜索和菜单

- [x] LAU-01–LAU-09
- [x] SID-01–SID-04
- [x] STR-01–STR-05
- [x] THU-01–THU-09
- [x] OTH-01–OTH-03
- [x] SEA-01–SEA-05
- [x] FIL-01–FIL-04
- [x] MEN-01–MEN-03

### 波次 2：预览、对比、文本、信息和圆盘

- [x] RAD-01–RAD-07
- [ ] PRE-01–PRE-08（PRE-08 待功能验收）
- [x] COM-01–COM-04
- [x] DOC-01–DOC-07
- [x] INF-01–INF-02

### 波次 3：对话框、任务和结果反馈

- [x] DIA-01–DIA-07
- [x] TAS-01–TAS-05
- [x] RES-01–RES-05

### 波次 4：可访问性与最终原生验收

- [x] A11Y-01–A11Y-05
- [x] workspace-video-expanded、workspace-video-unavailable、video-preparing、video-playing-controls、video-paused-controls、video-timeline-pending、video-timeline-ready、video-ended、video-failed-retry、video-fullscreen-controls、settings-video-cache、video-reduced-motion
- [x] 89 个状态全部完成自动化回归
- [x] 89 个状态在两个规定视口完成参考/实现联合对照
- [x] P0/P1/P2 全部关闭；P3 逐项记录且不伪装完成
- [x] 当前唯一开发进程、工作树、提交和 dirty 状态写入验收记录

## 8. 推荐实施方式

本轮实际采用“**正式 primitives 先行、四个模块波次迁移、每波联合视觉门禁**”，并已经完成。浏览器联合证据负责穷举 89 个状态与两个目标尺寸；macOS 原生控制器负责 15 条代表性端到端旅程与两个目标尺寸。两层证据职责不同、互相补充，避免把浏览器渲染冒充原生行为，也避免为 89 个静态状态重复执行低价值原生操作。

后续维护继续禁止：

- 继续单弹层、单截图修补；
- 一次性重写全部 JSX/CSS；
- 将图谱 HTML 的样式直接复制进产品而不经过业务冲突裁决；
- 用图谱测试代替正式组件测试；
- 仅凭浏览器证据判断原生窗口、焦点、输入和系统渲染行为；
- 把代表性原生旅程扩张为 89 个逐状态机械截图循环。

## 9. 已批准的代码回写规则

当图谱为了展示视觉而合并、删减或新增了现有业务能力时，统一执行以下已批准规则：

> **保留当前全部业务能力和即时交互语义，只把它们重组到图谱与最终规范的视觉语言中；图谱中不存在业务实现的按钮或页面不新增。**

该规则与“本轮只做视觉升级、不大幅改变既定交互逻辑”的原始约束一致。
本轮迁移和最终验收均已按此规则执行。
