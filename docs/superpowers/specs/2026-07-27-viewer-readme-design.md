# Viewer README 设计说明

> 状态：已确认，待实施
>
> 日期：2026-07-27
>
> 目标版本：Viewer 0.1.1

## 1. 目标

重写仓库根目录 `README.md`，使第一次访问仓库的普通使用者和开发贡献者
能够快速理解：

- Viewer 是什么、服务于什么场景；
- 当前版本已经具备哪些主要能力；
- Viewer 的本地优先边界和开发阶段状态；
- 如何从源码启动、验证和构建项目；
- 在哪里查阅架构、贡献、安全和许可证信息。

README 是项目入口，不替代产品规格、架构设计或历史评审文档。详细信息继续由
`docs/README.md` 指向对应的活动文档。

## 2. 受众与语言

- 全文使用简体中文。
- 同时服务普通使用者和开发贡献者。
- 采用清晰、克制、可验证的技术写作风格。
- 不使用“完整”“成熟”“生产就绪”等未经证实的营销表述。
- 不承诺 Windows 支持时间、公开发布计划或商业支持。

## 3. 结构方案

采用产品优先型信息漏斗：

1. 项目标题与一句话定位；
2. 当前开发状态；
3. 项目简介；
4. 核心功能；
5. 快速开始；
6. 技术栈与架构；
7. 开发与验证；
8. 文档、贡献和安全；
9. 许可证。

README 不设置手工目录。GitHub 会根据标题生成文档大纲，标准篇幅下无需额外维护
一份可能失效的目录。

README 暂不加入截图、GIF、徽章或外部状态图片。仓库目前没有稳定的公开发布地址
和远程 CI 徽章来源，不应展示不可验证或容易失效的状态。

## 4. 项目定位

开篇使用以下语义，不要求逐字复制：

> Viewer 是一款面向大量本地图像与文本素材审阅场景的 macOS 桌面应用，
> 帮助用户在不改变原有目录结构的前提下完成浏览、搜索、比较、标记和整理。

定位说明需要包含：

- 核心用户包括使用 AI 工具批量生成图片、管理多层素材目录的设计师；
- 项目目录与源文件仍是数据真实来源；
- Viewer 是本地查看、审阅与轻量文件管理工具；
- Viewer 不是图片编辑器、AI 图片生成工具或云端数字资产管理系统。

## 5. 当前状态与产品边界

README 必须明确：

- 当前开发版本为 `0.1.1`；
- `0.1.1` 是持续开发版本，不代表软件功能已经完整；
- 当前验证基线为 Apple Silicon、macOS 13 或更高版本；
- 当前以源码构建和内部开发使用为主，不提供公开下载链接；
- 应用一次打开一个本地项目；
- 应用不包含账户、云同步、遥测、自动上传、自动更新、Shell 权限或应用主动
  网络访问。

不得恢复已移除的 M4 验收环节，不得将 Viewer 0.1 或 0.1.1 描述为最终交付目标。

## 6. 核心功能

核心功能使用简短列表，只陈述当前代码已实现且现有测试覆盖的主要能力：

- 递归扫描本地项目目录，支持 JPG、JPEG、PNG、Markdown 和 TXT；
- 保留原始目录结构，提供目录树、分类目录和内容文件夹概览；
- 图片按原始宽高比展示，文件夹胶片条保持每行等高，内容视图按可用宽度换行；
- 提供紧凑、标准和大图三档全局缩略图密度；
- 提供项目内搜索、筛选、排序和渐进索引反馈；
- 提供图片预览、文本预览和 2～4 张图片并排对比；
- 提供审阅状态、收藏、多选、框选和常用键盘操作；
- 提供项目内重命名、复制、移动、废纸篓删除、会话撤销及拖至 Finder 导出；
- 监听外部文件变化，并在只读项目或单文件失败时安全降级。

README 不展开文件事务状态机、恢复矩阵、搜索算法或图片管线预算。这些内容通过
活动架构文档链接承载。

## 7. 快速开始

### 7.1 环境要求

- Apple Silicon Mac；
- macOS 13 或更高版本；
- Xcode Command Line Tools；
- Node.js 24.18.0；
- pnpm 10.0.0；
- Rust 1.97.0，包含 `rustfmt`、`clippy` 和 `aarch64-apple-darwin` 目标。

版本必须与 `scripts/check-dev-env.sh`、`rust-toolchain.toml` 和
`package.json` 保持一致。

### 7.2 安装与启动

README 提供以下最短路径：

```bash
corepack enable
corepack prepare pnpm@10.0.0 --activate
pnpm install --frozen-lockfile
./scripts/check-dev-env.sh
pnpm tauri dev
```

不提供未经项目验证的 Homebrew、npm 全局安装或预编译包下载命令。

## 8. 技术栈与架构

技术栈使用紧凑表格说明：

| 层级 | 主要技术 | 作用 |
| --- | --- | --- |
| 桌面外壳 | Tauri 2 | 窗口、IPC、权限与打包 |
| 核心逻辑 | Rust | 扫描、索引、文件操作、缓存与系统集成 |
| 前端 | React、TypeScript、Vite | 工作区界面、状态呈现与交互 |
| 数据 | SQLite、rusqlite、FTS5 | 会话索引、标记与全文搜索 |
| 图片 | Quick Look、Image I/O、Core Graphics | 缩略图、预览、方向与色彩处理 |

仓库结构使用第二个紧凑表格或列表说明：

- `crates/viewer-domain`：领域对象与不变量；
- `crates/viewer-application`：用例、端口与会话协调；
- `crates/viewer-infrastructure`：索引、持久化、缓存与文件事务；
- `crates/viewer-platform-macos`：macOS 原生适配；
- `src-tauri`：桌面运行时与类型化 IPC；
- `ui`：React 用户界面；
- `tests`：跨层集成与安全边界测试；
- `docs`：产品、架构、治理与历史证据。

## 9. 开发、验证与构建

README 只突出三个常用入口：

```bash
pnpm verify:clean
pnpm quality:report
pnpm build:macos
```

说明：

- `pnpm verify:clean` 是请求评审前的完整门禁；
- `pnpm quality:report` 生成覆盖率与架构健康趋势，需要单独安装
  `cargo-llvm-cov 0.8.7`；
- `pnpm build:macos` 生成 Apple Silicon macOS 构建；
- `cargo-llvm-cov` 不属于产品依赖，普通开发与 `pnpm verify:clean` 不要求安装。

## 10. 文档与项目协作

README 结尾提供相对链接：

- `docs/README.md`：活动文档、历史计划与评审证据索引；
- `CONTRIBUTING.md`：分支、提交、验证和依赖变更规范；
- `SECURITY.md`：私下报告漏洞与保护项目数据的方法；
- `THIRD_PARTY_NOTICES.md`：实际进入构建的第三方依赖；
- `ACKNOWLEDGEMENTS.md`：架构与产品参考来源；
- `LICENSE`：Apache-2.0 许可证。

所有仓库内链接使用相对路径，确保 GitHub 在不同分支中正确解析。

## 11. 验证策略

README 实施后执行：

1. 检查 Markdown 标题层级、代码块和相对链接；
2. 运行仓库策略测试，确认 README 仍满足活动文档与工程命令约束；
3. 运行 `pnpm verify:clean`，确认文档修改没有引入生成物或仓库漂移；
4. 检查 `git diff --check`；
5. 确认未修改用户已有的 `tests/fixtures/images/.viewer/`。

## 12. 参考结构

设计参考以下公开资料的结构原则，不复制其具体文案：

- GitHub 官方 README 指南：
  <https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-readmes>
- Tauri README：
  <https://github.com/tauri-apps/tauri>
- ripgrep README：
  <https://github.com/BurntSushi/ripgrep>
- Awesome README：
  <https://github.com/matiassingers/awesome-readme>

采用的共同原则是：先解释项目价值，再给出可信的功能概览和最短入门路径，
将深入细节链接到专门文档。
