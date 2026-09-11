# 性能优化实施方案：解码调度 / 取消与预取 / 内存压力联动 / 性能门禁

状态：设计提案（待评审）
日期：2026-09-11
范围：viewer-infrastructure / viewer-platform-macos / src-tauri（viewer-desktop）/ ui
关联：ADR 0003（会话级实体键）、docs/TECHNICAL_FOUNDATIONS.md

---

## 0. 背景与动机

2026-09-11 的四轮滚动性能采样（`sample` 线程栈 + cputime 增量）表明：

- 滚动期间 WebContent CPU 仅 ~0.14s/25s（≈5% 单核），**无 JS/布局瓶颈**；
- 残余开销集中在 **TileGrid 图层重绘** 与 **JSC GC**，根因是滚动挂载新缩略图时的
  **集中解码**：解码任务无并发闸门、无取消、无预取，涌向主线程的时间片；
- WebContent 物理内存 803MB（峰值 1.1GB），且前端缓存不响应 macOS 内存压力。

本方案回应四个已识别短板（详见当日工作日志），目标：

1. 滚动浏览的帧时间稳定在 60fps 预算内，与缓存冷热无关；
2. 长时间运行（>30min、万级文件项目）内存有界、可收缩；
3. 性能回归在 CI 拦截，而不是靠用户体感发现。

**非目标**：不改评审协议（viewer.review/*）、不改缓存键 schema、不动 mpv/VideoToolbox
视频管线、不引入新状态库。

---

## 1. 已核实的代码事实（设计输入）

| 事实 | 位置 | 对方案的约束 |
|---|---|---|
| 取消链路端到端已存在：`requestId` + `cancel_image_request` + `AbortSignal` 封装 | `ui/src/api/viewer.ts:248`（requestImage） | 方案②只需接线，不加新 IPC |
| 缩略图请求 DTO：`{ kind:'thumbnail', maxPixels, scaleMilli }` | `ui/src/api/viewer.ts:146`、`usePreviewData.ts` | 优先级字段可作可选扩展 |
| 前端缩略图缓存：512 条 LRU（Map 插入序）、inFlight 去重、generation 清空；**无取消、无收缩、无优先级** | `ui/src/app/projectThumbnailCache.ts` | 方案②③的主要改造面 |
| `AspectThumbnail` 不传 signal，卸载即弃单 | `ui/src/components/AspectThumbnail.tsx` | 同上 |
| 磁盘 session cache：2GB 上限、LRU 淘汰、blake3 版本化键 | `viewer-infrastructure/src/session_cache.rs`、`image_cache.rs` | 方案③加"压力收缩"入口 |
| 并发闸门已有两处先例：视频抽帧 Semaphore(1)、评审哈希 Semaphore(≤4) | `video_thumbnail.rs:288`、`review/assets.rs:32` | 方案①复用同一模式，不发明新机制 |
| GPU 侧：瓦片 image pass（TileCoordinate+level）、upload pool 槽位复用、显存退休、gpu_timing | `viewer-render-wgpu/src/{image_pass,upload_pool,gpu_memory,gpu_timing}.rs` | 不动；方案④直接消费 gpu_timing |
| Metal 无头捕获测试机制已存在（`VIEWER_RUN_METAL_TESTS=1`） | `crates/viewer-render-wgpu/tests/` | 方案④基准测试沿用同一门控模式 |

---

## 2. 方案一：统一解码调度器（P0）

### 2.1 设计原则

- **单例闸门 + 优先级队列**：所有"生成图像表示"（缩略图/预览/全图）的解码任务都经过
  同一个调度器获取许可；交互请求插队，预取请求排队。
- **协作式取消**：许可（ticket）drop 即让出并发槽；取消语义复用现有
  `cancel_image_request`，不新增命令。
- **QoS 落地**：worker 线程启动时设置一次 macOS QoS（缩略图 utility、预览
  user-initiated），通过 `pthread_set_qos_class_self_np`，FFI 收敛在一个函数内。

### 2.2 新模块与 API

新增 `crates/viewer-infrastructure/src/decode_scheduler.rs`：

```rust
pub const DEFAULT_DECODE_CONCURRENCY: usize = 3; // ≤ P-core 数，settings 可调 1..=6

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DecodePriority { Prefetch, Interactive }

pub struct DecodeScheduler { /* Semaphore + Mutex<BinaryHeap<Ticket>> */ }

impl DecodeScheduler {
    pub fn new(limit: usize) -> Arc<Self>;
    /// 返回 ticket；ticket 被 drop 或 cancel 时释放槽位并唤醒队首。
    pub async fn acquire(&self, priority: DecodePriority) -> DecodeTicket;
    pub fn set_limit(&self, limit: usize);   // 内存压力联动（方案三）调用
    pub fn snapshot(&self) -> DecodeSchedulerStats; // in_flight/waiting/cancelled（遥测）
}
```

- `DecodeTicket` 实现 `Drop` 释放槽位；内部带 `CancellationToken`（复用
  `ReviewTaskCancellation` 的机制或 tokio token）供解码循环检查。
- 优先级语义：`acquire` 时若槽位被占，任务按 (priority, 入队序) 入堆；
  `Interactive` 到达时唤醒直接给它，`Prefetch` 只在无 Interactive 等待时被唤醒。

### 2.3 接线点

1. **IPC 入口**：`request_image_representation` 的处理函数在进入表示生成前
   `acquire(Interactive)`；`cancel_image_request` 触发 ticket cancel。
   - DTO 扩展：`ImageRequest` 增加可选字段 `priority?: 'prefetch'`
     （缺省 interactive；内部 IPC DTO，非评审协议，不需要 ADR）。
2. **表示生成内部**：`viewer-infrastructure` 中实际执行 ImageIO 解码/编码的
   future（经 platform 层 `image_io.rs`）在关键步骤间检查 cancel token，
   取消时立即返回 `Cancelled`，**不写缓存文件**。
3. **优先级传播**：UI 的预取请求（方案二）带 `priority:'prefetch'`；
   图片预览大图保持 interactive。

### 2.4 与现有闸门的关系

- 视频抽帧 `thumbnail_gate(1)`、评审证据 `evidence_gate(≤4)` **保持独立**：
  它们是业务级限流（mpv 单路、哈希 CPU 密集），与像素管线闸门目标不同。
- 在 `decode_scheduler.rs` 模块注释中写明这一分工，避免后人"统一"掉。

### 2.5 测试

- 单测：并发上限不变量（200 任务并发峰值 == limit）、优先级抢占顺序、
  ticket drop 唤醒、set_limit 收缩时在途任务不被杀只拒绝新任务。
- 集成（`tests/decode_scheduler.rs`）：模拟 50 缩略图 + 中途取消一半，
  断言取消的任务不产生缓存文件、总耗时 ≤ 全量串行时间。
- 既有测试回归：`request_image_representation` 相关 IPC 测试全部加
  scheduler 注入（测试构造 limit=1 以暴露顺序问题）。

---

## 3. 方案二：滚动方向感知的取消与预取（P0）

### 3.1 缩略图请求生命周期接线（取消）

1. **`projectThumbnailCache.ts`**：`ThumbnailLoader` 签名增加第三参
   `signal?: AbortSignal`；abort 时从 `inFlight` 移除该 key（当前会残留，
   导致重挂载拿到已中止的 Promise——顺带修复的真 bug）。
2. **`AspectThumbnail.tsx`**：新增 `signal?: AbortSignal` prop，透传给
   `loadThumbnail`；effect 清理函数中不主动 abort（由宿主管理生命周期）。
3. **`FolderFilmstripRow.tsx`**：对每个挂载项用
   `useMemo(() => new AbortController(), [item.key])`，组件卸载 effect 清理时
   `abort()`。滚动出视口 → 卸载 → 自动取消 → 调度器槽位让给新方向。
4. **图片预览大图**已支持 signal（`requestPreviewImage`），确认调用方传了
   即可，无改动。

### 3.2 方向感知预取

1. **触发点**：`FolderOverview` 的垂直滚动容器监听（rAF 节流 + 滚动静止
   120ms 判定）：
   - 向下滚动：预取视口下方第 1 行的缩略图（density 决定 ~4-6 张）；
   - 向上滚动：对称预取上方 1 行；
   - 快速甩动（速度 > 阈值）时不预取，等静止。
2. **实现**：新增 `ui/src/app/workspace/usePrefetchOnScroll.ts`
   （输入：滚动容器 ref、行高模型、requestThumbnail）；
   预取调用 `thumbnailCache.request(file, w, h, prefetchSignal)`，信号在
   反向滚动发生时 abort。
3. **优先级**：预取请求经 IPC `priority:'prefetch'` 进入调度器低优先级队列，
   **不与交互请求抢槽**。
4. **不做**：跨文件夹预取、视频封面预取（mpv 抽帧成本高，保持点击触发）。

### 3.3 测试

- `projectThumbnailCache.test.ts`：abort 清理 inFlight、同 key 重新请求、
  LRU 行为回归。
- `usePrefetchOnScroll.test.ts`：滚动方向/速度分支、静止判定、反向 abort。
- `FolderFilmstripRow.test.tsx`：卸载时 abort 被调用（用 fake loader 断言）。
- 手动验收脚本：冷缓存冷启动 → 立即快速滚动 → p95 帧时间由性能门禁
  （方案四）度量，不再依赖体感。

---

## 4. 方案三：内存压力联动（P1）

### 4.1 Rust 侧：内存压力源

1. `src-tauri`（viewer-desktop）新增 `memory_pressure.rs`：
   `dispatch_source_create(DISPATCH_SOURCE_TYPE_MEMORYPRESSURE)` 监听
   WARN/CRITICAL（依赖 `dispatch2` crate，新增；若评审不接受新依赖，
   退化为每 60s 轮询 `os_proc_available_memory`，效果次之）。
2. 事件 → `tokio::sync::watch<u8>`（0=normal, 1=warn, 2=critical）广播：
   - `session_cache`：生效限额 = warn 时 `limit/4`、critical 时 `limit/16`，
     立即执行一轮淘汰（复用现有 `while total > limit` 循环）；恢复 normal
     后回到配置值。**只收缩派生缓存，绝不触碰 `.viewer/` 元数据与评审数据**。
   - `decode_scheduler.set_limit()`：warn 时降到 1、critical 时 0（暂停非交互），
     interactive 请求保留 1 个槽。
3. 新增 IPC 事件 `viewer://memory-pressure`（payload: level），并暴露
   `debug_simulate_memory_pressure(level)` 命令（仅 debug 构建注册）供测试。

### 4.2 UI 侧：缓存收缩

1. `projectThumbnailCache.ts` 增加 `shrink(capacity: number)`：LRU 从尾淘汰，
   `clear()` 语义不变（切项目仍用 clear）。
2. 监听 `viewer://memory-pressure`：warn → `shrink(128)`；critical → `clear()`。
3. WKWebView 的解码位图缓存无法直接指令清除，依赖虚拟化卸载 `<img>`
   （已有）+ 收缩后不再重新挂载旧 URL 来自然回落；该限制写入方案注释。

### 4.3 测试

- Rust：`debug_simulate_memory_pressure` 注入 warn → 断言 session cache
  淘汰后 `total_bytes ≤ limit/4`、评审数据文件数不变；恢复 → 限额回位。
- UI：`shrink` 淘汰顺序与容量断言；事件到达后调用 shrink/clear。
- Soak（手动里程碑验收）：脚本模拟 warn/critical 交替，观察 WebContent
  footprint 从 800MB 回落到 <300MB（目标值，见 §6）。

---

## 5. 方案四：性能预算门禁（P1）

### 5.1 基准场景与指标

新增 `crates/viewer-render-wgpu/tests/perf_scroll.rs`
（门控 `VIEWER_RUN_PERF_TESTS=1`，与 Metal 测试同模式，不在默认 CI 跑）：

| 场景 | 构造 | 指标 | 预算（M 系列基准机） |
|---|---|---|---|
| 冷滚动 | 合成项目 30 文件夹 × 30 图，模拟视口逐行下移 60 帧 | 帧时间 p95（gpu_timing + CPU 侧 present 间隔） | p95 < 8ms |
| 缩略图风暴 | 200 张缩略图并发请求（limit=3） | 总耗时、并发峰值 | 峰值 == 3；耗时 ≤ 串行 1.6× |
| 取消代价 | 风暴中途取消 50% | 取消后 100ms 内槽位回收 | ≤ 100ms |

结果写入 `docs/quality/perf-baseline.md`（日期 + 机型 + 数值），超标即失败。

### 5.2 CI 接入

1. 新增 `pnpm verify:perf`（显式执行，不并入 `verify:clean`——避免每次提交
   都跑 Metal 硬件测试；本机门禁先跑，CI runner 有 Apple GPU 后再提升为
   必跑）。
2. `docs/quality/` 增加基线记录约定：任何使 p95 恶化 >10% 的 PR 必须在
   描述中说明并更新基线，否则不允许合入。

---

## 6. 里程碑与验收

| 里程碑 | 内容 | 验收标准 |
|---|---|---|
| M1 | 方案一（调度器 + DTO + 接线 + 测试） | 单测/集成全绿；`verify:clean` 过；风暴基准并发峰值==limit |
| M2 | 方案二（取消接线 + 预取） | 冷启动快速滚动 p95 < 8ms（对比修复前基线记录）；单测全绿 |
| M3 | 方案三（内存压力联动） | 模拟压力事件后 footprint 回落 <300MB；数据完整性断言全绿 |
| M4 | 方案四（门禁 + 基线文档） | `verify:perf` 可重复执行；基线文档入库 |

每个里程碑独立提交（`feat/perf: <scope> <outcome>`），提交前
`pnpm verify:clean`。

**总目标**：冷缓存连续滚动帧时间 p95 < 8ms；30min soak 内存峰值 <1GB 且
压力事件后可回落 <300MB；用户可见行为无任何变化（纯性能改造）。

---

## 7. 风险与回滚

| 风险 | 缓解 | 回滚 |
|---|---|---|
| QoS FFI 误用 | 收敛单函数 + debug 断言返回值 | settings 开关关掉 QoS 设置（不影响功能） |
| 调度器死锁（ticket 泄漏） | Drop 语义 + 单测并发不变量 + `snapshot()` 遥测暴露 in_flight 卡死 | revert PR，无数据迁移 |
| DTO 新字段破坏旧前端 | `priority` 可选、serde default | 字段回退无成本 |
| 内存压力误触发频繁收缩 | warn 级收缩有 30s 冷却窗口；critical 不冷却 | 压力源开关常量置 false |
| 预取造成反向流量放大 | 单行上限 + 反向 abort + 快甩不预取 | `usePrefetchOnScroll` 空实现兜底 |

---

## 8. 决策请求

1. `dispatch2` 新依赖是否接受（方案三的推荐路径 vs 轮询退化）？
2. `verify:perf` 先作为本机显式门禁（推荐），待 CI runner 具备 Apple GPU
   后提升为必跑——是否同意该节奏？
3. 基准机以当前开发机（M 系列）为准记录基线——是否确认？
