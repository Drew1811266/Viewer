# Viewer 0.3.0 标记工具交互审计

日期：2026-11-01
范围：评审工作台 11 个标记工具（选择/浏览、点、箭头、画笔、矩形、椭圆、文字、直线、荧光笔、多边形、标尺）

## 1. 对标结论

主流图像编辑/截图标注软件的约定可以归纳为五条：

1. **拖拽类工具**（矩形、椭圆、直线、箭头、画笔、荧光笔）按下开始、松开提交；拖拽中 `Esc` 取消。
2. **点击类工具**（点、文字）单击落点；文字在落点直接进入编辑，`Cmd/Ctrl+Enter` 或明确按钮提交。
3. **多点击折线/多边形**必须提供至少两条结束路径：点回起点闭合，以及 `Enter`/双击结束；`Backspace` 退掉最后一个顶点，`Esc` 取消整次绘制。
4. **选择/编辑**单击选中，拖控制点改几何，`Delete/Backspace` 删除，双击文字进入文字编辑。
5. **模式提示**必须常驻或紧邻工具：用户不能靠猜知道“下一步该按什么”。Figma 在闭合点显示闭合圆点；Photoshop 的折线工具以点回起点或双击结束。

参考：

- Figma Vector networks（点击已有节点闭合并显示闭合提示；`Esc` 结束开放路径）：
  https://help.figma.com/hc/en-us/articles/360040450213-Vector-networks
- Adobe Photoshop Lasso / Polygonal Lasso（点回起点或双击闭合、`Backspace` 退一点、`Esc` 取消）：
  https://helpx.adobe.com/photoshop/desktop/make-selections/get-started-selections/isolate-specific-areas-in-images-with-lasso-tool.html
- Adobe Photoshop Marquee（拖拽创建，`Shift` 约束比例）：
  https://helpx.adobe.com/photoshop/desktop/make-selections/get-started-selections/make-selections-with-marquee-tools.html
- Apple Preview Markup（形状拖拽创建、选择后显示控制点、文字可再次编辑）：
  https://support.apple.com/guide/preview/annotate-a-pdf-or-image-prvw1005/mac

## 2. 逐工具审计

| 工具 | 当前操作 | 主流约定 | 缺口/结论 |
| --- | --- | --- | --- |
| 选择/浏览 | 单击选中，拖 8 个控制点/箭头端点/多边形顶点 | 单击选中、拖控制点、Delete 删除、双击文字编辑 | 已补 Delete/Backspace 和双击文字（Canvas）；native 双击仍缺 |
| 点 | 单击落点 | 单击落点 | 基本一致；无结束问题 |
| 箭头 | 拖拽起终点 | 拖拽；Shift 约束角度 | 已补 Shift |
| 画笔 | 按住拖动、松开提交 | 按住拖动；撤销 | 一致；无显式“完成”但松开即完成 |
| 矩形 | 拖拽；Shift 约束正方形 | 拖拽；Shift 约束比例 | 一致 |
| 椭圆 | 拖拽；Shift 约束正圆 | 拖拽；Shift 约束比例 | 一致 |
| 文字 | 单击落点，浮动编辑器输入，Cmd+Enter/保存 | 单击输入、双击已有文字编辑、Esc 取消 | Canvas 已补双击；native 仍依赖意见栏“编辑文字” |
| 直线 | 拖拽；Shift 水平/垂直 | 拖拽；Shift 约束 | 一致 |
| 荧光笔 | 按住拖动 | 按住拖动 | 一致 |
| 多边形 | 连续单击加点；仅“点回起点/双击”结束（且无提示） | 点回起点、Enter、双击；Backspace 退一点；Esc 取消；可见完成按钮 | **P0：已补 Enter/双击/完成按钮/退一点/提示；这是用户卡住的根因** |
| 标尺 | 拖拽；Shift 水平/垂直；显示 px | 拖拽测距，Shift 约束 | 交互一致；角度/面积留到 0.3.x |

## 3. 本次已修正

- 多边形绘制中显示浮动操作条：点数、`Enter / 双击 / 点起点完成`、`⌫ 退一点`、`Esc 取消`，并提供可点击的**完成/退一点/取消**。
- 多边形支持 `Enter` 完成、`Backspace/Delete` 退掉最后一个顶点。
- 工具菜单头部按当前工具显示操作说明；多边形明确写出三种结束方式。
- 多边形进行中切换工具时，禁用提示改为“按 Enter/双击/点回起点完成，或 Esc 取消”，不再只说“请先保存”。
- 箭头支持 `Shift` 约束角度。
- 选中标注支持 `Delete/Backspace` 删除；Canvas 上双击已选标注进入文字编辑。
- 工具菜单的提示从统一“拖拽”改为按工具区分的说明。

实机验证：2026-11-01 在 Viewer 中连续点击 4 个顶点后，底部出现完成/退一点/取消操作条；按 Enter 进入“返工意见”编辑器，按 Esc 撤销草稿。截图：[polygon-finish-hint.png](audit/polygon-finish-hint.png)；按 Enter 后进入意见编辑器：[polygon-finish-enter.png](audit/polygon-finish-enter.png)。

## 4. 仍建议后续处理

1. **native 双击文字编辑**：native 渲染器拥有指针输入，DOM 语义层只负责键盘/无障碍；需要在 native 事件中增加双击识别，或明确保留意见栏入口。
2. **切换工具的提交策略**：当前有未完成几何时禁止切换（并提示如何完成），这是安全策略；若希望“切换即完成”，需要在控制器层增加显式 commit 语义，不能只改 UI。
3. **多选/框选**：当前一次只能选中一条意见；主流标注工具支持框选多条。评审场景是否需要批量删除/改色需产品确认。
4. **Alt 从中心画形状**、**Shift 45° 约束箭头**等增强项，优先级低于当前结束路径。
5. **标尺角度/面积/多段测量**：设计文档已明确留到 0.3.x。
