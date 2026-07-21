# Viewer 内部整理 Pointer 拖拽设计

**日期：** 2026-07-20  
**状态：** 已批准，待实现  
**范围：** Viewer 0.1 / M3 内部文件移动与复制

## 1. 背景与根因

Finder 导出与图片框选通过真实打包应用验收后，`⋮⋮` 整理拖柄到左侧文件夹的默认移动仍未发生。直接磁盘证据显示源文件未消失、目标文件未出现；便携数据库的 `operation_batches` 与 `operation_items` 均没有新增行，因此故障发生在文件预检与执行之前。

当前窗口必须保持 Tauri 原生 drag-drop handler，以便 Finder 文件夹拖入 Viewer 时由 Rust 获得本机路径。Tauri/Wry 的该处理器与 WebView 完整 HTML5 drag-and-drop 会话互斥：原生处理器开启时，HTML5 `dragover`/`drop` 不能作为可靠的内部应用协议。卡片主体导出之所以成功，是因为它只使用最初的 `dragstart` 触发 Viewer 自有 AppKit 会话；现有内部整理却需要 HTML5 `DataTransfer` 从拖柄持续到文件夹，因此其架构前提不成立。

参考：

- [Tauri #14373](https://github.com/tauri-apps/tauri/issues/14373) 记录原生 handler 在 macOS 也会阻止 DOM drag-and-drop；
- [Tauri #8581](https://github.com/tauri-apps/tauri/issues/8581) 中维护者确认原生文件 drop 与 HTML 拖动组件的冲突是已知行为；
- [`tauri-runtime` WebviewAttributes](https://docs.rs/tauri-runtime/latest/tauri_runtime/webview/struct.WebviewAttributes.html) 说明禁用原生 handler 才能恢复前端 HTML5 drag-and-drop，但 Viewer 不能因此失去 Finder 文件夹路径导入。

## 2. 方案选择

采用 **Pointer Events 自定义内部整理拖拽**。`⋮⋮` 从按下开始由 Viewer 指针会话独占，文件夹命中、视觉反馈、自动滚动和释放提交全部在 React/DOM 指针层完成；不创建 HTML5 `DataTransfer`，也不改变 Tauri 原生文件导入开关。

不采用以下方案：

1. 关闭 Tauri 原生 drag-drop handler。HTML5 拖拽会恢复，但 WebView 无法安全获得 Finder 文件夹的本机路径，直接破坏已接受的项目导入流程。
2. 使用 AppKit 原生会话同时处理 Viewer 内部目标。它需要自定义 pasteboard 类型、WKWebView/React 坐标同步、原生目标路由和跨层文件夹命中，复杂度与 Viewer 0.1 风险过高。
3. 只保留“移动到/复制到”对话框。它可作为现有批量操作入口继续存在，但不能替代已经确认的拖柄生产力交互。

## 3. 三种拖动手势的唯一所有权

Viewer 对三个起点建立互不重叠的输入表面：

- **图片/文本文件主体：** 独立 `draggable` 导出表面，仅负责立即启动 copy-only AppKit Finder 导出；
- **图片网格空白：** Pointer Events 框选表面，仅负责选择；
- **`⋮⋮` 整理拖柄：** 非 HTML-draggable 的 Pointer Events 整理表面，仅负责 Viewer 内部移动/复制。

文件项外层 `role="option"` 不再承担 HTML `draggable`。图片预览、名称和标记包裹在专用导出表面中；文本名称、路径和标记同样包裹在专用导出表面中。整理拖柄是该表面的兄弟节点，不是其后代。这样拖柄 PointerDown 不可能回退到祖先卡片的 Finder `dragstart`。

拖柄设置 `draggable={false}`，并阻止任何意外冒泡的 `dragstart`。卡片单击、Command/Shift 选择、双击预览和可访问语义保持不变。

## 4. 会话模型与状态机

一个会话使用以下最小状态：

```ts
interface OrganizationPointerSession {
  pointerId: number
  entityIds: string[]
  mode: 'move' | 'copy'
  startClientX: number
  startClientY: number
  clientX: number
  clientY: number
  phase: 'armed' | 'dragging'
  targetFolderId: string | null
  targetValid: boolean
}
```

规则：

1. 仅主鼠标键可以启动。PointerDown 先执行现有 `freezeDragSelection`，按当前可见顺序冻结实体 ID；拖动未选项时先只选中该项。
2. PointerDown 同时冻结 Option：按下时为 copy，否则为 move；后续按键变化不修改模式。
3. 拖柄取得 Pointer Capture，调用 `preventDefault()` 与 `stopPropagation()`，会话进入 `armed`。
4. 指针移动达到 4 CSS px 后进入 `dragging` 并显示浮层；未达到阈值的 PointerUp 只作为普通拖柄点击结束，不执行文件命令。
5. `dragging` 中每次 PointerMove 通过 `document.elementFromPoint(clientX, clientY)` 查询当前真实元素，向上查找带有 `data-organization-folder-id` 的文件夹行。DOM 只携带不透明 folder entity ID，不携带路径。
6. App 使用现有 `isDropTargetValid(folderId, frozenMode)` 判断有效性。有效、无效与无目标状态分别渲染，不由 DOM 自行决定安全性。
7. 有效目标上的 PointerUp 先冻结会话值并清除视觉/捕获状态，再调用现有 `dropFiles(entityIds, folderId, mode)`；后续仍走同一 Rust 预检、冲突对话框、事务执行与 Watcher 对账。
8. 无目标或无效目标 PointerUp、PointerCancel、Escape、窗口失焦、项目/工作区替换和组件卸载都只取消，不调用文件命令。

同时只允许一个整理会话。文件操作繁忙、只读、比较视图或项目关闭期间不能启动；Rust 仍独立执行所有权限、身份和路径重验证。

## 5. 组件边界

### 5.1 `ContentBrowser`

- 渲染互斥的导出表面与整理拖柄；
- 冻结当前选择和启动模式；
- 持有捕获指针的 DOM 节点，并将 PointerDown/Move/Up/Cancel 规范化为不透明实体事件；
- 不查找文件夹、不调用文件命令、不接触路径。

### 5.2 `useOrganizationPointerDrag`

新增一个聚焦的 React hook，供 `App` 使用：

- 拥有唯一 `OrganizationPointerSession`；
- 执行 4 px 激活、`elementFromPoint` 命中、有效性计算、Escape/blur 清理和 PointerUp 提交；
- 只向 `FolderTree` 暴露 `{ entityId, mode, valid } | null`，只向视觉层暴露安全的项目数与模式；
- 只在有效释放时调用一次既有 `dropFiles` 回调。

将该协调器从 `App.tsx` 提取，避免继续扩大组合根并便于纯指针测试。

### 5.3 `FolderTree`

- 删除内部整理所需的 HTML `DragEvent`、Viewer MIME 与 `DataTransfer.types` 判断；
- 每行增加 `data-organization-folder-id={folder.entityId}`；
- 接收受控 drop target 状态并继续使用现有 `data-drop-mode`/`data-drop-invalid` 视觉选择器；
- 文件夹点击与展开行为不变。

文件夹虚拟列表的滚动容器增加稳定的 `data-organization-drop-surface` 标记。协调器只用该标记定位滚动表面，不依赖 CSS 类名或路径。

## 6. 浮层、目标反馈与自动滚动

进入 `dragging` 后，顶层工作区渲染 `pointer-events: none` 的浮层，内容仅为：

- `移动 1 项` / `移动 N 项`；或
- `复制 1 项` / `复制 N 项`。

浮层跟随指针并限制在窗口内，不显示绝对路径。有效文件夹沿用蓝色 move 或绿色 copy 高亮；无效文件夹显示红色边界；离开文件夹树即清除目标反馈。

当指针位于 `data-organization-drop-surface` 可视顶部/底部 32 CSS px 时，使用一个 `requestAnimationFrame` 循环垂直滚动，速度上限 18 CSS px/帧。每帧滚动后重新调用 `elementFromPoint`，因此虚拟列表新挂载的文件夹可以成为目标。指针离开边缘、会话取消/提交、滚动达到边界或工作区替换时必须取消动画帧。

## 7. 生命周期、错误和可访问性

- Escape 只在会话存在时消费，并立即恢复默认光标、捕获与目标状态；不影响普通快捷键。
- `releasePointerCapture` 使用保存的捕获节点，即使 React 已卸载 ref 也能释放；重复清理必须幂等。
- `window.blur`、`pointercancel`、项目关闭、文件集合替换和只读切换都无副作用取消。
- PointerUp 后的预检/执行错误继续使用现有安全消息与对话框；指针层不重复实现错误系统。
- 整理拖柄保留按钮角色、可访问名称和 tooltip。键盘用户继续通过“移动到…”与“复制到…”批量操作完成同一任务；自定义拖拽不引入键盘陷阱。

## 8. 安全与隐私

- 指针会话只传递当前 session/generation 下的不透明 entity IDs；DOM data attribute 只包含 folder entity ID。
- 不把路径放入 DOM、`DataTransfer`、前端日志或错误消息。
- 不新增 Tauri capability、文件系统权限、依赖或平台 API。
- `dragDropEnabled` 保持原生导入所需的默认启用状态；现有安全脚本继续拒绝显式 `false`。
- 文件移动/复制仍由 Rust 重新解析实体、验证根目录包含关系、文件身份、别名/符号链接、目标权限和冲突策略。

## 9. 测试与物理验收

实现遵循 RED→GREEN：

1. 证明打包配置保持原生 incoming drop，且内部整理源代码不再包含 Viewer MIME、`DataTransfer` 或 FolderTree HTML `dragover/drop`；
2. 证明导出表面与整理拖柄是互斥兄弟：主体只调用 Finder 导出，拖柄只启动 Pointer 会话；
3. 证明未选项先成为唯一选择，已选项冻结完整有序选择；
4. 证明普通模式和 PointerDown 时 Option-copy 冻结，释放 Option 后仍为 copy；
5. 证明 4 px 阈值、有效/无效/无目标命中及一次性提交；
6. 证明 Escape、cancel、blur、项目替换、只读切换和卸载不会调用 drop；
7. 证明文件夹树上下边缘自动滚动、虚拟目标重命中与 RAF/capture 清理；
8. 证明既有 Finder 主体导出、框选、点击/Command/Shift、文本行和 Finder 文件夹导入测试继续通过；
9. 完整 `pnpm gate:m3` 与 Apple Silicon 打包通过。

物理验收必须重新执行：

- `acceptance-photo.jpg` 从 `source-images` 的 `⋮⋮` 默认移动到 `organization-move`：原路径消失、目标路径出现、SHA-256 不变；
- PointerDown 时按住 Option，从 `source-images` 的 PNG 启动后松开 Option，再释放到 `organization-copy`：源与目标同时存在且 SHA-256 一致；
- 关闭项目后，从 Finder 将 fixture `project` 文件夹拖入空 Viewer，证明原生 incoming handler 未被破坏；
- 退出并重启同一打包应用后，再完成一次卡片主体 Finder 导出。

## 10. 非目标

- 不统一 Pointer 框选与整理会话为一个大型手势框架；二者共享数值常量时仍保持独立状态机；
- 不改变 Finder 导出 AppKit 适配器；
- 不关闭 Tauri 原生文件夹导入；
- 不增加跨窗口、跨 Viewer 实例或 Windows 内部拖拽实现；
- 不替换现有移动/复制对话框与键盘可访问路径。
