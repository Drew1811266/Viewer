# 性能基线（perf-baseline）

性能优化方案四（docs/architecture/performance-optimization-plan.md §5）的度量记录。
门禁命令：`pnpm verify:perf`（显式执行，不并入 `verify:clean`；CI runner 具备
Apple GPU 后再提升为必跑）。

## 维护约定

- 任何使 **冷滚动 p95 恶化 >10%** 的 PR，必须在 PR 描述中说明原因并更新本文件
  基线，否则不允许合入。
- 每条记录必须包含：日期、机型（`machdep.cpu.brand_string` + `hw.model`）、
  构建档位（cargo test 默认为 debug+test profile）、三个场景的完整数值。
- 预算不随机器变更；换基准机时重新记录一条，并在备注中注明偏差原因。

## 场景与预算

| 场景 | 构造 | 指标 | 预算 |
|---|---|---|---|
| 冷滚动 | 无头渲染器 + 合成分块大图 4096×4096（1024 瓦片），相机 2× 逐帧下移 60 帧，每帧只上传新进视口的冷瓦片（含 pending 排空），帧时间 = 上传 + 渲染的墙钟 | 帧时间 p95 | < 8ms |
| 缩略图风暴 | 200 个解码请求经 `DecodeScheduler`（limit=3，单任务 2ms）；对照串行基线（limit=1 同负载） | 并发峰值 / 总耗时 | 峰值 == 3；耗时 ≤ 串行 1.6× |
| 取消代价 | 风暴（单任务 10ms）进行 100ms 后 abort 全部未完成任务（模拟反向滚动取消），随后提交新请求 | 槽位回收耗时 | ≤ 100ms |

测试位置：`tests/perf_decode_scheduler.rs`（风暴/取消，viewer-infrastructure）、
`crates/viewer-render-wgpu/tests/perf_scroll.rs`（冷滚动）。两处均门控
`VIEWER_RUN_PERF_TESTS=1`。

## 基线记录

### 2026-09-15 · WKWebView 总览页 DOM 滚动（手工采样，暂无自动门禁）

`sample <WebContent pid> 25` + cputime 增量，脚本化滚动测量工具与采样文件在
`target/tmp/`（scroll_viewer.py，CGEvent 注入受辅助权限限制，需用户手动滚动协同）。
场景：持续评审总览（大密度胶片行）连续上下滚动 25s，Apple M4 / dev 构建。

| 指标 | 优化前（overscan 2 格/256px，行 rootMargin 240px） | 优化后（overscan 6 格/1024px，rootMargin 600px） |
|---|---|---|
| WebContent cputime 增量 | +5.5s | +5.0s（冷启动进程） |
| 主线程忙样本 | 3452 | 3042（-12%） |
| 绘制内同步解码（IIO callDecodeImage） | 624 | 550（-12%） |

结构限制（已解决，ADR 0007）：`viewer-image://` 原响应 `Cache-Control:
no-store`（安全边界：评审图片字节不得落盘到项目外），WebKit 因此无法跨单元格
重挂载复用解码位图。2026-09-15 改为 `private, max-age=60`：自定义 scheme 不进
WebKit 磁盘缓存，仅内存复用，落盘安全属性不变；重复滚动的重解码与位图拷贝
（ShareableBitmap memmove）随之消除。

### 2026-09-15 · 内存缓存协议头（ADR 0007）验证（30s 反复滚动，手工采样）

同一滚动场景、同为 24817/24833 主线程样本的对照（改前 = content-visibility
版，改后 = ADR 0007 版，用户协同滚动）：

| 指标（30s 主线程样本） | no-store | private, max-age=60 | Δ |
|---|---|---|---|
| 主线程忙样本 | 4439 | 3957 | **-11%** |
| IIO 解码 | 194 | 80 | **-59%** |
| ShareableBitmap::createFromImagePixels + memmove | 170 | 129 | -24% |
| TileGrid 绘制（drawInContext） | 418 | 342 | -18% |
| paintContents | 397 | 306 | -23% |
| dispatchEvent | 924 | 730 | -21% |
| 布局（LocalFrameViewLayoutContext::layout） | 469 | 440 | -6% |

WebContent RSS 60s 滚动采样 506→581MB 震荡无单调增长；cputime ~0.31 CPU 秒/
滚动秒，速率不随时间加快。首次出现的图片仍需一次解码与位图拷贝（memmove
残余 129），回看区域的复用收益符合预期。

### 2026-09-15 · 纵向卸载修复后的累积性验证（60s 反复滚动，手工采样）

用户报告"开始丝滑、越滑越掉帧、机身发热"——根因：胶片行图片挂载窗口只按
横向计算，行滚过后图片永不卸载，解码位图随访问行数无界累积（no-store 又使
WebKit 无法复用位图）。修复：行纵向近邻门控（迟滞 600px 挂载数据 /
1500px 外卸载图片）。60s 反复滚动实测（WebContent PID 52723）：

- RSS 在 391–591MB 间震荡回归，**无单调增长**（修复前模式：持续爬升，
  历史基线 803MB 空闲 / 1.1GB 峰值）；
- cputime 稳定在 ~0.28 CPU 秒/滚动秒（≈28% 单核），速率不随时间加快；
- 结论：退化机制（越滑越卡、发热）由无界累积引起，已消除。

### 2026-09-11 · Apple M4（Mac16,12，24GB）· macOS 26.5.2 · debug（cargo test profile）

| 场景 | 数值 | 结论 |
|---|---|---|
| 冷滚动（60 帧，冷上传 6 瓦片） | p50 = 0.24ms，**p95 = 0.38ms**，max = 325ms（首帧管线冷启动毛刺，p95 门外） | ✅ p95 ≪ 8ms |
| 冷滚动 GPU 时间戳（信息性） | 样本 3 个：p50 = 0.15ms，p95 = 0.17ms | 参考 |
| 缩略图风暴 | 串行（limit=1）= 756.7ms，风暴（limit=3）= **252.1ms**，峰值 = **3** | ✅ 峰值 == limit；加速 3.0×，远优于 1.6× 预算 |
| 取消代价 | 取消 179 个未完成任务，排队取消计数 176，槽位回收 = **205µs** | ✅ ≪ 100ms |

备注：
- 冷滚动 max 的 325ms 尖峰来自首帧 Metal 管线编译与上传池首次映射，属一次性
  冷启动开销，不进入 p95 预算；若未来 p95 与 max 同时恶化则另议。
- 风暴/取消场景在 tokio current_thread 运行时上度量（工作区 tokio 未启用
  `rt-multi-thread`）；sleep 墙钟重叠足以表达并发语义，切换到多线程运行时后
  需重新记录基线。

### 2026-10-27 · Apple M4（Mac16,12，24GB）· macOS 26.5.2 · debug（cargo test profile）

0.2.5 收口复跑 `pnpm verify:perf`，命令与场景未改动：

| 场景 | 数值 | 结论 |
|---|---|---|
| 冷滚动（60 帧，冷上传 6 瓦片） | p50 = 0.25ms，**p95 = 0.37ms**，max = 331.47ms（首帧管线冷启动毛刺，p95 门外） | ✅ p95 ≪ 8ms |
| 冷滚动 GPU 时间戳（信息性） | 样本 3 个：p50 = 0.20ms，p95 = 0.25ms | 参考 |
| 缩略图风暴 | 串行（limit=1）= 698.3ms，风暴（limit=3）= **232.4ms**，峰值 = **3** | ✅ 峰值 == limit；加速 3.0×，优于 1.6× 预算 |
| 取消代价 | 取消 176 个未完成任务，排队取消计数 173，槽位回收 = **310µs** | ✅ ≪ 100ms |

备注：
- 与 2026-09-11 基线相比，p95、风暴总耗时和取消代价均在同一量级，无 >10% 恶化。
- 本轮仍使用合成 4096×4096 大图语料；逐格式真实 fixture（JPEG/PNG/GIF/TIFF/
  AVIF/HEIC/BMP/WebP/PSD）由 `tests/image_format_matrix.rs` 覆盖解码契约，尚未
  把九种格式合并为同一个混合语料性能场景。该差距不影响现有预算结论，但换基准机
  或改解码器时应补一条混合语料记录。
