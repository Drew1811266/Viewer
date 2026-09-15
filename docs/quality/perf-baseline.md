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

结构限制（待权衡）：`viewer-image://` 响应 `Cache-Control: no-store`
（安全边界：评审图片字节不得落盘到项目外），WebKit 因此无法跨单元格重挂载
复用解码位图，绘制主线程承担兜底同步解码。若要消除该项，需要把响应头改为
`private, max-age=…` 之类并接受 WebKit 磁盘缓存——属安全边界变更，需 ADR。

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
