# 图片预览回归：根背景合成与解码尺寸取整

日期：2026-09-09。范围：修复图片无法显示，不新增产品功能，不回退原生渲染架构。

## 已确认的原因

1. 最近修复根文档滚动时，误删了 `app.css` 中原生图片预览的根背景透明规则。Metal 视图位于 WebView 下方，根背景不透明会遮住已经生成的图像。根节点禁止滚动与原生预览背景透明是两个独立约束，不能相互替代。
2. 此前解码内存管理改造在 `AccountedNativeImage::decode` 中增加了“系统缩略图尺寸必须精确等于计划尺寸”的校验。截图中的 JPEG 为 6308×4205；mip 1 计划为 3154×2103，Image I/O 实际返回 3154×2102。该 1 像素取整差异被判为 `InvalidResource`，进一步被统一映射为 `image_render_decode_failed`。真实窗口可以生成首帧，但在后续高清层解码时失败。

此前将其归因为“进程重建后会话未建立”，以及认定 CSS 不可能相关，均缺乏证据。临时移除 CSS 后组件测试通过，不是原生合成层的因果隔离实验。没有证据支持回退 ByteHandoff、epoch 或增加自动重试。

## 修复

- 保留 `html/body/#root` 的滚动锁定；仅原生预览期间恢复根背景透明。
- 将系统缩略图的取整与最终资源尺寸分离：原生分配先预留 1 像素取整余量，实际行跨度乘实际高度不得超过预留预算；只接受最多 1 像素的尺寸差异，不移除安全检查。
- 最终预览精确输出计划尺寸。分块使用统一整图坐标变换直接绘制到最终分块缓冲区，包含采样边缘；不创建第二份完整 mip。
- 尺寸一致时仍使用原来的直接裁切路径。
- 删除全部临时诊断日志；未增加前端自动重试、引擎兜底或会话重建补丁。

## 回归验证

- 新增 `image_render_rounding`：横竖奇数尺寸图、冷/热缓存、最末边缘块、无透明缺行、相邻分块重叠像素一致、非等比例预览边界。两个测试修复前均因 `InvalidResource` 失败，修复后通过。
- `nativeComposition.test.ts` 运行实际样式选择器和层叠结果，验证原生预览根背景透明、退出后恢复背景且滚动仍锁定。修复前因根背景不透明失败，修复后通过。
- Core Graphics 红/蓝扫描行测试覆盖裁切变换的上下方向。
- `image_render_memory`：6 项通过。取消解码测试的峰值预算同步包含 1 像素原生取整余量，使用保守上界验证；仍检查取消后不分配输出像素、内存归零，且预算不容纳额外完整 mip。
- `VIEWER_RUN_METAL_TESTS=1 cargo test -p viewer-desktop --lib image_render_runtime -- --test-threads=1`：44 项通过。
- `VIEWER_RUN_LARGE_IMAGE_TESTS=1 cargo test -p viewer-platform-macos --test image_render_rounding --test image_render_decode --test image_render_cache -- --test-threads=1`：21 项通过，包括实际 8K 解码。
- 前端 app 样式、原生合成、NativeImageViewport、NativeReviewViewportBridge：70 项通过；UI 静态/类型检查通过（已有 Biome 配置弃用提示）；Rust 格式及 diff 检查通过。
- Metal 测试中另有一个旧断言遗漏已有采样边缘，误将 513×513 实际分块按 512×512 计算。仅修正测试期望，未改变 GPU 生产代码。
- 独立代码审查覆盖本次取整处理、分块坐标与内存预算，未发现具体回归或阻断问题。

## 真实运行证据

使用用户报错的同一 JPEG，通过生产 NativeImageRenderDriver/Metal 窗口执行 `image_render_acceptance`：

- 修复前：首帧提交后稳定出现 `image_render_decode_failed`；临时诊断捕获计划 3154×2103 / 实际 3154×2102。
- 修复后 main：冷首帧约 266 ms、热首帧约 130 ms，完成 240 帧，恢复次数 0。
- 修复后 magnifier：冷首帧约 258 ms、热首帧约 133 ms，完成 240 帧，恢复次数 0。
- 结果文件位于本机 `/tmp/viewer-source-after-fix.json` 与 `/tmp/viewer-source-after-fix-magnifier.json`（临时证据，不作为永久发布产物）。
- 主程序从当前工作树 `target/debug/viewer-desktop` 启动，重新打开“测试图”项目，实际显示 `4-1205749.jpg`，缩放到 125%，打开标记菜单、选择矩形、绘制草稿、弹出意见编辑器、点击取消，切换到下一张并返回。未保存测试意见或修改已有评审内容。

这是针对本次回归的验证，不是全软件无 bug 声明。未执行本轮评审保存、视频、导出等无关流程。开发阶段不涉及签名、公证、正式安装包、上架或发售。
