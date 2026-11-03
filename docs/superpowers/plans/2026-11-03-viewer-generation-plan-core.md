# 生成计划 · 无界面核心实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现生成计划的无界面核心——协议解析、校验、素材库、拼装、版本发布、读取器与 MCP 服务，全部可独立测试，不依赖桌面 UI。

**Architecture:** 严格分层的纯函数核心：domain 定义类型与解析，application 提供唯一的校验与拼装函数，infrastructure 负责文件、素材库与两个二进制（reader / MCP）。桌面应用与两个二进制共用同一份校验实现，因此"能否接受一份计划"只有一个答案。

**Tech Stack:** Rust 2024 edition；`serde` / `serde_json`（工作区已有）；`sha2` 用于素材内容寻址（工作区已有）；`blake3` 用于分镜哈希（工作区已有）。**不新增任何 Rust 依赖。**

**Spec:** `docs/superpowers/specs/2026-11-02-viewer-generation-plan-graph-design.md`

## Global Constraints

- Rust edition 2024，`rust-version = "1.87"`，工具链 1.97.0。
- 不新增 Cargo 依赖；`cargo deny --offline --locked check bans licenses sources` 必须继续通过。
- 协议版本常量 `viewer.plan/1`、`viewer.plan-marks/1`、`viewer.plan.run/1`。
- 单文件上限 8 MiB；计划 ≤ 50 组、≤ 500 分镜；每维 ≤ 50 取值；每组引用 ≤ 20；`delta` ≤ 2000 字。
- 素材只写入 `.viewer/refs/`；计划只写入 `.viewer/plans/incoming/`；标记只由 Viewer 写入。
- 所有路径必须 canonicalize 后落在项目根内；拒绝 `.viewer` 前缀以外的越界路径。
- `src-tauri` 不参与本计划；`scripts/` 不实现校验逻辑。

## Review Focus

这些是规范暗示、但没有任何单个任务的测试会自然覆盖的输入类。每条都归到对应任务的步骤里。

1. **声明数量与实际条数不符**（`declaredCount: 8`，实际 7 条）——必须是警告而不是错误，且带路径。
2. **同一份字节以不同文件名导入两次**——必须只落一个文件，两次返回同一 `assetId`。
3. **计划文件写入到一半**（截断的 JSON）——读取器不得崩溃，也不得把半截文件当成最新版。
4. **计划引用了不存在的 `assetId`**——校验报 `asset_missing` 并指出 `shots[i].references[j]`。
5. **同一 `planId` 连续写入两版**——必须取最高有效版本；低版本不得覆盖高版本。

---

### Task 1: 领域类型与严格解析

**Files:**
- Create: `crates/viewer-domain/src/plan/mod.rs`
- Create: `crates/viewer-domain/src/plan/ids.rs`
- Create: `crates/viewer-domain/src/plan/document.rs`
- Create: `crates/viewer-domain/src/plan/error.rs`
- Modify: `crates/viewer-domain/src/lib.rs`（导出 `pub mod plan;`）
- Test: 各文件内 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: 无
- Produces:
  - `pub const PLAN_PROTOCOL_V1: &str = "viewer.plan/1";`
  - `pub const MAX_PLAN_BYTES: u64 = 8 * 1024 * 1024;`
  - `pub struct PlanDocument { plan_id: PlanId, revision: Revision, title: Option<String>, dimensions: Vec<Dimension>, rules: Vec<PlanRule>, groups: Vec<PlanGroup>, shots: Vec<PlanShot>, relocations: Vec<Relocation> }`
  - `pub fn parse_plan(bytes: &[u8], max_bytes: u64) -> Result<PlanDocument, PlanError>`
  - `pub struct PlanError { pub path: String, pub code: PlanErrorCode, pub message: String }`
  - `pub enum PlanErrorCode { UnsupportedVersion, InvalidSchema, DuplicateId, UnknownValue, UnknownGroup, AssetMissing, CountMismatch, RelocationConflict, TooLarge }`

- [ ] **Step 1: 写失败的测试**

```rust
#[test]
fn rejects_unknown_fields() {
    let json = br#"{"protocolVersion":"viewer.plan/1","planId":"p","revision":1,
        "createdAt":"2026-11-03T00:00:00Z","producer":{"name":"x"},
        "groups":[],"shots":[],"surprise":true}"#;
    let err = parse_plan(json, MAX_PLAN_BYTES).unwrap_err();
    assert_eq!(err.code, PlanErrorCode::InvalidSchema);
}

#[test]
fn rejects_newer_protocol_version() {
    let json = br#"{"protocolVersion":"viewer.plan/2"}"#;
    assert_eq!(parse_plan(json, MAX_PLAN_BYTES).unwrap_err().code,
               PlanErrorCode::UnsupportedVersion);
}

#[test]
fn rejects_duplicate_ids_across_groups_and_shots() {
    // group id "x" 与 shot id "x" 冲突
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p viewer-domain plan::`
Expected: 编译失败，`parse_plan` 不存在

- [ ] **Step 3: 实现类型与解析**

`serde` 派生加 `#[serde(rename_all = "camelCase", deny_unknown_fields)]`；
`parse_plan` 先比较 `protocolVersion` 再反序列化；超限返回 `TooLarge`；
所有错误带 `path`（如 `shots[3].references[1]`）。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p viewer-domain plan::`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add crates/viewer-domain/src/plan crates/viewer-domain/src/lib.rs
git commit -m "feat(domain): add generation plan document types and strict parsing"
```

---

### Task 2: 唯一校验函数

**Files:**
- Create: `crates/viewer-application/src/plan/mod.rs`
- Create: `crates/viewer-application/src/plan/validate.rs`
- Modify: `crates/viewer-application/src/lib.rs`
- Test: `crates/viewer-application/src/plan/validate.rs` 内 `mod tests`

**Interfaces:**
- Consumes: `PlanDocument`、`PlanErrorCode`（Task 1）
- Produces:
  - `pub struct AssetIndexView { /* fn contains(&self, id: &AssetId) -> bool */ }`
  - `pub struct PlanValidation { pub errors: Vec<PlanError>, pub warnings: Vec<PlanWarning> }`
  - `pub struct PlanWarning { pub path: String, pub code: PlanWarningCode, pub message: String }`
  - `pub enum PlanWarningCode { CountMismatch }`
  - `pub fn validate_plan(plan: &PlanDocument, assets: &dyn AssetIndexView) -> PlanValidation`

- [ ] **Step 1: 写失败的测试**

```rust
#[test]
fn declared_count_mismatch_is_a_warning_not_an_error() {
    // group declaredCount 8，shots 里该组只有 7 条
    let v = validate_plan(&plan, &index);
    assert!(v.errors.is_empty());
    assert_eq!(v.warnings.len(), 1);
    assert_eq!(v.warnings[0].code, PlanWarningCode::CountMismatch);
    assert_eq!(v.warnings[0].path, "groups[0].declaredCount");
}

#[test]
fn missing_asset_points_at_the_exact_reference() {
    let v = validate_plan(&plan, &empty_index);
    assert_eq!(v.errors[0].code, PlanErrorCode::AssetMissing);
    assert_eq!(v.errors[0].path, "shots[3].references[1]");
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p viewer-application validate_plan`
Expected: FAIL，函数不存在

- [ ] **Step 3: 实现校验**

顺序固定：id 唯一性 → 分组存在性 → 维度取值合法 → 素材存在 → 数量声明 → 重定位冲突。
数量不符写 `warnings`，其余写 `errors`；不短路，一次返回全部问题。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p viewer-application validate_plan`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add crates/viewer-application/src/plan crates/viewer-application/src/lib.rs
git commit -m "feat(application): add the single plan validation function"
```

---

### Task 3: 内容寻址素材库

**Files:**
- Create: `crates/viewer-infrastructure/src/plan/mod.rs`
- Create: `crates/viewer-infrastructure/src/plan/assets.rs`
- Create: `crates/viewer-infrastructure/src/plan/assets_index.rs`
- Modify: `crates/viewer-infrastructure/src/lib.rs`
- Test: 两个文件内 `mod tests`

**Interfaces:**
- Consumes: 无
- Produces:
  - `pub struct AssetStore`
  - `pub struct AssetId([u8; 32])` + `AssetId::from_hex(&str) -> Result<Self, AssetError>` + `as_hex(&self) -> String`
  - `pub struct ImportedAsset { pub asset_id: AssetId, pub path: PathBuf, pub deduped: bool }`
  - `pub fn open(project_root: &Path) -> Result<AssetStore, AssetError>`
  - `pub fn import_path(&self, source: &Path) -> Result<ImportedAsset, AssetError>`
  - `pub fn import_bytes(&self, bytes: &[u8], extension: &str) -> Result<ImportedAsset, AssetError>`
  - `pub fn contains(&self, id: &AssetId) -> bool`

- [ ] **Step 1: 写失败的测试**

```rust
#[test]
fn same_bytes_under_different_names_collapse_to_one_asset() {
    let store = open(tmp()).unwrap();
    let a = store.import_bytes(PNG_BYTES, "png").unwrap();
    let b = store.import_bytes(PNG_BYTES, "png").unwrap();
    assert_eq!(a.asset_id, b.asset_id);
    assert!(b.deduped);
    assert_eq!(std::fs::read_dir(tmp().join(".viewer/refs")).unwrap().count(), 2); // 1 文件 + index.json
}

#[test]
fn rejects_non_image_bytes() { /* 期望 AssetError::UnsupportedFormat */ }
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p viewer-infrastructure plan::assets`
Expected: FAIL

- [ ] **Step 3: 实现**

`sha2` 计算 sha256 → 目标 `.viewer/refs/<hex>.<ext>`；存在即 `deduped: true`；
解码校验复用现有图片管线的 UTI 白名单；`index.json` 追加式写入（临时文件 + rename）。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p viewer-infrastructure plan::assets`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add crates/viewer-infrastructure/src/plan crates/viewer-infrastructure/src/lib.rs
git commit -m "feat(infra): add content-addressed reference asset store"
```

---

### Task 4: 拼装与分镜哈希

**Files:**
- Create: `crates/viewer-application/src/plan/compose.rs`
- Test: 同文件 `mod tests`

**Interfaces:**
- Consumes: `PlanDocument`（Task 1）
- Produces:
  - `pub struct ComposedShot { pub shot_id: String, pub prompt: String, pub segments: Vec<PromptSegment>, pub references: Vec<EffReference>, pub hash: String }`
  - `pub struct PromptSegment { pub source: SegmentSource, pub text: String }`
  - `pub enum SegmentSource { Group, Value { dimension: String, value: String }, Shot }`
  - `pub fn compose_plan(plan: &PlanDocument) -> Vec<ComposedShot>`

- [ ] **Step 1: 写失败的测试**

```rust
#[test]
fn composes_group_then_values_in_declaration_order_then_delta() {
    let shots = compose_plan(&plan);
    let s = &shots[0];
    let sources: Vec<_> = s.segments.iter().map(|x| x.source.clone()).collect();
    assert_eq!(sources, vec![Group, Value{dimension:"series",value:"summer"},
                             Value{dimension:"subject",value:"boy"}, Shot]);
}

#[test]
fn explicit_prompt_overrides_composition() { /* segments 只剩 Shot */ }

#[test]
fn hash_changes_when_a_shared_fragment_changes() {
    // 改一个 value 的 fragment，断言该组所有分镜 hash 变化，其他组不变
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p viewer-application compose_plan`
Expected: FAIL

- [ ] **Step 3: 实现**

按 `dimensions` 声明顺序取取值 `fragment`，跳过空值；`shot.prompt` 非空时只用它；
`hash` = 对（分镜属性、片段文本、参考图 id 排序、delta）做 blake3。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p viewer-application compose_plan`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add crates/viewer-application/src/plan/compose.rs
git commit -m "feat(application): compose shot prompts and content hashes"
```

---

### Task 5: 版本发布与索引

**Files:**
- Create: `crates/viewer-infrastructure/src/plan/repository.rs`
- Create: `crates/viewer-infrastructure/src/plan/publish.rs`
- Test: 两个文件内 `mod tests`

**Interfaces:**
- Consumes: `parse_plan`、`validate_plan`
- Produces:
  - `pub struct PublishedPlan { pub plan: PlanDocument, pub blake3: String, pub ignored: Vec<IgnoredRevision> }`
  - `pub struct PlanRepository`
  - `pub fn open(project_root: &Path) -> Result<PlanRepository, RepoError>`
  - `pub fn current(&self, plan_id: &PlanId) -> Result<PublishedPlan, RepoError>`
  - `pub fn list(&self) -> Vec<PlanSummary>`

- [ ] **Step 1: 写失败的测试**

```rust
#[test]
fn picks_the_highest_valid_revision() { /* r1 有效、r2 有效 → 返回 r2 */ }

#[test]
fn truncated_highest_revision_falls_back_to_previous() {
    // 写 r3 为半截 JSON；current() 仍返回 r2，且 ignored 含 r3
}

#[test]
fn higher_invalid_revision_does_not_hide_lower_valid_one() { /* 同上，语义不同 */ }
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p viewer-infrastructure plan::repository`
Expected: FAIL

- [ ] **Step 3: 实现**

扫描 `incoming/<planId>/`，只接受 `^\d{1,9}\.json$`；按 revision 降序尝试解析+校验，
返回第一个通过者；解析失败的更高版本记入 `ignored`，重试 3 次、每次 50 ms 后放弃。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p viewer-infrastructure plan::repository`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add crates/viewer-infrastructure/src/plan/repository.rs crates/viewer-infrastructure/src/plan/publish.rs
git commit -m "feat(infra): publish the highest valid plan revision"
```

---

### Task 6: 读取器二进制与 CLI

**Files:**
- Create: `crates/viewer-infrastructure/src/plan/reader.rs`
- Create: `crates/viewer-infrastructure/src/bin/viewer-plan-reader.rs`
- Create: `scripts/plan-protocol/read-plan.mjs`
- Create: `scripts/plan-protocol/read-marks.mjs`
- Modify: `package.json`（新增 `build:plan-reader`）
- Test: `crates/viewer-infrastructure/src/plan/reader.rs` 内 `mod tests`

**Interfaces:**
- Consumes: `PlanRepository`、`compose_plan`
- Produces:
  - 请求形状 `{ "protocolVersion": "viewer.plan.reader/1", "operation": "current" | "list" | "marks", ... }`
  - `pub fn run_plan_reader<R: Read, W: Write>(input: R, output: W) -> bool`

- [ ] **Step 1: 写失败的测试**

```rust
#[test]
fn current_returns_composed_prompts_and_hashes() {
    let out = run_with(br#"{"protocolVersion":"viewer.plan.reader/1","operation":"current","projectRoot":"…","planId":"p"}"#);
    assert_eq!(out["status"], "ok");
    assert!(out["shots"][0]["prompt"].as_str().unwrap().contains("海边"));
    assert!(out["shots"][0]["hash"].is_string());
}

#[test]
fn unsupported_version_does_not_report_corruption() {
    assert_eq!(out["code"], "unsupported_version");
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p viewer-infrastructure plan::reader`
Expected: FAIL

- [ ] **Step 3: 实现**

一次性进程、stdin/stdout、请求 ≤ 64 KiB、响应 ≤ 64 MiB；错误码沿用
`no_plan` / `unknown_plan` / `invalid_plan` / `unsupported_version` / `unsafe_path` / `io`。
Node CLI 只做参数解析与退出码，逻辑全部在 Rust。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p viewer-infrastructure plan::reader && pnpm build:plan-reader`
Expected: PASS，二进制生成于 `target/debug/viewer-plan-reader`

- [ ] **Step 5: 提交**

```bash
git add crates/viewer-infrastructure/src/plan/reader.rs \
        crates/viewer-infrastructure/src/bin/viewer-plan-reader.rs \
        scripts/plan-protocol package.json
git commit -m "feat: add plan reader binary and CLI wrappers"
```

---

### Task 7: MCP 服务

**Files:**
- Create: `crates/viewer-infrastructure/src/plan/mcp/mod.rs`
- Create: `crates/viewer-infrastructure/src/plan/mcp/tools.rs`
- Create: `crates/viewer-infrastructure/src/plan/mcp/schema.rs`
- Create: `crates/viewer-infrastructure/src/bin/viewer-plan-mcp.rs`
- Modify: `package.json`（新增 `build:plan-mcp`）
- Test: `crates/viewer-infrastructure/src/plan/mcp/mod.rs` 内 `mod tests`

**Interfaces:**
- Consumes: 校验、素材库、仓库（Task 2–5）
- Produces:
  - stdio JSON-RPC 2.0：`initialize`、`tools/list`、`tools/call`
  - 工具：`plan_rules`、`plan_validate`、`plan_write`、`plan_current`、`plan_list`、`marks_read`、`assets_import`
  - `pub fn serve<R: Read, W: Write>(input: R, output: W)`

- [ ] **Step 1: 写失败的测试**

```rust
#[test]
fn tools_list_advertises_the_seven_tools() { /* 名称按字母序断言 */ }

#[test]
fn plan_write_rejects_invalid_and_returns_located_errors() {
    // 调用 plan_write 传 declaredCount 与条数不符的计划 → 返回 ok:true 但带 warnings
    // 传 asset_missing 的计划 → 返回 isError:true，错误路径精确到 shots[0].references[0]
}

#[test]
fn assets_import_is_idempotent() { /* 同一路径导入两次返回同一 assetId */ }
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p viewer-infrastructure plan::mcp`
Expected: FAIL

- [ ] **Step 3: 实现**

不引入 MCP SDK：JSON-RPC 2.0 over stdio 手写，请求/响应各一行 JSON。
`plan_write` 写入前必须先跑 `validate_plan`；有 error 则拒绝写入并返回 `isError: true`。
`plan_rules` 返回 `PLAN_PROTOCOL_V1` 常量、字段表与两条示例（字符串常量，内嵌 `include_str!`）。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p viewer-infrastructure plan::mcp`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add crates/viewer-infrastructure/src/plan/mcp \
        crates/viewer-infrastructure/src/bin/viewer-plan-mcp.rs package.json
git commit -m "feat: add stdio MCP server for generation plans"
```

---

### Task 8: 文档、ADR 与门禁接线

**Files:**
- Create: `docs/adr/0012-generation-plan-protocol-and-mcp-boundary.md`
- Create: `docs/protocol/plan-protocol.md`
- Modify: `docs/PRODUCT_SPEC.md`（§3 边界、§9 限制）
- Modify: `docs/README.md`（ADR 0012 与协议文档登记）
- Modify: `package.json`（`quality` 加入 plan 测试）
- Test: `scripts/repository-policy.test.mjs`（新增断言：MCP 二进制不被 UI 直接引用）

**Interfaces:**
- Consumes: 前七个任务的全部产物
- Produces: 无（收口任务）

- [ ] **Step 1: 写失败的测试**

```js
test('plan MCP server is a stdio binary and never a listening service', async () => {
  const binary = await read('crates/viewer-infrastructure/src/bin/viewer-plan-mcp.rs')
  assert.doesNotMatch(binary, /TcpListener|bind\(/)
  assert.match(binary, /serve\(/)
})
```

- [ ] **Step 2: 运行确认失败**

Run: `node --test --test-name-pattern="stdio binary" scripts/repository-policy.test.mjs`
Expected: FAIL

- [ ] **Step 3: 写 ADR 与协议文档，并接线到门禁**

ADR 必须写清：为什么是 stdio 子进程而不是本地服务、允许的写入路径、与现有
"无网络"承诺的关系。`package.json` 的 `quality` 脚本加入 `cargo test -p viewer-infrastructure plan::`。

- [ ] **Step 4: 运行全量门禁**

Run: `pnpm verify:clean`
Expected: PASS，且 `git status --porcelain` 为空

- [ ] **Step 5: 提交**

```bash
git add docs package.json scripts
git commit -m "docs: record the generation plan protocol and MCP boundary"
```

---

## Self-Review

**Spec coverage：** 协议（T1）、校验（T2）、素材库（T3）、拼装与哈希（T4）、版本发布（T5）、
读取器（T6）、MCP（T7）、ADR 与边界改写（T8）。设计 §12 的验收 1/2/3/4/8/10 由 T2–T7 覆盖；
5/6/7（版本、影响面、改名）依赖 UI 与状态层，属于后续计划 B。

**Review Focus 落点：** ①T3 的 `declared_count_mismatch` 测试；②T3 的 `same_bytes_under_different_names`；
③T5 的 `truncated_highest_revision_falls_back`；④T2 的 `missing_asset_points_at_the_exact_reference`；
⑤T5 的 `picks_the_highest_valid_revision`。

**类型一致性：** `AssetId` 在 T1（计划引用）与 T3（素材库）中同名同语义；
`validate_plan` 在 T2 定义、T5 与 T7 消费；`compose_plan` 在 T4 定义、T6 消费。

**未覆盖：** UI 侧（画布、标记、抽屉、变更影响）与 `@xyflow/react` / `@dagrejs/dagre` 集成
属于计划 B，不在本文件内。
