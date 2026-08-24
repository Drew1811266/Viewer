# Viewer 0.1.6 Product Documentation Reconstruction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Viewer’s mixed planning/current documentation with a detailed, evidence-backed product documentation suite describing the actual `v0.1.6` development build.

**Architecture:** Preserve the former requirement specification as a historical milestone artifact, then make `docs/PRODUCT_SPEC.md` the concise current-product authority. Put task-oriented and reference material in focused files under `docs/product/`, keep engineering and historical evidence separately indexed, and enforce version/link/status consistency through the existing repository policy tests.

**Tech Stack:** Markdown, Node.js 24 built-in test runner, existing repository policy scripts, Git.

**Spec:** `docs/superpowers/specs/2026-08-23-viewer-product-documentation-reconstruction-design.md`

## Global Constraints

- The documented product baseline is Git tag `v0.1.6`, commit `4f1e839763ba8cef143701bd393f023e445baad6`.
- Runtime code and tests outrank existing prose when sources disagree.
- Product documentation describes implemented behavior, partial behavior, explicit absence, and development limits; it does not promote future plans to current capability.
- The product remains a continuously iterated development build; signing, notarization, App Store delivery, public sales, and formal-release operations are out of scope.
- Existing historical specs, plans, reviews, ADRs, and progress records remain in the repository.
- Product-facing prose is Simplified Chinese and uses the actual interface labels present in `ui/src`.
- No product behavior changes are included.
- Each detailed fact has one authoritative location; other documents link to it instead of duplicating it.

---

## File Structure

### Current product authorities

- `docs/PRODUCT_SPEC.md` — concise 0.1.6 product baseline and current behavior summary.
- `docs/product/README.md` — product documentation portal and reading routes.
- `docs/product/USER_GUIDE.md` — complete task-oriented operating guide.
- `docs/product/FEATURE_REFERENCE.md` — detailed behavior and evidence map by feature domain.
- `docs/product/SHORTCUTS.md` — keyboard, mouse, and trackpad reference.
- `docs/product/SUPPORTED_FORMATS.md` — scan candidates, preview capabilities, and format limits.
- `docs/product/DATA_PRIVACY.md` — local data, metadata, cache, network, and privacy boundaries.
- `docs/product/TROUBLESHOOTING.md` — symptom-to-recovery guidance.
- `docs/product/DOCUMENTATION_MAINTENANCE.md` — future version documentation checklist.
- `CHANGELOG.md` — development-version history beginning with 0.1.6.

### Preserved historical authority

- `docs/milestones/viewer-0.1-target-requirements.md` — former requirement-oriented `docs/PRODUCT_SPEC.md`, explicitly historical.
- `docs/milestones/viewer-0.1-scope-matrix.md` — historical requirement-to-milestone mapping, retargeted to the preserved requirement file.

### Repository navigation and enforcement

- `README.md` — repository entry, current version, concise capability list, product-document links.
- `docs/README.md` — current product, current engineering, effective ADR, and historical evidence index.
- `scripts/check-scope-coverage.mjs` — reads the preserved historical requirements rather than the new current product spec.
- `scripts/repository-policy.test.mjs` — verifies product-doc inventory, current version agreement, local Markdown links, placeholder absence, and documentation status rules.

---

### Task 1: Preserve the former requirement baseline and retarget governance

**Files:**

- Move: `docs/PRODUCT_SPEC.md` → `docs/milestones/viewer-0.1-target-requirements.md`
- Modify: `docs/milestones/viewer-0.1-target-requirements.md`
- Modify: `docs/milestones/viewer-0.1-scope-matrix.md`
- Modify: `scripts/check-scope-coverage.mjs`
- Modify: `scripts/repository-policy.test.mjs`
- Test: `scripts/repository-policy.test.mjs`
- Test: `scripts/scope-coverage.test.mjs`

**Interfaces:**

- Consumes: the existing `REQ-*` identifiers and scope-matrix rows.
- Produces: a stable historical requirement source for `validateScopeCoverage`, freeing `docs/PRODUCT_SPEC.md` to describe current behavior without requirement IDs.

- [x] **Step 1: Change the policy expectation to the historical source and verify failure**

Update the policy test named `active governance has no M4 owner or Viewer 0.1 delivery gate` so the governance set reads `docs/milestones/viewer-0.1-target-requirements.md` instead of `docs/PRODUCT_SPEC.md`, and add this test:

```js
test('historical scope coverage reads the preserved Viewer 0.1 target requirements', async () => {
  const checker = await read('scripts/check-scope-coverage.mjs')
  const matrix = await read('docs/milestones/viewer-0.1-scope-matrix.md')

  assert.match(checker, /docs\/milestones\/viewer-0\.1-target-requirements\.md/)
  assert.doesNotMatch(checker, /readFileSync\('docs\/PRODUCT_SPEC\.md'/)
  assert.match(matrix, /\[Historical target requirements\]\(viewer-0\.1-target-requirements\.md\)/)
})
```

Run:

```bash
node --test --test-name-pattern='historical scope coverage|active governance' scripts/repository-policy.test.mjs
```

Expected: FAIL because the historical file and retargeted links do not exist yet.

- [x] **Step 2: Move the old specification and label it historical**

Move the file with Git so its history is preserved:

```bash
git mv docs/PRODUCT_SPEC.md docs/milestones/viewer-0.1-target-requirements.md
```

Replace the old title and metadata block with:

```markdown
# Viewer 0.1 历史目标需求

> Status: Historical

> 原位置：`docs/PRODUCT_SPEC.md`
> 保存目的：为历史里程碑范围矩阵保留稳定的 `REQ-*` 定义。
> 当前产品事实：以 [`../PRODUCT_SPEC.md`](../PRODUCT_SPEC.md) 和 [`../product/`](../product/) 为准。

本文档记录 Viewer 0.1 开发过程中的目标需求，包含已经实现、部分实现、未来规划和当前不适用的条目。它不是 Viewer 0.1.6 的当前功能说明。
```

Keep every existing `REQ-*` heading and body after this block unchanged so the scope coverage count remains stable.

- [x] **Step 3: Retarget the scope matrix and checker**

Change the matrix metadata to:

```markdown
> Status: Historical

- Source of truth: [Historical target requirements](viewer-0.1-target-requirements.md)
- Current product behavior: [Viewer product specification](../PRODUCT_SPEC.md)
- Architecture baseline: [Viewer 0.1 API baseline](../architecture/viewer-0.1-api-baseline.md)
- Coverage check: `node scripts/check-scope-coverage.mjs`
```

Change `scripts/check-scope-coverage.mjs` to read:

```js
readFileSync('docs/milestones/viewer-0.1-target-requirements.md', 'utf8')
```

Keep the matrix input and success message unchanged.

- [x] **Step 4: Run focused governance verification**

Run:

```bash
node --test scripts/scope-coverage.test.mjs scripts/repository-policy.test.mjs
node scripts/check-scope-coverage.mjs
```

Expected: scope validation reports the same requirement and frozen M3 counts as before; repository policy may still fail only because the newly committed documentation-reconstruction design has not yet been added to the index.

- [x] **Step 5: Commit the historical split**

```bash
git add docs/milestones scripts/check-scope-coverage.mjs scripts/repository-policy.test.mjs
git commit -m "docs: preserve historical Viewer 0.1 requirements"
```

---

### Task 2: Create the current product authority and documentation portal

**Files:**

- Create: `docs/PRODUCT_SPEC.md`
- Create: `docs/product/README.md`

**Interfaces:**

- Consumes: version/config facts from `src-tauri/tauri.conf.json`, user-facing types from `ui/src/api/types.ts`, shell behavior from `ui/src/App.tsx`, and accepted scope in the design spec.
- Produces: the canonical current-product baseline linked by every other product document.

- [x] **Step 1: Write the concise current product specification**

Create `docs/PRODUCT_SPEC.md` with this metadata:

```markdown
# Viewer 软件产品规格

> Status: Current
> 适用版本：0.1.6
> 基线提交：`4f1e839763ba8cef143701bd393f023e445baad6`
> 事实日期：2026-08-24
> 产品阶段：持续开发版
```

Use these exact top-level sections, each with factual prose rather than requirement language:

1. `产品概述` — local macOS browser/reviewer/light organizer for image, text, and local-video projects.
2. `目标用户与核心任务` — large nested local asset projects; find, judge, compare, mark, and safely organize.
3. `产品边界` — not an editor, AI generator, cloud DAM, streaming player, or multi-project manager.
4. `运行与交付状态` — Apple Silicon, macOS 13+, source-built development use, no signed/public distribution promise.
5. `信息架构` — empty entry, two-column browser, dynamic workspace, previews, compare, task feedback, settings.
6. `端到端工作流` — open, progressive scan, browse/search, review, preview/compare, organize, close.
7. `当前功能总览` — link each domain to `product/FEATURE_REFERENCE.md` and the appropriate focused guide.
8. `数据、隐私与安全` — local filesystem truth, portable markers, rebuildable caches, no automatic egress.
9. `当前限制` — single project/window, current formats/platform, decode-dependent video, session-bounded undo, development status.
10. `文档导航` — link every file in `docs/product/`, `CHANGELOG.md`, technical foundations, and the documentation index.

Do not include `REQ-*` identifiers, future platform promises, unverified performance thresholds, or public-release acceptance language.

- [x] **Step 2: Create the product documentation portal**

Create `docs/product/README.md` with:

- an `0.1.6` applicability banner;
- the same source-priority rules as the design spec;
- a “按任务阅读” table mapping first use, daily workflow, shortcuts, format questions, privacy questions, and recovery to the focused files;
- a “文档状态” table declaring every file Current for 0.1.6;
- a statement that historical specs and plans explain decisions but do not define current behavior;
- a maintenance link to `DOCUMENTATION_MAINTENANCE.md`.

- [x] **Step 3: Verify current authority boundaries**

Run:

```bash
rg -n 'REQ-[A-Z0-9-]+|后续支持 Windows|公开销售|M4' docs/PRODUCT_SPEC.md docs/product/README.md
rg -n '0\.1\.6|4f1e839763ba8cef143701bd393f023e445baad6|持续开发版' docs/PRODUCT_SPEC.md docs/product/README.md
git diff --check
```

Expected: the first command prints no matches; the second finds the required version/baseline/status metadata; diff check exits zero.

- [x] **Step 4: Commit the product authority**

```bash
git add docs/PRODUCT_SPEC.md docs/product/README.md
git commit -m "docs: establish Viewer 0.1.6 product baseline"
```

---

### Task 3: Write the task-oriented user guide and interaction reference

**Files:**

- Create: `docs/product/USER_GUIDE.md`
- Create: `docs/product/SHORTCUTS.md`

**Interfaces:**

- Consumes: actual labels and ownership rules from `ui/src/App.tsx`, `ui/src/components/EmptyProject.tsx`, `ContentBrowser.tsx`, `SearchToolbar.tsx`, `ImagePreview.tsx`, `TextPreview.tsx`, `CompareWorkspace.tsx`, `VideoPreview.tsx`, `WorkspaceMoreMenu.tsx`, `useReviewShortcuts.tsx`, `useVideoShortcuts.tsx`, and their tests.
- Produces: complete operating instructions and one centralized interaction table.

- [x] **Step 1: Build an evidence checklist before writing**

Run and save the relevant facts in working notes, not a committed file:

```bash
rg -n "aria-label=|title=|快捷键|event\.key|event\.code|metaKey|shiftKey|altKey" \
  ui/src/App.tsx ui/src/components ui/src/state/useReviewShortcuts.tsx \
  ui/src/components/videoPreview --glob '*.{ts,tsx}'
```

Cross-check each shortcut against a corresponding UI test. Do not document a key from an old design file unless current code owns it.

- [x] **Step 2: Write `USER_GUIDE.md`**

Use these sections in order:

1. `开始之前` — platform/development assumptions and local-project warning.
2. `启动开发版` — `pnpm start:viewer`, single-instance launcher behavior, log path, and what it does not do.
3. `打开项目` — picker, folder drop, validation, progressive scan, read-only outcome.
4. `认识工作区` — sidebar/tree, toolbar, dynamic content area, task surface.
5. `浏览目录和内容` — category overview, content folders, aggregate descendants, image/text/video partitions, density.
6. `选择文件` — click, Command-toggle, Shift-range, marquee, type-specific select-all in mixed folders.
7. `搜索、筛选和排序` — whole project/subtree, fuzzy name/path, text snippets, filters, clearing and returning.
8. `审阅和收藏` — keep/pending/reject/clear/favorite for one or many items.
9. `预览图片` — fit, zoom/pan, rotation, navigation, magnifier and session reset.
10. `预览文本` — Markdown/TXT, two panes, encoding changes, copy, truncation, external links.
11. `对比图片` — valid image count from current code, sync/independent transforms, removal/exit, read-only availability.
12. `预览视频` — open/prepare/ready/error, controls, timeline, stepping, rate menu, full screen, end-of-playback replay.
13. `整理文件` — rename, batch rename, copy, move, Trash, conflict choices, Finder export.
14. `撤销与操作结果` — exactly which operations are undoable, session boundary, unsafe reversal behavior.
15. `处理外部变化和权限` — watcher repair, deleted current item, read-only banner, reselection/settings recovery.
16. `设置与缓存` — five density levels, magnifier shape/rate/area, video-cache stats and clear.
17. `关闭项目与退出` — active-operation choices, derived-task cancellation, no implicit project restore.

Every task section includes: entry point, precondition, action, visible result, and a short “如果未完成” recovery link to `TROUBLESHOOTING.md`.

- [x] **Step 3: Write `SHORTCUTS.md`**

Organize verified controls by context:

- global/project browser;
- selection and review;
- image preview;
- compare mode;
- video preview;
- dialogs and text inputs;
- mouse/trackpad gestures.

For each shortcut include columns `操作`, `按键或手势`, `适用界面`, `限制`. Explicitly state that single-key review commands and preview shortcuts are suppressed when an editable control owns focus, during input-method composition, or when modifiers change ownership. Do not infer native macOS shortcuts that are not handled or explicitly supported by the app.

- [x] **Step 4: Validate labels and cross-links**

Run:

```bash
rg -n '打开文件夹|显示全部后代|保留|待定|淘汰|清除视频缓存|重新选择|完成' \
  docs/product/USER_GUIDE.md docs/product/SHORTCUTS.md ui/src
rg -n '\]\((TROUBLESHOOTING|FEATURE_REFERENCE|SUPPORTED_FORMATS|DATA_PRIVACY)\.md' \
  docs/product/USER_GUIDE.md docs/product/SHORTCUTS.md
git diff --check
```

Expected: every documented label has a current UI occurrence, focused-guide links are present, and diff check exits zero.

- [x] **Step 5: Commit user-facing workflows**

```bash
git add docs/product/USER_GUIDE.md docs/product/SHORTCUTS.md
git commit -m "docs: add Viewer user and interaction guides"
```

---

### Task 4: Write the detailed feature and supported-format references

**Files:**

- Create: `docs/product/FEATURE_REFERENCE.md`
- Create: `docs/product/SUPPORTED_FORMATS.md`

**Interfaces:**

- Consumes: `ui/src/api/types.ts`, `ui/src/fileKinds.ts`, `crates/viewer-domain/src/file.rs`, `crates/viewer-infrastructure/src/scan/file_classifier.rs`, text preview services, video probe/runtime code, Tauri commands, and integrated tests.
- Produces: a searchable statement of actual behavior and a format table that distinguishes recognition from successful decode.

- [x] **Step 1: Extract exact format and limit facts**

Run:

```bash
sed -n '1,220p' crates/viewer-domain/src/file.rs
sed -n '1,180p' crates/viewer-infrastructure/src/scan/file_classifier.rs
rg -n '10 \* 1024 \* 1024|10_485_760|GB18030|UTF_16|UTF-16|MAX_|limit|truncate' \
  crates/viewer-application crates/viewer-infrastructure src-tauri ui/src --glob '*.{rs,ts,tsx}'
sed -n '1,260p' scripts/video/runtime.lock.json
```

Record exact extension sets, encoding choices, text bounds, video-runtime versions/options, cache budget, and failure kinds. Treat an extension classifier as a scan-candidate list, not a codec guarantee.

- [x] **Step 2: Write `FEATURE_REFERENCE.md`**

Use one section per domain:

- project/session lifecycle;
- scanning and folder projection;
- content layout and progressive thumbnails;
- selection;
- search/filter/sort;
- markers;
- image preview and magnifier;
- text preview;
- compare;
- video browse and playback;
- file operations and conflict handling;
- undo and operation recovery;
- Finder drag/export;
- external change reconciliation;
- read-only and error containment;
- settings and task feedback;
- accessibility behavior actually covered by code/tests.

Each section contains `状态`, `用户行为`, `边界与失败`, and `维护证据`. Evidence uses repository-relative code/test paths and names at least one current component/command and one relevant test where available. Clearly distinguish code-level capability from fully validated native behavior.

- [x] **Step 3: Write `SUPPORTED_FORMATS.md`**

Use separate tables for:

- images: `.jpg`, `.jpeg`, `.png`, supported preview versus `unsupported_image` containment;
- text: `.md`, `.markdown`, `.txt`, read-only preview, supported encodings, sanitization, remote-resource rejection, size/truncation limits;
- video candidates: the exact case-insensitive extension list from `file_classifier.rs`, probe/decode dependency, bundled mpv/FFmpeg runtime, cover/timeline behavior, fixture-proven examples, and failure categories;
- other files and directories: indexed/displayed behavior versus no in-app preview;
- explicitly unsupported product behaviors: editing, remote streams, playlists, subtitle/audio-track management, and cloud sources.

Add a prominent rule: “被扫描识别不代表文件内容一定可解码；实际结果取决于文件内容、权限、完整性和打包运行时。”

- [x] **Step 4: Cross-check evidence and format claims**

Run:

```bash
rg -n '\.(jpg|jpeg|png|md|markdown|txt|mp4|m4v|mov|mkv|webm|avi|wmv|mpg|mpeg|mts|m2ts|flv|ogv|3gp)' \
  docs/product/SUPPORTED_FORMATS.md crates/viewer-infrastructure/src/scan/file_classifier.rs \
  crates/viewer-domain/src/file.rs
rg -n '维护证据|状态|边界与失败' docs/product/FEATURE_REFERENCE.md
git diff --check
```

Expected: every candidate extension in code appears in the format reference, every feature section has evidence/boundary fields, and diff check exits zero.

- [x] **Step 5: Commit behavior references**

```bash
git add docs/product/FEATURE_REFERENCE.md docs/product/SUPPORTED_FORMATS.md
git commit -m "docs: document Viewer features and supported formats"
```

---

### Task 5: Document data/privacy, troubleshooting, and maintenance

**Files:**

- Create: `docs/product/DATA_PRIVACY.md`
- Create: `docs/product/TROUBLESHOOTING.md`
- Create: `docs/product/DOCUMENTATION_MAINTENANCE.md`

**Interfaces:**

- Consumes: portable metadata, session index/cache, video cache, CSP/capabilities, error DTOs, recovery logic, `SECURITY.md`, and troubleshooting states in UI tests.
- Produces: explicit trust boundaries, recoverable-data rules, symptom-based recovery, and a repeatable next-version update process.

- [x] **Step 1: Verify storage and network boundaries**

Inspect:

```bash
sed -n '1,260p' src-tauri/tauri.conf.json
sed -n '1,260p' src-tauri/capabilities/main.json
rg -n '\.viewer|cache|Caches|Application Support|sqlite|remote|network|external link|Trash' \
  crates src-tauri ui/src SECURITY.md --glob '*.{rs,ts,tsx,md,json}'
```

Classify data into source files, portable project metadata, ephemeral session indexes/thumbnails, bounded video cache, settings, diagnostics/logs, and explicit external-link handoff.

- [x] **Step 2: Write `DATA_PRIVACY.md`**

Include:

- filesystem-as-source-of-truth model;
- what Viewer reads and what it may mutate after explicit commands;
- `.viewer` portable identity/marker data and reserved-path handling;
- session indexes and image artifacts as rebuildable derived data;
- video cache location semantics, 1 GiB budget, verification marker, and safe clear behavior;
- settings persistence scope;
- no account, telemetry, automatic upload, cloud sync, automatic updater, shell access, or implicit remote Markdown resource loading;
- external links open only after user action in the system browser;
- project-root canonicalization, symlink/alias boundaries, CSP, and redacted user-facing errors;
- backup guidance: source project and `.viewer` metadata matter; caches need not be backed up.

Avoid stating an absolute filesystem location unless current code/config guarantees it across the supported environment.

- [x] **Step 3: Write `TROUBLESHOOTING.md`**

Use a consistent four-column table: `现象`, `可能原因`, `处理方法`, `不会发生的副作用`. Cover:

- project cannot open / another project is active;
- root lost read permission / project opens read-only;
- scan appears incomplete / task fails / watcher rescan;
- missing or failed thumbnail;
- damaged or unsupported image;
- text encoding failure, truncation, and blocked remote content;
- compare candidate rejection;
- video cover missing, preparing timeout, unsupported/damaged/missing video, render surface error, retry, end-screen replay, and cache clear;
- rename/move/copy conflicts, partial operation results, cancellation semantics, and blocked close;
- externally deleted/renamed current item;
- stale development process and launcher log location.

For destructive-looking recovery, explicitly state whether source files, `.viewer` markers, or only rebuildable caches are affected.

- [x] **Step 4: Write `DOCUMENTATION_MAINTENANCE.md`**

Define this version-update order:

1. identify the tagged/baselined commit;
2. audit code/types/tests/config before prose;
3. update product spec and focused references;
4. update user guide/shortcuts only for visible behavior changes;
5. update formats, privacy, and troubleshooting when boundaries change;
6. add a changelog entry;
7. update README and documentation index status/version;
8. run product-doc policy, scope policy, link/version scans, and repository gates;
9. preserve planning documents as historical evidence rather than making them current sources.

Include a change-impact matrix mapping code areas (`ui/src/components`, `ui/src/state`, `src-tauri/src/commands`, scanner/file classifier, portable metadata, video runtime, settings) to the product documents that must be reviewed.

- [x] **Step 5: Validate safety language and commit**

Run:

```bash
rg -n '1 GiB|\.viewer|遥测|自动上传|外部链接|系统浏览器|只读|废纸篓|缓存' \
  docs/product/DATA_PRIVACY.md docs/product/TROUBLESHOOTING.md
rg -n 'PRODUCT_SPEC|USER_GUIDE|FEATURE_REFERENCE|SHORTCUTS|SUPPORTED_FORMATS|DATA_PRIVACY|TROUBLESHOOTING|CHANGELOG' \
  docs/product/DOCUMENTATION_MAINTENANCE.md
git diff --check
```

Expected: all safety/data categories and all maintenance targets are present; diff check exits zero.

Commit:

```bash
git add docs/product/DATA_PRIVACY.md docs/product/TROUBLESHOOTING.md docs/product/DOCUMENTATION_MAINTENANCE.md
git commit -m "docs: add Viewer data and recovery documentation"
```

---

### Task 6: Add version history and rebuild repository documentation navigation

**Files:**

- Create: `CHANGELOG.md`
- Modify: `README.md`
- Modify: `docs/README.md`

**Interfaces:**

- Consumes: current product documents, Git history through tag `v0.1.6`, existing engineering docs, all files in `docs/superpowers/specs/`, and historical plan/review sections.
- Produces: accurate entry points and a status classification compatible with repository policy.

- [x] **Step 1: Derive the 0.1.6 change record from Git**

Inspect the version interval and recent history:

```bash
git log --oneline --decorate --reverse v0.1.5..v0.1.6
git show --stat --oneline v0.1.6
```

Write `CHANGELOG.md` with:

- a statement that entries describe development baselines, not publicly distributed signed releases;
- `## 0.1.6 — 2026-08-23`;
- grouped `新增`, `改进`, `修复`, `文档与工程` bullets supported by commits in the interval;
- an `Earlier development history` note directing readers to Git history rather than inventing complete prior release notes.

- [x] **Step 2: Correct and tighten the root README**

Change current version to `0.1.6`. Update the capability list so it mentions actual video browsing/preview and does not claim that scanning is limited to only image/text files. Keep the development launch/build instructions intact. In `文档与参与`, add links to:

```markdown
- [软件产品规格](docs/PRODUCT_SPEC.md)
- [产品文档](docs/product/README.md)
- [用户指南](docs/product/USER_GUIDE.md)
- [版本记录](CHANGELOG.md)
```

Retain engineering, contribution, security, notices, and acknowledgements links.

- [x] **Step 3: Rebuild `docs/README.md` by authority type**

Use these sections:

1. `当前产品文档` — `PRODUCT_SPEC.md` plus every file under `product/`, status Current.
2. `当前工程与治理文档` — technical foundations, open-source research, dependency health, API baseline, effective ADRs, status Active.
3. `历史目标、设计与实施资料` — scope matrix, preserved target requirements, every dated superpowers spec/plan, reviews, progress, prototypes, and acceptance evidence, statuses Historical or Superseded as applicable.
4. `状态定义` — Current, Active, Historical, Superseded, Development evidence.

Every `docs/superpowers/specs/*.md` file must have exactly one three-column row matching the existing repository-policy grammar:

```markdown
| [label](path) | Active\|Superseded\|Historical | — or replacement link |
```

Mark the approved documentation-reconstruction design Active until implementation verification is complete; at Task 7 it becomes Historical together with this plan.

- [x] **Step 4: Verify navigation and status inventory**

Run:

```bash
node --test --test-name-pattern='documentation index|active governance' scripts/repository-policy.test.mjs
rg -n '当前开发版本：`0\.1\.6`|软件产品规格|产品文档|用户指南|版本记录' README.md
git diff --check
```

Expected: documentation index tests pass, README contains the corrected version and four new links, diff check exits zero.

- [x] **Step 5: Commit entry points and history**

```bash
git add CHANGELOG.md README.md docs/README.md
git commit -m "docs: publish Viewer 0.1.6 documentation index"
```

---

### Task 7: Enforce product-document integrity and complete verification

**Files:**

- Modify: `scripts/repository-policy.test.mjs`
- Modify: `docs/README.md`
- Modify: `docs/superpowers/plans/2026-08-24-viewer-product-documentation-reconstruction.md`
- Test: `scripts/repository-policy.test.mjs`

**Interfaces:**

- Consumes: the completed current product document inventory and Tauri’s authoritative application version.
- Produces: regression protection against missing docs, version drift, broken local Markdown links, unfinished placeholders, and incorrect history status.

- [x] **Step 1: Add a failing current-document integrity test**

Add the `access` helper import, then add:

```js
const currentProductDocuments = [
  'docs/PRODUCT_SPEC.md',
  'docs/product/README.md',
  'docs/product/USER_GUIDE.md',
  'docs/product/FEATURE_REFERENCE.md',
  'docs/product/SHORTCUTS.md',
  'docs/product/SUPPORTED_FORMATS.md',
  'docs/product/DATA_PRIVACY.md',
  'docs/product/TROUBLESHOOTING.md',
  'docs/product/DOCUMENTATION_MAINTENANCE.md',
  'CHANGELOG.md',
]

test('current product documentation is complete, versioned, and locally linked', async () => {
  const tauri = JSON.parse(await read('src-tauri/tauri.conf.json'))
  assert.equal(tauri.version, '0.1.6')

  const documents = await Promise.all(
    currentProductDocuments.map(async (path) => [path, await read(path)]),
  )
  const placeholderPattern = new RegExp(['T' + 'BD', 'T' + 'ODO', '待' + '补充'].join('|'))
  for (const [path, source] of documents) {
    assert.doesNotMatch(source, placeholderPattern, `${path} contains a placeholder`)
    for (const [, target] of source.matchAll(/\]\(([^)]+\.md)(?:#[^)]+)?\)/g)) {
      if (/^[a-z]+:/i.test(target)) continue
      const sourceUrl = new URL(`../${path}`, import.meta.url)
      await assert.doesNotReject(
        access(new URL(target, sourceUrl)),
        `${path} has a broken local link: ${target}`,
      )
    }
  }

  for (const path of ['README.md', 'docs/PRODUCT_SPEC.md', 'docs/product/README.md']) {
    const source = documents.find(([candidate]) => candidate === path)?.[1] ?? (await read(path))
    assert.match(source, new RegExp(`当前(?:开发)?版本[：:]\\s*\\x60?${tauri.version}`))
  }
})
```

Run:

```bash
node --test --test-name-pattern='current product documentation' scripts/repository-policy.test.mjs
```

Expected: FAIL initially if the exact version labels or link parser assumptions do not match the completed files; fix the documents or the narrow parser without weakening the inventory, version, placeholder, or local-link assertions.

- [x] **Step 2: Make integrity enforcement pass**

Normalize the three version labels to:

```markdown
当前开发版本：`0.1.6`
```

Keep the product-spec metadata’s separate baseline commit and fact date. Resolve each reported local link by correcting the target; do not silence missing files.

- [x] **Step 3: Mark implementation records historical**

In `docs/README.md`, change the documentation-reconstruction design and this implementation plan to `Historical`, each appearing exactly once. Check every task box in this plan only after its command/output has been observed.

- [x] **Step 4: Run focused product-document verification**

Run:

```bash
node --test scripts/repository-policy.test.mjs scripts/scope-coverage.test.mjs
node scripts/check-scope-coverage.mjs
rg -n '当前开发版本：`0\.1\.5`' README.md docs/PRODUCT_SPEC.md docs/product docs/README.md
git diff --check
```

Expected: both Node test files pass; scope counts remain stable; the current-version scan prints no matches; diff check exits zero.

- [x] **Step 5: Run repository verification**

Because repository policy code changes, run the full gate:

```bash
pnpm verify
```

Expected: quality and security gates exit zero. Existing explicitly allowed dependency-duplicate warnings may remain warnings; no test, lint, type, build, clippy, license, or security failure is acceptable.

- [x] **Step 6: Inspect final coverage and status**

Run:

```bash
find docs/product -maxdepth 1 -type f -name '*.md' | sort
git diff --stat HEAD~6..HEAD
git status --short --branch
```

Verify the nine current product authorities plus `CHANGELOG.md`, the historical requirement source, the rebuilt indexes, and policy updates are present. The worktree must contain only the final Task 7 changes before commit.

- [x] **Step 7: Commit final policy and verification state**

```bash
git add scripts/repository-policy.test.mjs docs/README.md docs/superpowers/plans/2026-08-24-viewer-product-documentation-reconstruction.md
git commit -m "test: enforce Viewer product documentation integrity"
```

- [x] **Step 8: Verify the committed result**

Run:

```bash
git status --short --branch
git log --oneline --decorate -8
node --test scripts/repository-policy.test.mjs scripts/scope-coverage.test.mjs
```

Expected: clean worktree, local `main` ahead of `origin/main` by the documentation commits, and focused tests exit zero.
