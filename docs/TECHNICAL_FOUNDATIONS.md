# Viewer 技术框架与开源来源

> Status: Active

> 状态：G1～G4 技术证据已完成；当前开发治理见 ADR 0005
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
- 平台中立的视频会话、代次、控件、缓存和错误语义。

## 2. 正式基础技术框架

| 层级 | 技术 | Viewer 中的职责 | 来源关系 |
| --- | --- | --- | --- |
| 桌面外壳 | Tauri 2 | 窗口、Rust 核心、WKWebView、IPC、应用打包 | 正式依赖 |
| 核心语言 | Rust | 扫描、索引、文件操作、任务、缓存和系统集成 | 正式工具链 |
| 前端 | React + TypeScript + Vite | 两栏界面、状态呈现、快捷键和交互 | 正式依赖 |
| 数据库 | SQLite + rusqlite | 持久标记、会话索引和 FTS5 全文搜索 | 正式依赖 |
| 图片后端 | Quick Look Thumbnailing + Image I/O + Core Graphics + ColorSync | Quick Look 主缩略图、Image I/O 回退与高清预览、ICC 和 EXIF | 已通过 G1，见 ADR 0001 |
| 视频后端 | LGPL libmpv 0.41.0 + FFmpeg n8.0 + libplacebo 6.338.2 + VideoToolbox | 随包媒体探测、原生 GPU 表面、硬件解码、封面与时间轴帧 | macOS 开发实现；发布签名验收延期 |
| 文件监听 | notify + notify-debouncer-full + Unix device/inode identity | FSEvents、事件归并、重命名/移动关联 | 已通过 G3，见 ADR 0003；未直接引入 `file-id` |
| 网格虚拟化 | 自定义 `VirtualGrid` / `VirtualList` | 缩略图、目录卡片和搜索结果虚拟化 | 项目内实现；`ui/package.json` 未引入 TanStack Virtual |
| 系统废纸篓 | trash-rs | 将文件移入 macOS 废纸篓 | 已通过 G2，封装于平台 Adapter |
| 模糊匹配 | nucleo-matcher | Unicode/中文路径和文件名匹配 | 已通过 G3；发布时履行 MPL-2.0 notice/source 义务 |

### 2.1 G1 图片管线结论

[ADR 0001](adr/0001-macos-image-pipeline.md) 已将 macOS 0.1 的策略确定为“Quick Look 主缩略图，Image I/O 失败回退；Fit/100% 预览使用 Image I/O”。项目原图不复制到 Viewer 缓存；缓存只保存可重建的会话表示，通过会话绑定的随机 token 交给 WebView。

Apple M4 标准开发设备上的合成样本 gate 验证了 sRGB、Display P3、EXIF orientation 6、Alpha、损坏图片、100 次取消、4 路代理、受限协议和 700 MB 峰值预算。合成夹具数据只用于持续回归比较，不代表真实素材最终验收。真实素材体验检查可以按功能需要单独执行，但不阻塞阶段完成。

### 2.2 G2 文件事务结论

[ADR 0002](adr/0002-file-transaction-protocol.md) 已确定“逐项持久日志 + 确定性临时路径 + BLAKE3 证据 + no-replace 原子落位 + 证据驱动恢复”的文件事务协议。复制、同卷重命名/移动、循环与大小写重命名、Skip/KeepBoth/Replace、macOS Trash Adapter 和会话撤销边界均有真实文件系统测试。

故障矩阵覆盖复制、重命名、移动和替换各自 7 个持久状态，共 28 个强制终止点。恢复第二次运行无动作；未知目标、多个匹配候选、被篡改的临时路径或外部身份变化都会停止并进入人工审查，不覆盖或猜测删除用户文件。真实系统废纸篓测试默认跳过，只允许开发者显式设置环境变量后本地执行。

### 2.3 G3 扫描、搜索与代次安全结论

[ADR 0003](adr/0003-scan-search-and-generation.md) 已确定“文件夹优先的 128 节点渐进批次 + 可丢弃 WAL 会话索引 + FTS5 trigram/nucleo 分层搜索 + `(SessionId, Generation)` 双边界校验 + Watcher 只触发范围化重扫”的方案。

Apple M4 标准开发设备上的 20 轮新会话基准覆盖 1,000 个图片占位文件、100 个中文/英文 Markdown/TXT 和 210 个三级目录。首个文件夹事件 p95 为 20.08 ms，基础扫描 p95 为 39.17 ms，80 次索引查询 p95 为 0.86 ms，峰值 RSS 为 6.67 MB；旧代次发布为零，便携 `.viewer` 元数据哨兵在全部会话索引删除/重建后哈希不变。

### 2.4 G4 许可证与依赖冻结基线

Viewer 源码采用 **Apache-2.0**。Rust 和 npm 解析结果分别由 `Cargo.lock` 与 `pnpm-lock.yaml` 固定，`scripts/check-locked-dependencies.sh` 使用 locked/frozen 模式重新解析和安装，并在执行前后比较两个锁文件的 SHA-256。Rust 全图由 `cargo-deny` 执行漏洞、重复版本、许可证与来源策略；npm 全图由 `pnpm licenses list` 生成清单并拒绝未知/未审查许可证表达式，同时由 `pnpm audit` 检查已知高危漏洞。任何直接依赖变更都必须同步更新审计、本文和 `THIRD_PARTY_NOTICES.md`。

| 依赖组 | Viewer 0.1 锁定结果 |
| --- | --- |
| 桌面与前端运行时 | Tauri 2.11.5、`@tauri-apps/api` 2.11.1、React/React DOM 19.2.7 |
| 图片与 macOS 绑定 | `objc2` 0.6.4、`objc2-*` 0.3.2、`block2` 0.6.2、`dispatch2` 0.3.1；系统 Quick Look/Image I/O/Core Graphics |
| 文件、索引与搜索 | rusqlite 0.40.1、notify 8.2.0、notify-debouncer-full 0.7.0、nucleo-matcher 0.3.1、trash 5.2.6、walkdir 2.5.0 |
| 核心运行时 | tokio 1.52.3、tokio-util 0.7.18、serde 1.0.228、uuid 1.24.0、thiserror 2.0.18、blake3 1.8.5、sha2 0.10.9（bundle/runtime 与测试夹具完整性） |
| 前端构建 | `@tauri-apps/cli` 2.11.4、Vite 8.1.5、TypeScript 6.0.3、`@vitejs/plugin-react` 6.0.3 |
| 测试工具 | Vitest 4.1.10、jsdom 29.1.1、Testing Library React 16.3.2、tempfile 3.27.0 |

完整逐项版本、许可证、用途、上游与是否进入分发产物的人工复核记录见仓库根目录 `THIRD_PARTY_NOTICES.md`。`nucleo-matcher` 以未修改 MPL-2.0 依赖使用；架构灵感项目不进入构建，单独列于 `ACKNOWLEDGEMENTS.md`。

### 2.5 本地视频预览边界

React 只负责浏览、布局与控件，Tauri IPC 只传递类型化命令、代次状态和已注册缩略图标识；解码帧不进入 JavaScript。平台中立的 `VideoEngine`/服务层隔离 macOS AppKit 与 libmpv 类型，macOS adapter 管理 VideoToolbox、GPU 表面、音频、回调和确定性关闭。

Task 14 的旧 Apple M4 native artifacts 没有绑定 dirty tracked bytes、untracked Task 14 manifest、fixture、machine、app/runtime 与同一 run，不能事后补绑，因此 core native、H.264 1080p60 与 4K readiness/performance 都保持 `UNVERIFIED`，直到新 native run 产生完整 provenance。30-cycle 门只量化 clients/render contexts/surfaces；worker/artifact 关闭由独立 project/session close tests 覆盖，decoder/audio 没有在该门中计数，均不冒充 30-cycle 实测。

开发 smoke 在启动前严格验证 inventory/hash/architecture/loader containment。离线状态还必须读取 runner 在 exact audited app 经 canonical macOS `sandbox-exec` deny-network profile 启动、target process 与 stable window 绑定后原子写出的 launch artifact；env bit 或 emitter boolean 不能成为证据。该记录证明 sandbox launch boundary，不冒充独立 kernel packet capture。signed mode 额外 fail-closed 验证 Developer ID、timestamp、nested/app signature、stapler 与 `spctl`；当前用户 scope 明确延期发布签名验收。

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
