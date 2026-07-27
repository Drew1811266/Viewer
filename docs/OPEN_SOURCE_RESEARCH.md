# Viewer 开源技术框架采纳与致谢策略

> Status: Active

> 调研日期：2026-07-15  
> 状态：Viewer 0.1 许可证与直接依赖已在 G4 冻结；未来新增依赖仍须按锁定版本重新核验
> 说明：本文是工程许可证管理建议，不构成法律意见

## 1. 结论

Viewer 的目标不是复制或改写其他完整应用，而是采用成熟开源框架、学习经过验证的架构方法，并围绕 Viewer 的产品文档独立实现代码。需要区分三种关系：

1. **正式技术依赖**：通过 Cargo、npm 或系统框架采用，并保留依赖许可证、版权和版本信息。
2. **架构方法参考**：学习项目公开描述的模块拆分、任务模型和工程方法，由 Viewer 团队独立设计接口和实现代码，并在致谢中说明灵感来源。
3. **产品行为参考**：观察公开软件如何解决浏览、预览、文件操作和异常反馈问题，形成 Viewer 自己的需求与测试，不复刻其界面或源码。

Viewer 当前不计划从其他完整应用复制源码。只有正式引入的通用库或框架代码会作为依赖进入构建。

Viewer 已明确采用 **Apache-2.0**。依赖治理策略是：

- 宽松许可证框架可以进入正式依赖候选，但接入前仍需依赖审计。
- MPL、GPL、AGPL、FSL 和自定义许可证的完整应用默认只作为架构或产品行为参考。
- 没有明确许可证的仓库视为保留全部权利，不使用其源码或资产。

## 2. 优先借鉴的完整项目

| 项目 | 当前许可证 | 与 Viewer 的相关性 | 采用方式 |
| --- | --- | --- | --- |
| [Tiefsee4](https://github.com/hbl917070/Tiefsee4) | MIT | 图片浏览、目录面板、文件面板、空格预览、拖出文件、批量查看 | 产品行为与 WebView 交互参考；Viewer 独立实现界面与状态模型 |
| [Oculante](https://github.com/woelper/oculante) | MIT | Rust 图片查看器、预览状态、图片解码与平台边界 | 图片任务、错误隔离和测试方法参考；不引入其 egui 应用代码 |
| [Yazi](https://github.com/sxyazi/yazi) | MIT | 高性能异步文件管理、任务优先级、取消、预加载、批量重命名和废纸篓 | 任务调度框架的重要参考；Viewer 独立实现面向 GUI 的 Rust 调度器 |
| [Spacedrive](https://github.com/spacedriveapp/spacedrive) | FSL-1.1-ALv2，特定版本未来转 Apache-2.0 | Rust + Tauri、内容指纹、事务式文件操作、索引与任务架构 | **只研究、不复制当前源码**；当前许可证限制竞争性产品或服务，Viewer 与其文件管理能力存在重叠 |
| [qView](https://github.com/jurplel/qView) | GPL-3.0 | 极简图片查看、缩放、导航、删除与跨平台错误案例 | 研究交互和测试场景；除非 Viewer 选择兼容 GPL-3.0 的许可证，否则不复制源码 |
| [nomacs](https://github.com/nomacs/nomacs) | GPL-3.0 | macOS/Windows 图片预览、色彩、方向、格式错误与批处理 | 研究图片查看行为和边界案例；不直接复制源码 |
| [digiKam](https://github.com/KDE/digikam) | GPL-2.0 系列 | 图片数据库、缩略图数据库、标签、搜索和完整性维护 | 研究数据分层、缩略图失效和数据库修复策略；不直接复制源码或数据库实现 |
| [TagStudio](https://github.com/TagStudioDev/TagStudio) | GPL-3.0 | 单项目元数据文件、标签、未标记筛选和搜索表达 | 研究项目级元数据与标记体验；不直接复制源码 |

## 3. 推荐直接采用或验证的依赖

### 3.1 桌面框架

- [Tauri 2](https://github.com/tauri-apps/tauri)：Apache-2.0/MIT 生态，作为应用外壳与 Rust/前端边界。
- React、TypeScript、Vite：使用各自官方发行版本，锁定精确版本并保留许可证清单。

### 3.2 数据与索引

- [rusqlite](https://github.com/rusqlite/rusqlite)：MIT；封装 SQLite，适用于 `.viewer/metadata.sqlite` 与会话索引。
- SQLite FTS5：用于 Markdown/TXT 全文索引；构建时确认 SQLite 功能和许可证声明。
- [nucleo-matcher](https://github.com/helix-editor/nucleo)：MPL-2.0；Unicode 感知的高性能模糊匹配候选，适合中文路径与文件名。因许可证为文件级弱 copyleft，接入前需确认分发义务并优先作为未修改依赖使用。

### 3.3 文件系统与任务

- [notify](https://github.com/notify-rs/notify)：CC0；macOS 使用 FSEvents 或 kqueue，可用于项目文件监听。
- `notify-debouncer-full` 与 `file-id`：MIT/Apache-2.0；用于事件合并和重命名/移动关联候选。
- [trash-rs](https://github.com/Byron/trash-rs)：MIT；支持 macOS 系统废纸篓。接入后仍需用真实目录测试权限、同名和批量失败。
- Yazi 的异步任务与优先级模型：优先研究设计，不建议直接引入整个项目或插件系统。

### 3.4 前端性能

- [TanStack Virtual](https://github.com/TanStack/virtual)：MIT；用于文件夹概览、缩略图网格和搜索结果的虚拟化。
- 引入前必须针对固定尺寸网格、调整缩略图尺寸、恢复滚动位置、多选和动态更新建立 Viewer 自己的回归测试，不能只依赖库的示例。

### 3.5 图片管线

- 0.1 的缩略图网格优先使用 macOS Quick Look Thumbnailing；Image I/O 负责失败回退，并与 Core Graphics、ColorSync 共同承担 JPG/PNG 探测和高清预览。
- Quick Look 与 Image I/O 必须通过 ICC、Display P3、EXIF 和透明 PNG 一致性原型；不一致时 JPG/PNG 缩略图统一改用 Image I/O。
- Oculante、nomacs、qView 主要用于补充错误样例、交互行为和回归用例，不替换系统图片后端。
- 不直接引入支持大量额外格式的解码器，避免突破 0.1 只支持 JPG/PNG 的产品边界。

## 4. 仅作为技术架构参考的项目

### 4.1 Spacedrive

Spacedrive 当前主分支为 FSL-1.1-ALv2，许可证明确限制与其相同或相似功能的竞争性用途，并约定特定版本在两年后转为 Apache-2.0。Viewer 属于文件查看与管理工具，存在功能重叠，因此：

- 可以阅读公开文档和 README 中描述的内容身份、操作预演与持久任务思想。
- 不复制当前主分支实现、模块、测试或界面代码。
- 如果未来希望采用某段代码，必须锁定文件、提交日期和未来许可证生效时间，重新核验后再决定。

### 4.2 GPL 项目

qView、nomacs、digiKam 和 TagStudio 使用 GPL 系列许可证。在 Viewer 已选择 Apache-2.0 的前提下：

- 可以观察公开产品行为并建立独立的需求与测试用例。
- 不复制函数、类、数据库代码、界面代码或资源。
- 不以“改写变量名”“翻译语言”方式规避衍生作品义务。
- 如果未来 Viewer 选择 GPL 兼容许可证，再单独进行源码级兼容性评估。

## 5. Viewer 仓库的技术来源与许可证治理

建议 GitHub 仓库从第一天包含：

- `LICENSE`：Viewer 自身许可证。
- `THIRD_PARTY_NOTICES.md`：所有正式分发依赖、版权人、许可证和来源。
- `ACKNOWLEDGEMENTS.md`：不是正式依赖、但对 Viewer 架构或产品行为有启发的开源项目及具体借鉴点。
- `CONTRIBUTING.md`：贡献流程和贡献者对代码授权的声明。
- `SECURITY.md`：安全问题的私下报告方式。
- `CODE_OF_CONDUCT.md`：社区行为准则。
- `docs/architecture/`：架构决策记录（ADR）。
- Rust 使用 `cargo-deny` 检查许可证、漏洞、重复依赖和禁用来源。
- 前端使用许可证扫描工具检查 npm 依赖，并把锁文件提交到仓库。
- GitHub Actions 在每次合并请求中执行许可证、单元测试、格式化和静态检查。

每个正式依赖都应记录：

- 来源仓库和永久链接。
- 锁定版本或精确提交哈希。
- 许可证和版权声明。
- 在 Viewer 中承担的职责。
- 是否修改或派生了依赖代码。

每个架构参考项目在 `ACKNOWLEDGEMENTS.md` 中记录：

- 项目名称和官方仓库。
- 启发 Viewer 的具体架构或产品问题。
- Viewer 的实现是独立完成，而非复制来源代码。

图片、图标、字体、示例素材和截图必须单独检查资产许可证，不能因为代码仓库使用 MIT 就自动假设所有资产均为 MIT。

## 6. Viewer 自身许可证建议

在没有特殊商业限制诉求时，优先考虑：

- **Apache-2.0**：允许广泛使用和再分发，同时提供明确的专利授权与 NOTICE 机制；适合希望建立长期公共项目的 Viewer。
- **MIT**：最简洁、采用门槛最低，但专利条款和 NOTICE 治理弱于 Apache-2.0。
- **GPL-3.0**：要求衍生软件继续开源，适合希望强制共享改进的项目，但会限制部分商业或闭源集成。

Viewer 已确认使用 **Apache-2.0**，完整许可证见仓库根目录 `LICENSE`。这一选择不改变第三方依赖各自的许可证义务，也不授权复制不兼容或用途受限项目的源码。

## 7. 下一步审计清单

1. 每个新增候选依赖先建立版本、许可证、维护状态和替代方案记录。
2. 网格虚拟化等尚未冻结的候选技术必须先制作原型并通过性能与异常测试。
3. 只在原型达标和许可证复核通过后将新框架或依赖纳入正式技术栈。
4. 每次发布前从锁文件复核 `THIRD_PARTY_NOTICES.md`，并保留自动审计和人工审查证据。
