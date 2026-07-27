# Viewer README Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the repository root README with a concise Chinese introduction for Viewer 0.1.1 that serves both prospective users and development contributors.

**Architecture:** Keep `README.md` as the product-first repository entry point and link deeper engineering detail to the existing documentation set. Use only repository-relative links and commands already defined by the project; do not duplicate detailed product specifications or architecture evidence.

**Tech Stack:** GitHub Flavored Markdown, repository-relative links, pnpm project scripts, Cargo workspace metadata

## Global Constraints

- Write the README entirely in Simplified Chinese.
- Identify `0.1.1` as the current development version, not a complete product or final delivery target.
- Do not add an M4 acceptance stage or restore Viewer 0.1 as a delivery goal.
- Do not add screenshots, GIFs, badges, external status images, or a hand-maintained table of contents.
- State only capabilities implemented by the current source and covered by active specifications or tests.
- Do not promise a Windows delivery date, public release date, commercial support, or a downloadable binary.
- Preserve the local-first boundary: no accounts, cloud sync, telemetry, automatic upload, automatic updater, shell access, or application-originated network access.
- Keep the verified environment floor exact: Apple Silicon, macOS 13+, Node.js 24.18.0, pnpm 10.0.0, and Rust 1.97.0.
- Preserve the user-owned untracked `tests/fixtures/images/.viewer/` directory without staging or modifying it.

---

### Task 1: Rewrite and verify the repository README

**Files:**
- Modify: `README.md`
- Reference: `docs/superpowers/specs/2026-07-27-viewer-readme-design.md`
- Reference: `docs/PRODUCT_SPEC.md`
- Reference: `docs/TECHNICAL_FOUNDATIONS.md`
- Reference: `package.json`
- Reference: `scripts/check-dev-env.sh`

**Interfaces:**
- Consumes: Viewer product version `0.1.1`, existing pnpm scripts, the current architecture boundaries, and repository-relative documentation paths.
- Produces: A root `README.md` that GitHub renders as the canonical project introduction.

- [ ] **Step 1: Capture the repository baseline**

Run:

```bash
git status --short
pnpm test:policy
```

Expected:

- policy tests pass;
- the only unrelated entry remains `?? tests/fixtures/images/.viewer/`;
- no README implementation change exists yet.

- [ ] **Step 2: Replace `README.md` with the approved product-first content**

Use these exact headings in this order:

```markdown
# Viewer

## 当前状态

## 项目简介

## 核心功能

## 快速开始

### 环境要求

### 安装依赖

### 启动开发版

## 技术栈

## 架构与仓库结构

## 开发与验证

## 文档与参与

## 许可证
```

Populate the sections with the following exact content requirements:

- Opening: describe Viewer as a local-first macOS desktop application for reviewing large local image and text collections without changing the existing directory structure.
- Current status: state `0.1.1`, active development, incomplete feature set, Apple Silicon/macOS 13+ baseline, and source-build/internal-development availability.
- Project introduction: identify designers working with AI-generated image batches as a core audience; clarify that Viewer is a viewer, review tool, and lightweight file organizer rather than an image editor, AI generator, or cloud DAM.
- Core features:
  - recursive local scanning for JPG, JPEG, PNG, Markdown, and TXT;
  - directory tree, category overview, and content-folder overview;
  - aspect-aware thumbnails with compact, standard, and large global density settings;
  - search, filters, sorting, progressive indexing feedback;
  - image preview, text preview, and 2–4 image comparison;
  - review states, favorites, multiselect, marquee selection, and keyboard operations;
  - in-project rename/copy/move, Trash deletion, session undo, and Finder export;
  - external filesystem reconciliation, read-only mode, and isolated file-error handling;
  - local-only operation with no accounts, cloud sync, telemetry, automatic upload, updater, shell access, or application-originated network access.
- Environment requirements: Apple Silicon Mac, macOS 13+, Xcode Command Line Tools, Node.js 24.18.0, pnpm 10.0.0, and Rust 1.97.0 with `rustfmt`, `clippy`, and `aarch64-apple-darwin`.
- Installation:

```bash
corepack enable
corepack prepare pnpm@10.0.0 --activate
pnpm install --frozen-lockfile
./scripts/check-dev-env.sh
```

- Development launch:

```bash
pnpm tauri dev
```

- Technology table: Tauri 2, Rust, React/TypeScript/Vite, SQLite/rusqlite/FTS5, and Quick Look/Image I/O/Core Graphics.
- Repository table:
  - `crates/viewer-domain`;
  - `crates/viewer-application`;
  - `crates/viewer-infrastructure`;
  - `crates/viewer-platform-macos`;
  - `src-tauri`;
  - `ui`;
  - `tests`;
  - `docs`.
- Development commands:

```bash
pnpm verify:clean
pnpm quality:report
pnpm build:macos
```

- Explain that `verify:clean` is the review gate; `quality:report` is optional trend evidence and requires `cargo-llvm-cov 0.8.7`; `build:macos` builds the Apple Silicon application.
- Documentation links:
  - `[文档索引](docs/README.md)`;
  - `[贡献指南](CONTRIBUTING.md)`;
  - `[安全策略](SECURITY.md)`;
  - `[第三方依赖声明](THIRD_PARTY_NOTICES.md)`;
  - `[致谢与参考](ACKNOWLEDGEMENTS.md)`;
  - `[Apache-2.0](LICENSE)`.

Do not insert a screenshot placeholder, badge placeholder, roadmap checklist, release download link, or manual table of contents.

- [ ] **Step 3: Validate Markdown structure and repository-relative links**

Run:

```bash
node -e 'const fs=require("node:fs");const text=fs.readFileSync("README.md","utf8");const links=[...text.matchAll(/\[[^\]]+\]\((?!https?:|#)([^)]+)\)/g)].map(m=>m[1]);for(const link of links){if(!fs.existsSync(link))throw new Error(`missing README link: ${link}`)}console.log(`README relative links verified: ${links.length}`)'
```

Expected: `README relative links verified: 14`, with no missing-link error.

Run:

```bash
git diff --check
```

Expected: exit code 0 with no whitespace errors.

- [ ] **Step 4: Run the repository documentation and quality gates**

Run:

```bash
pnpm test:policy
pnpm verify:clean
```

Expected:

- repository policy and scope coverage tests pass;
- UI tests report 357 passing tests;
- Rust workspace, security, license, build, lint, and formatting checks pass;
- verification does not create or remove tracked or untracked entries.

- [ ] **Step 5: Review the final diff and commit only the README**

Run:

```bash
git diff -- README.md
git status --short
```

Verify:

- every capability claim matches the approved design;
- `tests/fixtures/images/.viewer/` remains untracked and unchanged;
- no file other than `README.md` belongs to this implementation commit.

Commit:

```bash
git add README.md
git commit -m "docs: introduce Viewer 0.1.1"
```
