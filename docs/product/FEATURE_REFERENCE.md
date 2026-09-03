# Viewer 0.1.7 详细功能参考

> Status: Current
>
> 当前开发版本：`0.1.7`

本文按功能域记录 Viewer 0.1.7 的实际行为、边界和维护证据。这里的“当前可用”表示代码
和自动化测试已经覆盖主要流程；“部分可用”表示核心流程存在，但结果还受本地文件内容、
权限、原生环境或操作种类限制。使用步骤见[用户指南](USER_GUIDE.md)，格式识别规则见
[支持格式与限制](SUPPORTED_FORMATS.md)。

## 1. 项目与会话生命周期

**状态：** 当前可用。

**用户行为：** 用户通过系统目录选择器或拖入一个文件夹建立项目。Viewer 一次只维护一
个活动项目；关闭项目、窗口或应用时结束对应会话，并停止扫描、监听和派生任务。

**边界与失败：** 单个文件不能作为项目。已有项目时不能直接叠加打开另一个项目。项目
不会在下次启动时自动恢复；活动文件操作会阻止无条件关闭，已经提交的修改不会回滚。

**维护证据：** `ui/src/components/EmptyProject.tsx`、`src-tauri/src/commands/project.rs`、
`crates/viewer-application/src/project.rs`；测试见 `ui/src/components/EmptyProject.test.tsx`、
`ui/src/App.test.tsx` 和 `src-tauri/src/commands/project.rs`。

## 2. 扫描与目录投影

**状态：** 当前可用。

**用户行为：** 打开项目后递归扫描目录，文件夹可以先于全部文件与派生表示显示。分类目录
投影为内容文件夹概览，内容目录投影为图片、视频和其他文件；也可显示全部后代文件。

**边界与失败：** 点开头的条目、`Thumbs.db` 和 `Desktop.ini` 不进入产品投影。扩展名只
决定候选分类，不保证内容可解析。单项读取失败被隔离，不应使整个项目不可浏览。

**维护证据：** `crates/viewer-infrastructure/src/scan/walker.rs`、
`crates/viewer-infrastructure/src/scan/file_classifier.rs`、`src-tauri/src/commands/browse.rs`；
测试见 `crates/viewer-infrastructure/src/scan/file_classifier.rs` 和 `ui/src/App.test.tsx`。

## 3. 内容布局与渐进缩略图

**状态：** 当前可用。

**用户行为：** 图片使用保留宽高比的虚拟化网格；视频使用独立分区；文本和其他文件使用
自适应面板。缩略图按可见范围请求，五档缩略图大小会改变浏览密度。

**边界与失败：** 缩略图失败只影响对应卡片，图片或视频的其他能力仍可独立尝试。快速滚动
和导航会取消过时请求；晚到结果只有在会话、代次和请求身份仍匹配时才会发布。

**维护证据：** `ui/src/components/AspectVirtualGrid.tsx`、
`ui/src/components/contentBrowser/VideoSection.tsx`、`src-tauri/src/commands/browse.rs`；测试见
`ui/src/components/AspectVirtualGrid.test.tsx`、`ui/src/components/contentBrowser/VideoCard.test.tsx`。

## 4. 文件选择

**状态：** 当前可用。

**用户行为：** 支持单击、Command 增减、Shift 范围、框选和当前内容全选。混合内容中全选
会先要求明确图片、视频、其他文件或全部范围；活动项用于键盘导航和径向菜单定位。

**边界与失败：** 范围选择以当前投影和锚点为准，目录或搜索范围变化会重新校准选择。预览、
对话框或文本控件拥有焦点时，全局选择快捷键不会抢占输入。

**维护证据：** `ui/src/components/ContentBrowser.tsx`、
`ui/src/components/contentBrowser/contentSelection.ts`、`ui/src/components/marqueeSelection.ts`；
测试见对应的 `ContentBrowser.test.tsx`、`contentSelection.test.ts` 和 `marqueeSelection.test.ts`。

## 5. 搜索、筛选与排序

**状态：** 当前可用。

**用户行为：** 可在全项目或目录子树中搜索名称、路径和已建立索引的文本正文，并按文件类型、
审阅状态、收藏、未标记、图片方向、像素尺寸、文件大小和修改时间筛选。结果支持相关度、自然
名称、修改时间、大小、像素尺寸或审阅状态排序，以及分组或平铺布局。

**边界与失败：** 输入采用 120 ms 防抖，单页最多请求 200 项。正文命中依赖文本索引；扫描
修订变化会拒绝过时搜索结果。清空搜索和筛选后返回原目录上下文。

**维护证据：** `ui/src/components/SearchToolbar.tsx`、
`ui/src/state/controllers/useSearchController.ts`、`src-tauri/src/commands/search.rs`；测试见
`ui/src/components/SearchToolbar.test.tsx`、`ui/src/components/SearchResults.test.tsx`。

## 6. 审阅状态与收藏

**状态：** 当前可用。

**用户行为：** 一个或多个文件可设置“保留”“待定”“淘汰”，可清除审阅状态，也可独立切换
收藏。目录概览汇总后代文件的审阅进度；搜索可用这些标记过滤。

**边界与失败：** 只读项目、活动文件操作或过期会话禁止写入。批量写入作为一个操作批次发布，
部分状态只在当前投影刷新后可见。审阅与收藏可在当前会话撤销。

**维护证据：** `ui/src/components/MarkerControls.tsx`、`src-tauri/src/commands/markers.rs`、
`crates/viewer-infrastructure/src/portable/markers.rs`；测试见 `MarkerControls.test.tsx`、
`FolderOverview.test.tsx` 和 `src-tauri/src/commands/markers.rs`。

## 6.1 持续评审、手动存档与 Agent 交接

**状态：** 当前可用；图片区域工作台和本地 current/history 读取已通过原生多轮验收。

**用户行为：** 用户在大图中通过统一“标记”菜单选择点、箭头、画笔、矩形或椭圆，也可用整图意见记录
自然语言返工要求。点用于精确位置，箭头从说明起点拖向问题点，画笔用于不规则轮廓，矩形和椭圆用于区域；
按住 Shift 绘制椭圆会按源图像素比例约束成视觉圆。作者态保存成功后
界面立即保留意见和标记；后台生成并验证 Agent 数据，界面显示“已保存，可供外部读取”后，
当前 Review Stream 的完整 current 才可由外部读取器消费，不需要“完成本轮”。保存后当前标记工具保持不变，
便于连续添加同类意见；当前文字和几何仍可编辑、重绘或删除，已存档历史保持不可变。收到返工素材后，用户手动预览并存档精确意见版本；部分存档
保留未选目标及后来新增／修改的意见。历史可以查看证据、继续提出或显式恢复，原记录不改写。

**边界与失败：** 网格浏览和大图浏览都不生成“已通过”事实；空 current 不代表全部通过。
存档不证明 Agent 已读取、执行、修复或获得用户验收。可选返工依据声明不证明 lineage；无
声明也能完成保存、读取和存档。源文件换版、历史继续或恢复都按精确素材版本失败关闭，需要
用户确认候选与位置。只读、未来协议、迁移未完成、保存中断、发布待处理或发布失败均保留
类型化状态；`publication_pending` 不返回旧 current，也不从时间戳、目录或路径猜测结果。
当前没有视频时间段标注和内置 Agent 执行器。

**维护证据：** `ui/src/components/review/`、`ui/src/app/review/`、
`crates/viewer-application/src/review_workspace/`、`crates/viewer-infrastructure/src/review/continuous/`、
`src-tauri/src/commands/review_workspace.rs`；验证见
[`../quality/2026-08-27-continuous-review-validation.md`](../quality/2026-08-27-continuous-review-validation.md)。

## 7. 图片预览与放大镜

**状态：** 当前可用。

**用户行为：** JPEG/PNG 可进入适合窗口的单图预览，支持缩放、平移、顺逆时针旋转、前后
导航和恢复适合窗口。放大镜可用 Q 开关，并按设置使用圆形或圆角矩形、2×/3×/4×和三档面积。
在图片评审工作台中，点、箭头、画笔轨迹、矩形、椭圆、意见编号和正在绘制的草稿会与图片使用同一
取样点、缩放和旋转投影，一起出现在放大镜内。

**边界与失败：** 原图请求失败时显示局部错误，不使用模糊缩略图冒充原图。图片切换会撤销
旧请求；关闭预览会重置预览变换和放大镜会话状态。镜片和镜片内的评审覆盖层不接收指针
操作；用户仍通过镜片外的普通标记进行选择、绘制和编辑。覆盖层绘制失败时退化为纯图片
放大镜，不改变意见保存、存档或 Agent 读取数据。

**维护证据：** `ui/src/components/ImagePreview.tsx`、`ui/src/components/imagePreview/ImageMagnifier.tsx`、
`ui/src/components/imagePreview/magnifierGeometry.ts`、`ui/src/components/review/annotationScene.ts`、
`src-tauri/src/commands/browse.rs`；测试见 `ui/src/components/ImagePreview.test.tsx`、
`ui/src/components/imagePreview/ImageMagnifier.test.tsx` 和视觉验收状态 `RVW-33`～`RVW-37`。

## 8. 文本预览

**状态：** 当前可用。

**用户行为：** 一到两个 Markdown/TXT 文件可只读预览。每栏可以独立选择 UTF-8、UTF-16
LE、UTF-16 BE 或 GB18030；Markdown 表格和删除线可以呈现，本地图片通过受控会话 URL 加载。

**边界与失败：** 每个文件最多读取前 10 MiB，超出部分会标记截断。无法自动判定的编码要求
用户选择。原始 HTML、未解析图片和远程图片不会进入预览；文本不能在 Viewer 内编辑或保存。

**维护证据：** `ui/src/components/TextPreview.tsx`、`src-tauri/src/commands/preview.rs`、
`src-tauri/src/markdown.rs`、`crates/viewer-infrastructure/src/text/preview.rs`；测试见
`ui/src/components/TextPreview.test.tsx` 和 `src-tauri/src/markdown.rs`。

## 9. 多图对比

**状态：** 当前可用。

**用户行为：** 选择 2–8 张图片进入对比，布局按可用空间和图片比例计算。用户可在同步与独立
视图间切换，对各窗格缩放、平移、适合窗口、旋转、移除，并直接修改审阅标记。

**边界与失败：** 不满足数量或存在活动文件操作时拒绝进入。单窗格原图失败不会清除其他窗格；
不可预览图片会停留在明确失败状态。自动化主要验证布局算法和 UI 合同，原生大图体验仍取决于机器。

**维护证据：** `ui/src/components/CompareWorkspace.tsx`、`ComparePane.tsx`、
`compareLayoutEngine.ts`、`ui/src/state/comparePolicy.ts`；测试见对应的 `.test.tsx`/`.test.ts` 文件。

## 10. 视频浏览与播放

**状态：** 部分可用。

**用户行为：** 候选视频在浏览区显示时长和封面；打开后进入准备、就绪、播放、暂停、结束或错误
状态。支持时间轴定位、逐帧前后、音量/静音、0.5×–2×速度、全屏和前后视频导航。结束后再次
点击播放会从头重播。

**边界与失败：** 扩展名不是解码保证。封面失败不等于视频不可播放；损坏、缺失、不可读、引擎
初始化、解码回退和渲染表面错误分别隔离。主播放使用本地原生表面，行为仍受打包媒体运行时影响。

**维护证据：** `ui/src/components/VideoPreview.tsx`、`ui/src/components/videoPreview/`、
`src-tauri/src/commands/video.rs`、`src-tauri/src/video_runtime.rs`；测试见 `VideoPreview.test.tsx`、
`crates/viewer-application/tests/video_preview_service.rs` 和 `viewer-platform-macos/tests/`。

## 11. 文件操作与冲突处理

**状态：** 当前可用。

**用户行为：** 支持单项/批量重命名、复制、移动和移入系统废纸篓。重命名先预览目标；复制和
移动先选择项目内目的地；冲突必须逐项选择跳过、两者都保留或替换。

**边界与失败：** 操作只接受当前项目内经过身份校验的源和目的地，不静默覆盖。只读项目禁用
修改。批次可出现成功、跳过、失败和取消的混合结果；已经完成的项不会因取消而反向修改。

**维护证据：** `ui/src/components/RenameDialog.tsx`、`BatchRenameDialog.tsx`、
`DestinationDialog.tsx`、`src-tauri/src/commands/operations.rs`；测试见对应组件测试和
`crates/viewer-infrastructure/src/operation/` 下的测试。

## 12. 撤销与操作恢复

**状态：** 部分可用。

**用户行为：** Command+Z 撤销当前会话最近一个可安全反向的重命名、移动、审阅状态或收藏批次。
启动时的操作恢复会依据操作日志把中断状态收敛为可报告结果，并重建必要投影。

**边界与失败：** 复制和移入废纸篓不自动撤销。撤销栈只属于当前项目会话；源身份改变、恢复位置
被占用、越出项目或项目只读时拒绝执行，避免覆盖后来发生的外部变化。

**维护证据：** `crates/viewer-application/src/undo.rs`、
`crates/viewer-infrastructure/src/operation/recovery.rs`、`src-tauri/src/operation_runtime/undo.rs`；
测试见 `crates/viewer-application/src/undo.rs` 和 `ui/src/App.test.tsx`。

## 13. Finder 拖出

**状态：** 当前可用，受原生环境限制。

**用户行为：** 从文件卡片的拖动把手开始，可把当前选择作为真实本地文件拖到 Finder 或其他接收
文件 URL 的 macOS 应用；未选择的活动项会先成为拖动选择。

**边界与失败：** 只导出文件引用，不上传或复制到 Viewer 管理空间。Web 测试环境没有原生拖动
能力时会明确返回不可用；源身份或授权失效时拒绝开始。

**维护证据：** `ui/src/components/OrganizationDragPreview.tsx`、
`src-tauri/src/commands/finder_drag.rs`、`crates/viewer-platform-macos/src/files/drag.rs`；测试见
`OrganizationDragPreview.test.tsx` 和 `crates/viewer-application/src/finder_drag.rs`。

## 14. 外部变化协调

**状态：** 当前可用。

**用户行为：** 项目打开后监听 Finder 和其他应用产生的变化，并对受影响范围重新协调扫描、索引、
选择和预览。当前文件消失时，界面尝试移动到邻近仍存在的项或退出失效上下文。

**边界与失败：** 文件系统通知只是提示，不直接当作真实索引；Viewer 会重新读取和验证。变化过大、
路径不明确或修订过期时扩大重扫范围，而不是应用可能错误的局部补丁。

**维护证据：** `crates/viewer-application/src/watcher.rs`、
`crates/viewer-infrastructure/src/scan/reconcile_service.rs`、`src-tauri/src/watcher_runtime.rs`；测试见
`ui/src/App.test.tsx` 和扫描协调模块测试。

## 15. 只读与错误隔离

**状态：** 当前可用。

**用户行为：** 根目录可读但不可写时以只读项目打开，浏览、搜索和预览仍可使用；横幅提供权限
设置和重新选择目录。错误按验证、冲突、环境、内容、一致性或内部类别映射为可恢复提示。

**边界与失败：** 只读状态禁止标记和文件修改。内容错误尽量停留在单卡片、单窗格或单视频；项目
级权限/一致性错误才要求重新选择或关闭项目。用户消息不直接暴露内部路径和诊断细节。

**维护证据：** `ui/src/components/ReadOnlyBanner.tsx`、`src-tauri/src/error.rs`、
`crates/viewer-application/src/project.rs`；测试见 `ReadOnlyBanner.test.tsx`、`App.test.tsx` 和
`src-tauri/src/error.rs`。

## 16. 设置与任务反馈

**状态：** 当前可用。

**用户行为：** 设置提供五档缩略图大小、放大镜形状/倍数/面积，以及视频缓存统计与确认清理。
扫描、文本索引、文件操作等后台工作在任务区域报告等待、运行、完成、失败或取消状态。

**边界与失败：** 设置写入失败时保留当前可用状态并允许重试。视频缓存预算为 1 GiB，清理只针对
拥有 Viewer 标记的缓存目录。只有声明为可取消的后台任务才显示取消入口。

**维护证据：** `ui/src/components/SettingsDialog.tsx`、`TaskBar.tsx`、
`crates/viewer-application/src/settings.rs`、`src-tauri/src/commands/video.rs`；测试见
`SettingsDialog.test.tsx`、`TaskBar.test.tsx` 和 `crates/viewer-infrastructure/tests/video_cache.rs`。

## 17. 可访问性与输入所有权

**状态：** 当前代码和 Web 自动化覆盖主要结构；原生辅助技术仍需实机验证。

**用户行为：** 主要按钮、对话框、菜单、进度和错误具有可访问名称或角色；对话框限制焦点并在关闭
后恢复入口。键盘命令按界面上下文分配，文本输入、输入法组合和修饰键会阻止单键命令误触发。

**边界与失败：** 自动化测试能验证 DOM 角色、焦点和键盘路由，但不能替代每个 macOS/VoiceOver
版本的人工验收。原生视频表面与 Web 控件的组合焦点是持续验证边界。

**维护证据：** `ui/src/components/ModalSheet.tsx`、`ui/src/App.tsx`、
`ui/src/state/useReviewShortcuts.ts`；测试见 `ModalSheet.test.tsx`、`useReviewShortcuts.test.tsx`、
`scripts/viewer-native-acceptance-focus-safety.test.mjs`。
