# Viewer 统一图片标记工具验证记录

> 状态：验证完成
>
> 设计日期：2026-09-02；最终验证：2026-09-03
>
> 产品阶段：源码开发与本地验证；不包含签名、公证、正式安装包、商店上架、公开发布或发售

## 验证范围

本记录覆盖统一“标记”菜单中的点、箭头、画笔、矩形和椭圆，当前意见的创建／保存／编辑／
删除，放大镜复合渲染，增量作者态保存，`viewer.review/3` 兼容和 `viewer.review/4` 单向提升，以及
Agent current/history 读取门控。视频时间段标注和具体 Agent 连接器不在本轮实现范围。

## 自动化证据

- Domain、协议、Schema、Rust/TypeScript DTO 已覆盖五种图片 Anchor 的有效与无效边界。
- 统一菜单、快捷键、绘制手势、几何调整、两阶段 Escape、Space 临时平移及当前工具保存后保持已覆盖。
- 扩展 Anchor 走现有作者态 Patch、发布队列、脏证据和幂等重试路径；不会引入第二套保存队列。
- 保存 Patch 不替换图片 DOM、缩放或变换状态；只有素材版本确实变化时才重新请求原图。
- 放大镜使用与主画布相同的标记场景，放大图片和标记，不放大交互手柄与文字编辑器。

最终验证命令：

```sh
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
pnpm architecture:boundaries
pnpm test:review-protocol
pnpm test:review-loop
pnpm security
pnpm verify
```

结果：

- UI：154 个测试文件，1413 项通过，1 项按环境约束跳过；类型检查、Biome 和生产构建通过。
- Rust：整个 Workspace 的格式、Clippy `-D warnings`、单元／集成／文档测试通过；仅保留仓库已声明的
  原生视频运行时或手工性能测试跳过项。
- Review 协议：58/58 通过；Review Loop：15/15 通过；架构边界和安全门禁通过。
- `pnpm verify` 在最终代码上以退出码 0 通过。构建仅报告既有的大 Chunk 提示，`cargo-deny`
  仅报告锁文件中已知的重复版本提示，没有新增依赖例外、网络能力或桌面权限。
- `viewer.review/3` 兼容测试
  `opening_and_reading_an_untouched_v3_project_does_not_rewrite_its_index` 通过，确认只读打开不会重写
  旧项目索引。

## 保存延迟证据

Release 模式本地文件系统性能门禁：

- 作者态保存：p50 `0.888 ms`、p95 `1.462 ms`、p99 `2.954 ms`。
- 脏证据选择／生成：1 条 `3.328 ms`、10 条 `1.554 ms`、100 条 `1.143 ms`、1000 条
  `1.750 ms`（均为 p95）。
- 1000 个扩展 Anchor action key：`9.840 ms`。
- 原生桌面从点击“保存”到新标记可见的五次观察值：`95.1 ms`、`75.5 ms`、`84.9 ms`、
  `91.0 ms`、`91.0 ms`；保存后没有图片重载、加载页闪烁或缩放／平移复位。

## 原生桌面与 Agent 读取证据

使用用户指定的 `/Users/abc/Downloads/测试图` 作为真实测试工程，并通过当前 Worktree 的精确进程
运行原生桌面验收。在一张 `6720 × 4480` 的 3:2 图片上完成点、箭头、画笔、矩形、椭圆五种标记
的创建和保存。按住 Shift 创建的圆形在归一化数据中为宽 `0.078914141414`、高
`0.118371212121`，换算到源图均约为 `530.3 px`，证明约束依据源图比例而不是屏幕像素或图片内容。

真实工程随后由 `viewer-review-reader current` 成功读取：

- 状态 `ok`，协议 `viewer.review/4`，角色 `current`。
- Stream `8e0bb230-ca0b-4345-b953-364519aaa064`，Snapshot
  `fc6e8fc0-dbbd-43ec-a1b4-60f78e284026`。
- 19 条当前意见，其中 9 条可直接执行、10 条需要确认，6 项源文件检查。
- 五条原生验收意见保留原始自然语言，并分别输出 `imagePoint`、`imageArrow`、`imageStroke`、
  `imageRect`、`imageEllipse` 的完整几何数据；这些测试意见保留在该测试工程中。

## 视觉状态证据

在 `1024 × 720` 下重新采集 `RVW-33` 至 `RVW-37`：五个场景全部达到 `ready`，均无横向溢出、
Console Error 或 Page Error。

- `RVW-33`：放大镜与复合标记图层均已绘制，放大镜内能看到标记。
- `RVW-34`／`RVW-35`：宽／紧凑布局均显示统一菜单的点、箭头、画笔、矩形、椭圆五项。
- `RVW-36`：箭头选中且显示 2 个编辑手柄。
- `RVW-37`：椭圆选中且显示 4 个编辑手柄。

## 验收中发现并修复的问题

- 统一标记菜单打开时按 Escape 曾继续冒泡到预览层，导致返回网格；现由菜单消费 Escape、关闭菜单并
  恢复触发器焦点，回归测试验证父级不会收到该事件（提交 `6b3202d`）。
- 原生验收助手原先不允许在 mouse-up 保留 Shift，无法准确验证约束圆；现仅禁止无意义的 move
  modifier，并保持 down／drag／up 的修饰键一致。
- 视觉验收曾把弹出菜单项当作工具栏按钮而误报溢出，并错误假设几何手柄嵌套在 Marker 内；现按
  弹出层与 Annotation Layer 的真实 DOM 关系判定，同时对无 DOM Mutation 的异步图片就绪执行有界
  帧探测（提交 `8a72313`）。

## 结论

统一图片标记计划的 14 项任务全部完成。五种工具共享一套归一化几何、作者态增量保存、证据渲染和
Agent 读取链路；旧 v3 项目保持只读兼容，出现扩展 Anchor 时才单向发布为 v4。验收期间发现的产品
事件问题和验收基础设施误判均先以失败测试复现后修复，最终完整验证通过。
