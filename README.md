# Viewer

> 面向大量本地图像与文本素材的 macOS 浏览、审阅与轻量整理工具。

Viewer 在不改变现有目录结构的前提下，帮助用户快速浏览、搜索、比较、标记和整理
项目内容。项目文件始终保留在本机，原始目录与文件系统是唯一的数据真实来源。

## 当前状态

- 当前开发版本：`0.1.7`。
- 项目仍在持续开发，`0.1.7` 不代表功能已经完整，也不是最终交付目标。
- 当前开发与验证基线为 Apple Silicon、macOS 13 或更高版本。
- 目前以源码构建和内部开发使用为主，暂不提供公开下载。
- 当前不进入发布工程阶段；代码签名、Apple 公证、正式安装包、公开发布和发售均不在
  每轮开发的默认范围内，只有项目负责人明确切换阶段或单独授权时才处理。

## 项目简介

Viewer 主要面向需要审阅大量 AI 批量生成图片、设计方案和配套文本资料的设计师。
它会递归索引用户选择的本地项目文件夹，并将多层目录中的内容集中呈现在一个工作区
内，减少在 Finder 中反复进入目录、查找素材和比较结果的操作。

Viewer 是本地项目查看器、审阅工具和轻量文件整理工具，不是图片编辑器、AI 图片
生成工具或云端数字资产管理系统。应用一次只打开一个项目，不要求用户迁移素材或
改变已有的文件组织方式。

## 核心功能

- **本地项目扫描**：递归识别图片、Markdown/TXT、本地视频候选和其他普通文件，
  并渐进展示扫描与派生结果。
- **目录化浏览**：保留原始目录层级，提供目录树、分类目录概览和内容文件夹概览。
- **自适应缩略图**：按图片原始宽高比展示缩略图，支持 1–5 五档全局缩略图大小设置。
- **搜索与整理**：提供项目内搜索、筛选、排序和渐进索引反馈。
- **预览与比较**：支持图片预览、Markdown/TXT 双栏只读预览、2～8 张图片并排
  对比，以及本地视频封面、时间轴和原生画面播放。
- **高效审阅**：支持审阅状态、收藏、多选、框选和常用键盘操作。
- **安全文件操作**：支持项目内重命名、复制、移动、移入系统废纸篓、会话撤销，
  以及拖至 Finder 复制导出。
- **外部变化与异常处理**：自动协调项目打开期间的文件系统变化，支持只读模式，
  并隔离单个损坏或无法解码的文件。
- **本地优先**：不包含账户、云同步、遥测、自动上传、自动更新、Shell 访问或应用
  主动网络访问。

## 快速开始

### 环境要求

- Apple Silicon Mac
- macOS 13 或更高版本
- Xcode Command Line Tools
- Node.js 24.18.0
- pnpm 10.0.0
- Rust 1.97.0，包含 `rustfmt`、`clippy` 和 `aarch64-apple-darwin` 目标

### 安装依赖

```bash
corepack enable
corepack prepare pnpm@10.0.0 --activate
pnpm install --frozen-lockfile
./scripts/check-dev-env.sh
```

### 启动开发版

使用项目启动器打开当前本地源码，包括尚未提交的修改：

```bash
pnpm start:viewer
```

启动器会关闭已有 Viewer 开发进程，清理废弃的运行时包装器，再从当前仓库
执行 Tauri 开发启动；成功后只保留一个当前仓库的 Viewer 开发实例。日志保存
在 `target/dev-launcher/tauri-dev.log`。它不会拉取远端代码、切换分支、安装或
启动已打包的 `Viewer.app`。

开发自动化在命令成功后不得再通过 bundle identifier 或临时 `Viewer.app`
二次打开 Viewer；验证应使用启动器输出的 Viewer PID 和进程表。

需要让原生验收自行管理生命周期时使用：

```bash
pnpm accept:native:managed -- --smoke --viewport 1024x720
```

该命令会启动当前工作树、把验收绑定到本次启动返回的 PID，并在结束后关闭同一会话；
不会按应用名称打开已打包的 `Viewer.app`；若已有打包版正在运行，命令会报告实例冲突而不关闭它。

## 技术栈

| 层级 | 主要技术 | 作用 |
| --- | --- | --- |
| 桌面外壳 | Tauri 2 | 窗口、类型化 IPC、权限边界与应用打包 |
| 核心逻辑 | Rust | 扫描、索引、文件操作、缓存、任务和系统集成 |
| 前端 | React、TypeScript、Vite | 工作区界面、状态呈现、快捷键与交互 |
| 数据 | SQLite、rusqlite、FTS5 | 会话索引、标记数据和全文搜索 |
| 图片 | Quick Look、Image I/O、Core Graphics | 缩略图、高清预览、方向与色彩处理 |
| 视频 | libmpv、FFmpeg、AppKit 原生表面 | 本地探测、封面/时间轴缩略图与播放控制 |

## 架构与仓库结构

Viewer 按职责分离领域规则、应用用例、基础设施、平台适配、桌面边界与用户界面：

| 路径 | 职责 |
| --- | --- |
| [`crates/viewer-domain`](crates/viewer-domain) | 领域对象、标识和值约束 |
| [`crates/viewer-application`](crates/viewer-application) | 应用用例、端口、会话与任务协调 |
| [`crates/viewer-infrastructure`](crates/viewer-infrastructure) | 索引、持久化、缓存与文件事务 |
| [`crates/viewer-platform-macos`](crates/viewer-platform-macos) | macOS 原生图片、文件监听和系统适配 |
| [`src-tauri`](src-tauri) | 桌面运行时、权限边界和类型化 IPC |
| [`ui`](ui) | React 用户界面 |
| [`tests`](tests) | 跨层集成、文件系统和安全边界测试 |
| [`docs`](docs) | 产品、架构、治理与历史证据 |

更深入的模块、数据流、安全边界和架构决策由项目文档单独维护，README 只作为仓库
入口。

## 开发与验证

请求代码评审前运行完整且不产生仓库漂移的门禁：

```bash
pnpm verify:clean
```

`pnpm architecture:boundaries` 是 `pnpm verify:clean` 中的强制确定性门禁，检查 Rust
生产依赖方向与环、生产 UI 的 Tauri 导入边界，以及既有 IPC、DTO、capability 和安全
契约。`pnpm architecture:trends` 只报告源码规模、函数长度、决策分数和测试比例趋势，
不会因趋势恶化阻断交付；基线中的每个异常键都必须在分类注册表中有一条经过评审的
责任、理由和复核触发条件。

`pnpm quality:report` 仅用于生成可选的持续趋势证据，不是发布或交付门禁。它会依次
运行 UI 覆盖率、Rust 覆盖率和非阻塞架构趋势报告；使用前先安装独立的 Rust 覆盖率
工具：

```bash
cargo install cargo-llvm-cov --version 0.8.7 --locked
pnpm architecture:boundaries
pnpm architecture:trends
pnpm quality:report
```

`cargo-llvm-cov` 是开发工具，不属于产品依赖；普通开发和 `pnpm verify:clean` 不要求
安装它。

构建 Apple Silicon macOS 应用：

```bash
pnpm build:macos
```

## 文档与参与

- [软件产品规格](docs/PRODUCT_SPEC.md)：0.1.7 的定位、边界、主流程与当前功能总览。
- [产品文档](docs/product/README.md)：用户指南、功能、格式、数据和故障排除入口。
- [用户指南](docs/product/USER_GUIDE.md)：从启动开发版到关闭项目的完整操作流程。
- [版本记录](CHANGELOG.md)：各开发基线已经发生的用户可见与工程变化。
- [文档索引](docs/README.md)：当前产品、架构、治理文档与历史实施证据。
- [贡献指南](CONTRIBUTING.md)：分支、提交、验证和依赖变更规范。
- [安全策略](SECURITY.md)：漏洞私下报告方式与项目数据保护要求。
- [第三方依赖声明](THIRD_PARTY_NOTICES.md)：进入构建产物的依赖与许可证信息。
- [致谢与参考](ACKNOWLEDGEMENTS.md)：架构和产品行为的参考来源。

## 许可证

Viewer 采用 [Apache-2.0](LICENSE) 许可证。
