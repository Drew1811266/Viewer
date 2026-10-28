# 图片评审标记工具调研

> Status: Research draft（2026-10-28）
>
> 目的：回答“市面上设计/截图/评审工具到底有哪些标记类型，哪些值得放进 Viewer”，
> 为 0.3.0 的工具栏、协议与视觉稿提供依据。
>
> 相关：[0.3.x 开发方案](../milestones/viewer-0.3x-development-plan.md) ·
> [0.3.0 视觉规格](../design/0.3.0/README.md)

## 1. 调研范围

| 产品 | 主要场景 | 观察重点 |
| --- | --- | --- |
| Adobe Photoshop | 图像编辑/标注 | 工具分组、形状、文字、测量、注释 |
| Figma | 界面设计/评审 | 评论、标注、箭头、便签、多人协作 |
| Sketch | 界面设计 | 评论、标注、切片、测量 |
| Adobe Acrobat | 文档评审 | 高亮、便签、图章、绘图、测量 |
| macOS 预览 | 快速标注 | 矩形/椭圆/箭头/文字/签名/高亮 |
| Snagit | 截图说明 | 箭头、序号、聚光灯、放大镜、模糊 |
| CleanShot X | 截图/录屏 | 标注、模糊、聚光灯、快速分享 |
| Markup.io | 网页/图片评审 | 点击标记、评论、任务状态 |
| macOS Quick Look | 快速查看 | 最小化标注能力 |

> 这些产品的完整工具集差异很大，且会随版本变化。本文按“能力类型”归纳，不追求逐项复刻。

## 2. 工具类型全景

### A. 定位（Locate）

| 类型 | 典型产品 | 作用 | Viewer 适配 |
| --- | --- | --- | --- |
| 图钉 / 编号点 | Figma、Markup、Snagit | 在复杂截图上指出“就是这里” | **已有 `ImagePoint`，保留** |
| 箭头 / 引导线 | 预览、Acrobat、Snagit | 从说明文字指向目标 | **已有 `ImageArrow`，扩展线型** |
| 聚光灯 / 聚焦遮罩 | Snagit、CleanShot | 压暗周围，只亮重点 | 高价值，但协议/渲染成本高；P1 |
| 放大镜 | Snagit、CleanShot | 局部放大，保留上下文 | P1，可复用画布缩放 |
| 序号序列 | Snagit、Acrobat | 让多条意见可被口头引用 | 用 point 的 `label` 表达，不新增锚点 |

### B. 圈选（Delimit）

| 类型 | 典型产品 | 作用 | Viewer 适配 |
| --- | --- | --- | --- |
| 矩形 | 全部 | 圈出卡片、按钮、区块 | **已有 `ImageRect`** |
| 椭圆 | 预览、Acrobat | 圈出不规则/圆形控件 | **已有 `ImageEllipse`** |
| 自由画笔 | 预览、Snagit | 快速圈画、手绘强调 | **已有 `ImageStroke`** |
| 直线 | 预览、Acrobat | 下划线、分隔、指向 | 用 arrow 的 `head: none` 表达，避免新锚点 |
| 多边形 | Photoshop、Acrobat | 圈出不规则区域 | `ImageStroke` + `style.shape=polygon`；P0 |
| 高亮笔 | Acrobat、预览 | 半透明覆盖文字/区域 | 用 stroke 的 `opacity/width` 表达；P0 |
| 括号 / 大括号 | Acrobat、Figma 插件 | 标注一段范围 | 可用 polygon/line 组合；P1 |
| 删除线 | Acrobat | 标记“删掉这段” | 用 line + style；P1 |

### C. 说明（Describe）

| 类型 | 典型产品 | 作用 | Viewer 适配 |
| --- | --- | --- | --- |
| 文字标注 | 全部 | 在画布上直接写结论 | `ImagePoint` + `style.shape=text`；P0 |
| 便签 / 气泡 | Figma、Acrobat | 长文本、讨论 | 现有右侧意见输入框已覆盖；画布文字用于短结论 |
| 编号 + 图例 | Snagit、Acrobat | 多条意见对应编号 | point label + 右侧列表；P0 |
| 语音备注 | Figma 插件、Markup | 口述比打字快 | **0.3.0 语音转文字，不存音频；P0** |
| 图章 | Acrobat | 通过/驳回/待定 | 状态字段已覆盖；不新增图章 |

### D. 测量（Measure）

| 类型 | 典型产品 | 作用 | Viewer 适配 |
| --- | --- | --- | --- |
| 标尺 / 距离 | Photoshop、Acrobat、Sketch | 量间距、尺寸 | `ImageArrow` + `style.shape=ruler`；P0（仅图片） |
| 参考线 | Photoshop、Figma | 对齐检查 | 画布辅助线，不进入评审协议；P1 |
| 像素网格 | Photoshop | 1:1 检查 | 缩放 >400% 时显示；P1 |
| 颜色/对比度取样 | Figma 插件、Stark | 检查对比度 | P1，需颜色空间与 WCAG 计算 |
| 坐标/尺寸读数 | Photoshop | 精确检查 | 状态栏 + measure 标签；P0 |

### E. 比较（Compare）

| 类型 | 典型产品 | 作用 | Viewer 适配 |
| --- | --- | --- | --- |
| 前后对比滑块 | Figma 插件、Markup | 对比两版 | P1，需要双图/版本模型 |
| 叠加/洋葱皮 | Photoshop、Figma | 检查偏移 | P1 |
| 版本并排 | Figma、Markup | 评审历史 | 0.3.x 历史已有，并排 P1 |

### F. 弱化/遮挡（Redact / De-emphasize）

| 类型 | 典型产品 | 作用 | Viewer 适配 |
| --- | --- | --- | --- |
| 模糊 / 马赛克 | Snagit、CleanShot | 遮隐私、弱化干扰 | P1；涉及栅格化与不可逆，需明确警告 |
| 裁剪 | 全部 | 聚焦局部 | P0 预览能力，不进评审协议 |
| 遮罩 | Photoshop | 非破坏性隐藏 | P1，需图层 |

### G. 组织（Organize）

| 类型 | 典型产品 | 作用 | Viewer 适配 |
| --- | --- | --- | --- |
| 图层 | Photoshop、Figma | 管理重叠标注 | P1；0.3.0 用右侧列表替代 |
| 多选/框选 | Figma、Photoshop | 批量移动/删除 | P1；0.3.0 先做单选 + 右侧列表 |
| 锁定/隐藏 | Figma、Photoshop | 防止误改 | P1 |
| 分组 | Figma | 多条意见成组 | P1 |
| 撤销/重做 | 全部 | 试错 | P0（草稿作用域） |

## 3. Viewer 现状与差距

0.3.0 之前，Viewer 只有 5 种锚点：

```text
ImagePoint / ImageArrow / ImageStroke / ImageRect / ImageEllipse
```

没有：

- 文字标注（只能写右侧意见，无法在画布上直接落结论）。
- 直线/下划线/高亮（只能用箭头或画笔近似）。
- 多边形（不规则区域只能矩形+画笔）。
- 测量（间距/尺寸只能靠肉眼）。
- 样式（颜色/线宽/透明度固定红色 2px）。
- 多选、图层、只读解释（已归档评审只能看到灰色工具）。

## 4. 筛选标准

每个候选工具按 5 个维度打分（1–5）：

| 维度 | 说明 |
| --- | --- |
| 真实评审价值 | 是否经常遇到；是否能减少一次口头解释 |
| 锚点复用 | 能否用现有 anchor + style 表达，避免协议膨胀 |
| 协议成本 | 新字段/新版本/迁移/跨端读取的成本 |
| UI 成本 | 工具切换、属性栏、快捷键、触控板操作 |
| macOS 可行性 | 现有 Canvas/SVG 渲染与事件模型能否稳定支持 |

## 5. 推荐工具集（0.3.0）

### 5.1 P0：10 种标记 + 选择

| # | 工具 | 锚点 | 新增 | 快捷键 | 说明 |
| --- | --- | --- | --- | --- | --- |
| 1 | 选择/浏览 | — | 否 | V | 选中、移动、删除、微调 |
| 2 | 图钉 | `ImagePoint` | 否 | P | 编号/无编号；颜色可改 |
| 3 | 箭头 | `ImageArrow` | 否 | A | 拖拽直线；可关箭头 |
| 4 | 画笔 | `ImageStroke` | 否 | B | 自由圈画；可调粗细/透明度 |
| 5 | 矩形 | `ImageRect` | 否 | R | 卡片/区块 |
| 6 | 椭圆 | `ImageEllipse` | 否 | O | 圆形控件/头像 |
| 7 | 文字 | `ImagePoint` + `style.shape=text` | 新工具（复用锚点） | T | 画布短结论，双击编辑 |
| 8 | 直线 | `ImageArrow`（head=none） | 否 | L | 下划线/分隔/指向 |
| 9 | 高亮 | `ImageStroke`（opacity<1） | 否 | H | 半透明覆盖，不遮原文 |
| 10 | 多边形 | `ImageStroke`（closed） | 新工具（复用锚点） | G | 不规则区域；双击闭合 |
| 11 | 测量 | `ImageArrow`（ruler） | 新工具（复用锚点） | M | 间距/尺寸；显示 px |

> 实际是 10 种标记 + 选择 = 11 个入口。若担心工具栏过长，把“直线”收进“箭头”长按菜单，
> 把“高亮”收进“画笔”长按菜单，把“多边形”收进“形状”长按菜单；首屏保留 7 个常驻入口：
> 选择、图钉、线条、画笔、形状、文字、标尺。

### 5.2 P1：0.3.x 候选

- 聚光灯/聚焦遮罩（Snagit 风格）：高价值，但需要新的 `ImageSpotlight` 锚点与暗色遮罩渲染。
- 模糊/马赛克：涉及不可逆栅格化，必须二次确认；优先做“导出时遮罩”而不是改原图。
- 曲线箭头/折线箭头：箭头 anchor 增加 `bend` 或控制点；先观察真实使用。
- 便签/贴纸：容易变成装饰，优先级低。
- 图层/分组/锁定：标注数量 >50 时才有明显收益；先用右侧列表 + 单选定位。
- 前后对比/叠加：需要版本双图模型，适合 0.4.x。
- 颜色/对比度取样：设计评审刚需，但需要准确的色彩管理；单独评估。

### 5.3 不进入 Viewer 的工具

- 裁剪/旋转/调色：这是编辑器，不是评审工具；会破坏“原素材不被改写”的承诺。
- 贴纸/表情/形状库：增加视觉噪音，和“结论优先”冲突。
- AI 自动生成标注：0.3.0 不做；先把手动工具做稳。

## 6. 样式模型

不把颜色/线宽塞进每个 anchor，而是新增统一的 `AnnotationStyle`：

```text
AnnotationStyle {
  shape: enum,          // pin/arrow/line/brush/highlighter/rectangle/ellipse/polygon/text/ruler
  color: enum,          // 8 个预设色，不存任意 hex
  width: u8,            // 1 / 2 / 4
  opacity: f32,         // 0.2–1.0
  line_style: enum,     // solid / dashed
  text_size: Option,    // 仅 text
}
```

每个 `FeedbackTarget` 增加可选 `style: AnnotationStyle`（缺省时按 anchor 推导默认样式）。
样式属于“某条意见在某个素材上的一个目标”，不是 anchor 的全局属性；同一意见有多个 target 时，
每个 target 可以有自己的样式。这样既复用现有 anchor，又不用为文字/多边形/标尺新增几何类型。

## 7. 交互细节

### 7.1 工具切换

- 单击工具 = 选中；长按/右键 = 展开同族变体（如箭头：直线/折线/双向）。
- 工具按钮显示图标 + 名称 + 快捷键；激活态用 `secondary-container`，不只靠颜色。
- 选择工具下：单击选中，方向键微移 1px（Shift 10px）；多选/框选留到 0.3.x。
- 画布缩放：⌘+ / ⌘− / 0 适应；空格拖拽平移；触控板双指缩放。

### 7.2 文字标注

- 点击画布 → 出现内联输入框（不是模态对话框）。
- 输入时画布不抢焦点；⌘↵ 保存，Esc 取消。
- 文字过长时自动换行，最大宽度 240px；超出显示“在右侧展开”。
- 文字标注与右侧意见是同一实体：画布显示正文的截断预览（最多 3 行/240px），右侧显示完整正文；双击画布文字重新打开输入框。

### 7.3 测量

- 拖拽两点生成 measure；标签显示 `W × H` 或距离。
- 按住 Shift 约束水平/垂直；Alt 从中心量。
- 需要知道图片的像素比例；非 1:1 时显示“≈”并注明缩放。
- 测量是某条意见的一个目标，不单独产生意见；未填写正文时默认给“测量”占位并可编辑。

### 7.4 语音输入

- 麦克风按钮放在意见输入框右下角，不单独占工具栏。
- 录音时显示波形、计时、“本地转写 · 不上传音频”。
- 停止后先插入转写文本，用户可编辑；不自动保存。
- 权限被拒：在输入框内显示解释 + “打开系统设置”，不弹重复系统框。
- 识别失败：保留已识别的前半段，提供“重试/改为手动输入”，不丢草稿。

### 7.5 只读评审

- 顶部 banner 说明“已归档，无法修改标记”，提供“创建新评审”主按钮。
- 工具栏整体禁用，但保留 tooltip 说明原因；不要只变灰。
- 右侧意见仍可复制/导出；画布可缩放、平移、查看。

## 8. 验收标准

| 能力 | 验收 |
| --- | --- |
| 11 个工具入口 | 10 种标记 + 选择都可用鼠标/触控板完成一次操作；快捷键不冲突 |
| 样式 | 颜色/线宽/透明度修改后立即生效，撤销可恢复 |
| 文字 | 双击编辑、⌘↵ 保存、Esc 取消、空文本不创建 |
| 多边形 | 双击/回车闭合；少于 3 点丢弃；可拖动顶点 |
| 测量 | 水平/垂直约束；标签显示 px；缩放后读数正确 |
| 只读 | 所有修改入口禁用且有原因说明；查看/复制/导出可用 |
| 协议 | v5 可读 v4、写 v5；v4 客户端遇到 v5 明确拒绝而不是静默丢字段 |
| 性能 | 100 个标注下平移/缩放 ≥50fps；撤销 <16ms |

## 9. 待确认

1. 是否接受“11 个入口、首屏 7 个常驻 + 长按变体”的密度？
2. 测量是否需要角度/面积（当前只承诺距离与水平/垂直约束）？复杂测量可后置到 0.3.x。
3. 聚光灯/模糊是否必须首发？它们会显著增加渲染与协议成本。
4. 是否需要“标尺参考线”进入评审记录？建议只作为本地视图状态，不写入协议。

## 10. 来源

- Photoshop tools：<https://helpx.adobe.com/photoshop/using/tools.html>
- Photoshop 标注/测量：<https://helpx.adobe.com/photoshop/using/annotating-images.html>
- Figma 评论：<https://help.figma.com/hc/en-us/articles/360039824974-Comment-on-frames-and-layers>
- Figma 标注：<https://help.figma.com/hc/en-us/articles/14879772322711-Add-annotations-to-frames>
- Acrobat 评论工具：<https://helpx.adobe.com/acrobat/desktop/share-and-review-documents/review-documents/add-comments.html>
- macOS 预览使用手册：<https://support.apple.com/guide/preview/welcome/mac>
- Snagit：<https://www.techsmith.com/snagit.html>
- CleanShot X：<https://cleanshot.com/>
- Sketch 文档：<https://www.sketch.com/docs/>
- Markup.io：<https://markup.io/>
