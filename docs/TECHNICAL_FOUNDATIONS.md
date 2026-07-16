# Viewer 技术框架与开源来源

> 状态：全局架构已确认，正式依赖版本待技术原型验证  
> 原则：采用通用框架，独立实现 Viewer，不复制其他完整应用

完整模块、数据流、安全和测试设计见 `docs/superpowers/specs/2026-07-16-viewer-system-architecture-design.md`。

## 1. Viewer 自有实现范围

以下部分由 Viewer 根据产品文档独立设计和实现：

- 项目、分类目录、内容文件夹、文件项和标记领域模型。
- 两栏主界面、目录概览、缩略图网格、文本区域、单图预览和对比工作区。
- 搜索、筛选、排序和审阅交互规则。
- `.viewer` 可迁移元数据格式与版本迁移规则。
- 文件操作事务、撤销语义、冲突处理和崩溃恢复协议。
- 任务优先级在 Viewer 场景下的具体队列、状态机和 IPC 接口。
- 性能预算、缓存生命周期和异常降级规则。

## 2. 正式基础技术框架

| 层级 | 技术 | Viewer 中的职责 | 来源关系 |
| --- | --- | --- | --- |
| 桌面外壳 | Tauri 2 | 窗口、Rust 核心、WKWebView、IPC、应用打包 | 正式依赖 |
| 核心语言 | Rust | 扫描、索引、文件操作、任务、缓存和系统集成 | 正式工具链 |
| 前端 | React + TypeScript + Vite | 两栏界面、状态呈现、快捷键和交互 | 正式依赖 |
| 数据库 | SQLite + rusqlite | 持久标记、会话索引和 FTS5 全文搜索 | 正式依赖 |
| 图片后端 | Quick Look Thumbnailing + Image I/O + Core Graphics + ColorSync | 系统缩略图、JPG/PNG 解析、高清预览、ICC 和 EXIF | macOS 系统框架 |
| 文件监听 | notify + notify-debouncer-full + file-id | FSEvents、事件归并、重命名/移动关联 | 正式依赖候选 |
| 网格虚拟化 | TanStack Virtual | 缩略图、目录卡片和搜索结果虚拟化 | 正式依赖候选 |
| 系统废纸篓 | trash-rs | 将文件移入 macOS 废纸篓 | 正式依赖候选 |
| 模糊匹配 | nucleo-matcher | Unicode/中文路径和文件名匹配 | 正式依赖候选，需确认 MPL-2.0 义务 |

## 3. 架构方法来源

### Yazi

来源：[sxyazi/yazi](https://github.com/sxyazi/yazi)

借鉴点：

- 非阻塞 I/O 与 CPU 任务分离。
- 用户任务、预览任务和后台预加载的优先级。
- 任务取消、进度和批量文件操作反馈。
- 当前预览优先于后台工作的资源调度原则。

Viewer 独立实现适合 Tauri GUI、单项目会话和图片浏览的任务调度器，不引入 Yazi 的终端 UI、插件或完整任务实现。

### Spacedrive

来源：[spacedriveapp/spacedrive](https://github.com/spacedriveapp/spacedrive)

借鉴点：

- 将文件身份与绝对路径分离。
- 内容指纹辅助恢复移动或重命名后的关联。
- 文件操作执行前预演冲突与结果。
- 文件操作作为可追踪任务，而不是瞬时 UI 动作。

Viewer 只参考公开架构思想，独立设计轻量的单项目实现，不复制其 FSL 主分支代码，也不引入分布式文件系统、同步或守护进程。

### digiKam

来源：[KDE/digikam](https://github.com/KDE/digikam)

借鉴点：

- 持久业务元数据与可重建缩略图数据分离。
- 标签、搜索索引与缩略图采用不同生命周期。
- 数据库完整性检查和可重建缓存策略。

Viewer 独立设计 `.viewer/metadata.sqlite` 与会话索引，不复制 digiKam 数据库结构或 GPL 实现。

### Tiefsee4、Oculante、qView、nomacs

来源：

- [hbl917070/Tiefsee4](https://github.com/hbl917070/Tiefsee4)
- [woelper/oculante](https://github.com/woelper/oculante)
- [jurplel/qView](https://github.com/jurplel/qView)
- [nomacs/nomacs](https://github.com/nomacs/nomacs)

借鉴点：

- 空格快速预览、方向键导航、适应窗口与 100% 查看。
- 图片面板、目录面板和大图之间的上下文保持。
- 损坏图片、删除失败、格式差异和色彩方向等边界测试。
- 极简图片查看器应避免的常驻面板和视觉干扰。

Viewer 的界面、组件、状态模型和视觉系统均独立实现。

## 4. 仓库中的透明披露

- `README.md` 的“技术栈”列出正式框架和库。
- `THIRD_PARTY_NOTICES.md` 列出实际进入构建产物的第三方依赖及许可证。
- `ACKNOWLEDGEMENTS.md` 列出架构和产品行为参考项目及具体启发。
- 架构决策记录说明为何选用某项框架、评估过哪些替代方案以及如何验证。
- 不把“Inspired by”项目描述为 Viewer 的代码依赖，也不暗示原项目维护者认可 Viewer。
