# Viewer 0.3.0 视觉规格（Material Design 3 · 浅色）

> Status: **Correction in progress — do not implement**（2026-10-29）
>
> 审计发现本稿把“M3 视觉升级”做成了新的信息架构，并混入产品没有的能力。
> 在 [2026-10-29 审计](AUDIT-2026-10-29.md) 的决策确认前，本文件不作为实现依据。
> 下文的“21 屏”“导航栏/历史页/轮次”等描述保留作问题记录，不代表已确认方案。
>
> 这套稿子回答的是 **0.3.0 长什么样、每个界面怎么组织、组件如何复用**。
> 它不是最终实现，也不替代前端验收；代码落地时以本文的 token、组件和状态说明为准。
>
> **Pen 源文件**：`/Users/abc/Project/9.22/design.pen`（pen.dev，21 个 1440×900 frame；不进入 Git）。
> 总览：[pen/overview.png](pen/overview.png)。
>
> 本目录的 `screens.html` / `screens/*.png` 是同日早些时候的 23 屏静态探索，保留作对照；
> 其中「17 批量重命名」「18 批量处理图片」已确认延后到 0.3.0 之后，不再属于本轮范围。
>
> 相关：[0.3.x 目标需求](../../milestones/viewer-0.3x-target-requirements.md) ·
> [0.3.x 开发方案](../../milestones/viewer-0.3x-development-plan.md) ·
> [标记工具调研](../../quality/2026-10-28-annotation-tools-research.md)

## 1. 范围与原则

### 1.1 本轮范围

- **只做浅色主题**。不设计深色模式的配色/对比度；深色留到后续版本。
- 桌面端优先，基准画布 **1440 × 900**；不在本轮设计手机/平板布局。
- 覆盖 0.3.0 的全部主要界面：项目选择、工作区、预览、评审、历史、设置、冲突/空态；批量操作延后到 0.3.0 之后。
- pen.dev 视觉稿是静态 frame，不包含真实交互；交互规则写在每屏说明和开发方案中。

### 1.2 设计原则

1. **素材是主角**：界面用中性色，强调色只给主操作、选中态和评审标记。
2. **结论优先**：评审页的视觉动线是“画布 → 意见列表 → 输入”，不让工具栏抢注意力。
3. **可解释的禁用**：归档、无权限、冲突等状态必须说明原因和下一步，不能只变灰。
4. **本地感**：不出现云端同步、账号、团队空间等暗示；文案强调“本机/离线”。
5. **克制的 Material 3**：用 tonal surface、形状、状态层表达层级；不用重阴影、玻璃拟态、渐变装饰。

## 2. 设计 token

### 2.1 颜色

| Token | 值 | 用途 |
| --- | --- | --- |
| `primary` | `#00696D` | 主按钮、选中、焦点 |
| `on-primary` | `#FFFFFF` | 主色上的文字 |
| `primary-container` | `#9CF1F0` | 品牌/图标底、轻强调 |
| `on-primary-container` | `#00201F` | 容器上的文字 |
| `secondary-container` | `#CCE8E7` | 选中行、激活工具、chip |
| `on-secondary-container` | `#051F1F` | 选中行文字 |
| `tertiary-container` | `#D3E4FF` | 头像、次级信息底 |
| `error` | `#BA1A1A` | 删除、录音、错误 |
| `error-container` | `#FFDAD6` | 错误提示底 |
| `surface` | `#F6FAF8` | 应用底色 |
| `surface-container-lowest` | `#FFFFFF` | 卡片、输入框、画布 |
| `surface-container-low` | `#F0F5F3` | 侧栏、表头 |
| `surface-container` | `#EAF0EE` | 浮层、分段控件 |
| `surface-container-high` | `#E4EAE8` | 对话框、菜单、工具条 |
| `on-surface` | `#171D1C` | 主文字 |
| `on-surface-variant` | `#3F4948` | 次级文字 |
| `outline` | `#6F7978` | 输入框边框 |
| `outline-variant` | `#BEC9C7` | 分隔线、卡片边框 |

规则：

- 正文对比度 ≥ 4.5:1；大字号 ≥ 3:1。
- 不使用 `color-mix()`（macOS 13 WebKit 兼容），所有状态色预计算为静态值。
- 强调色面积控制在单屏 <10%；标记颜色（红/橙/黄/绿/蓝/墨）独立于品牌色。

### 2.2 字体

| 层级 | 字号/行高 | 字重 | 用途 |
| --- | --- | --- | --- |
| Display | 40/48 | 700 | 项目选择标题 |
| Headline | 28/36 | 700 | 设置页、历史详情标题 |
| Title | 22/28 | 700 | 工作区标题 |
| Title small | 16/22 | 600 | 对话框、卡片标题 |
| Body | 14/20 | 400 | 正文、输入 |
| Body small | 13/18 | 400 | 列表、意见 |
| Label | 12/16 | 500 | 按钮、chip、元信息 |
| Caption | 11/15 | 400 | 辅助说明、时间 |

字体栈：`Roboto`（拉丁，37 KB 可变字体）+ `PingFang SC`（中文，系统字体）。
不引入网络字体；不打包完整 CJK 字库（体积不可接受）。
pen.dev 渲染时用 `Noto Sans SC` 替代 `PingFang SC`（工具字体列表限制），实现仍走系统 PingFang。

### 2.3 形状 / 间距 / 高度

| Token | 值 | 用途 |
| --- | --- | --- |
| `r-xs` | 4px | 小标签、缩略图内元素 |
| `r-sm` | 8px | 输入框、菜单项、chip |
| `r-md` | 12px | 卡片、面板、列表容器 |
| `r-lg` | 16px | 导航选中、图片卡 |
| `r-xl` | 28px | 对话框、composer、欢迎卡 |
| `r-full` | 999px | 按钮、搜索框、分段控件 |

密度：桌面端 **−1**。顶栏 64、导航栏 88、列表行 52、工具栏按钮 40、卡片信息区 48。
触控目标不小于 32×32；主要按钮 40 高。

### 2.4 层级 / 动效

- Elevation 1：卡片 hover、缩略图；2：菜单/浮层；3：对话框/composer。
- 阴影只用于浮层，不给普通卡片加阴影；普通卡片用 1px `outline-variant`。
- 动效 150–250ms，`emphasized decelerate`；只做透明度/位移/缩放，不做弹跳。
- `prefers-reduced-motion` 下全部降为瞬时。

## 3. 组件清单

### 3.1 已有 15 个原语（沿用/扩展）

`ViewerButton`、`ViewerChoiceChip`、`ViewerDialog`、`ViewerEmptyState`、`ViewerField`、
`ViewerIcon`、`ViewerInspector`、`ViewerLocalFeedback`、`ViewerMenuRow`、`ViewerPopover`、
`ViewerSegmentedControl`、`ViewerStatusTag`、`ViewerTaskSurface`、`ViewerToolbar`、
`ViewerToolbarReviewSlot`。

视觉稿把这些原语映射到 M3 形状/状态，不重写行为。

### 3.2 新增/升级组件

| 组件 | 说明 |
| --- | --- |
| `M3TopAppBar` | 品牌、面包屑、搜索、通知、头像；高度 64 |
| `M3NavigationRail` | 88px，图标+文字，选中 tonal pill |
| `M3SegmentedButton` | 网格/列表、预览/编辑、语言 |
| `M3FilterChip` | 状态筛选；选中用 `secondary-container` |
| `M3Dialog` | 28px 圆角、scrim 32%、标题 24 |
| `M3Menu` | 12px 圆角、6px padding、40px 行高 |
| `M3Snackbar` | 反色 surface、单个文字动作 |
| `ReviewToolRail` | 72px；选择/标记/线条/画笔/形状/文字；长按变体 |
| `ReviewStyleBar` | 颜色、线宽、透明度、撤销/重做 |
| `ReviewComposer` | 文本 + 麦克风 + 保存；内联浮在画布底部 |
| `ReviewReadonlyBanner` | 归档原因 + 创建新评审；替代“只变灰” |
| `SpeechWaveform` | 录音态波形、计时、停止 |
| `SettingsRow` | 标题/说明/控件/状态；用于语音、隐私 |
| `HistoryTable` | 统计卡 + 轮次表格；按素材聚合，可回到某轮 |

## 4. 界面清单

所有画面为 1440×900。当前权威源文件是 pen.dev 的 `design.pen`；
本目录只保留总览 [pen/overview.png](pen/overview.png)。

| # | 界面 | 主要任务 | 关键决策 |
| --- | --- | --- | --- |
| 01 | 项目选择 | 打开文件夹/最近项目 | 本地文件夹即项目；无账号；拖拽区是唯一主 CTA |
| 02 | 工作区 · 网格 | 浏览素材、看评审状态 | 三栏（导航/文件夹/内容）；卡片显示状态和意见数 |
| 03 | 工作区 · 列表 | 扫描、排序 | 表格列固定；行高 52；状态 chip 不抢文件名 |
| 04 | 图片预览 | 看大图、进入评审 | 中性浅色画布突出图片；右侧信息/评审摘要；底部悬浮缩放 |
| 05 | Markdown 预览 | 读文档、看大纲 | 文档是普通文件，不参与评审协议；只读预览 |
| 06 | 视频预览 | 播放、跳到意见时间点 | 视频工具不扩展，只统一 UI；时间点/范围沿用现有能力 |
| 07 | 其他格式 | 解释不能预览、仍可记录整体意见 | 不假装支持；提供“默认应用打开/Finder 显示” |
| 08 | 评审工作台 · 默认 | 在画布上标注、看意见 | 工具轨 72；右侧意见 332；画布居中；状态栏显示保存 |
| 09 | 评审 · 选中/样式 | 改颜色/线宽、移动标注 | 选中态双层描边；样式条浮在画布顶部；句柄 8px |
| 10 | 评审 · 文字标注 | 画布写短结论、口述长意见 | 文字内联；麦克风在输入框内，不单独占工具栏 |
| 11 | 评审 · 录音转写 | 录音、看波形、停止转写 | 明确“本地转写、音频不上传”；停止后才插入文本 |
| 12 | 评审 · 麦克风权限 | 权限被拒时恢复 | 在 composer 内解释 + “打开系统设置”；保留手动输入 |
| 13 | 评审 · 只读归档 | 解释为什么不能改 | 顶部 banner + 禁用工具 + “创建新评审”，不只变灰 |
| 14 | 开始评审 | 新建一轮、确认评审人 | 说明保留上一轮；默认基于当前素材版本 |
| 15 | 完成评审 | 归档、生成摘要 | 统计待处理/疑问；归档后只读；可继续评审 |
| 16 | 评审历史 | 看统计、回到某轮 | 统计卡 + 轮次表格；按素材聚合；外部修改也是事件 |
| 17 | 设置 · 语音 | 查看内置模型、语言、权限 | 当前展示 Qwen3-ASR 0.6B；最终以 S0 bake-off 为准 |
| 18 | 外部修改冲突 | 选择重新加载/保留 | 说明坐标可能偏移；不自动覆盖 |
| 19 | 搜索无结果 | 清除搜索/筛选 | 解释搜索范围；不显示空白页 |
| 20 | 旧版评审迁移 | 迁移旧格式 | 说明不可逆；保留原矩形/文字；可稍后 |
| 21 | 标注工具一览 | 设计系统/工具总览 | 11 个工具（选择 + 10 种标记）；7 个常驻分组 |

## 5. 关键界面说明

### 5.1 项目选择

- 目标：第一次打开时，用户不需要理解“项目/工作区/评审”的关系。
- 主 CTA 只有一个：“打开文件夹”；“最近打开”是次级。
- 拖拽区同时是说明区，不额外放插画。
- 底部声明本地/离线，建立信任。

### 5.2 工作区

- 文件夹侧栏是“位置”，不是“筛选”；评审状态是“筛选”，放在文件树下方。
- 网格卡片的信息顺序：缩略图 → 文件名 → 状态 → 尺寸/意见数。
- 选中卡片用 1px primary 边框 + 右上角勾，不用大面积高亮。
- 列表视图服务“扫一眼”而不是“阅读”；文件名左对齐，状态用 chip，时间弱化。

### 5.3 评审工作台

- 画布是最大面积；工具轨和意见栏固定，不随画布缩放。
- 顶部只放返回、文件名、轮次/保存状态、完成评审；不放二级导航。
- 意见列表的编号与画布标记编号一致；选中列表项时画布对应标记高亮。
- 输入框浮在画布底部，避免右侧栏被键盘/长文本挤爆；关闭后回到纯画布。
- 语音按钮是输入框的一部分，不增加第七个工具；录音时输入框扩展出波形。
- 只读态：banner 必须给出“为什么”和“下一步”，工具禁用但仍可通过 tooltip 解释。

### 5.4 设置 · 语音

- 模型卡片显示：名称、体积、语言、量化、状态；不显示“AI 魔法”文案。
- 隐私用两条可验证的承诺：音频不落盘、完全离线。
- 权限行显示当前状态和“系统设置”入口；不要在每次录音时重复弹系统框。
- 视觉稿中的 **Qwen3-ASR 0.6B / 937 MB** 是 S0 首选候选；最终模型见
  [ASR 调研](../../quality/2026-10-28-asr-model-research.md)，S0 后回填。

### 5.5 失败与边界

- 外部修改：不自动 reload；给“保留当前/重新加载”两个明确选择。
- 搜索无结果：给出搜索范围和两个恢复动作。
- 旧版迁移：说明不可逆、旧版 Viewer 无法再打开，避免用户误操作。
- 麦克风拒绝：不丢草稿，保留手动输入。

## 6. M3 落地方式

- **Token-first**：用 `@material/material-color-utilities` 在构建时生成浅色 scheme，输出静态 CSS
  变量；不引入 MUI/Material Web 运行时。
- **组件自建**：现有 15 个原语保留行为，只替换视觉层；新组件按上表逐步引入。
- **状态层**：hover 8%、focus 12%、pressed 12%；选中用 `secondary-container`，不用纯色块。
- **焦点**：2px primary outline + 2px offset；键盘可达；不用 `outline: none` 无替代。
- **图标**：Material Symbols Rounded 子集（88 个，44 KB），Apache-2.0；本地 sprite，无网络请求。
- **字体**：Roboto Latin 子集 40 KB；中文用系统 PingFang；不打包 CJK。字体/图标许可见 [assets/NOTICE.md](assets/NOTICE.md)。
- **兼容**：macOS 13 WebKit；避免 `color-mix()`、`backdrop-filter`、容器查询等新特性。
- **响应式**：以 1440 为基准；窗口变窄时优先收起文件夹栏，再让意见栏变 overlay；
  不在 0.3.0 承诺手机布局。

## 7. 可访问性

- 所有图标按钮有 `aria-label`；纯图标不单独承担语义。
- 颜色不是唯一状态信号：状态 chip 带文字，选中项带勾/边框。
- 文本对比度：正文 ≥4.5:1；大标题 ≥3:1；禁用态仍可读（≥3:1）。
- 键盘：Tab 顺序为顶栏 → 导航 → 内容 → 工具轨 → 画布 → 意见栏；Esc 关闭浮层。
- 录音/转写状态用 `aria-live="polite"` 宣告，不只靠波形。
- 动效遵循 `prefers-reduced-motion`。

## 8. 验收清单

- [ ] 21 张画面在 1440×900 下无溢出、无遮挡、无重复滚动条。
- [ ] 所有主要按钮/输入/工具都有 hover、focus、disabled、selected 四态。
- [ ] 只读、空态、错误、冲突、权限拒绝都有解释和下一步。
- [ ] 颜色/字号/圆角/间距全部来自 token，不出现魔法值。
- [ ] 不依赖网络字体/图标；pen 文件在 pen.dev 离线打开正常。
- [ ] 浅色下正文/次级文字对比度达标；标记颜色在白色画布上可辨。
- [ ] 视觉稿中的模型名/体积与最终 ASR 选型一致（当前为 Qwen3-ASR 0.6B 候选，S0 后回填）。

## 9. 待确认

1. 工具轨是否接受 7 个首屏分组 + 长按变体？还是全部平铺？
2. 评审页右侧意见栏 332px 是否够用？是否需要在窄窗口变为 overlay？
3. 项目选择页是否保留“最近项目”，还是直接进入文件夹选择？
4. 设置页是否需要在 0.3.0 暴露“更换模型”，还是只显示内置模型与状态？

## 10. 文件

```text
docs/design/0.3.0/
├── README.md             # 本文
├── pen/README.md         # Pen frame 名称/id 对照
├── pen/overview.png      # 21 屏总览（Pen 导出）
├── screens.html          # 早期 23 屏静态探索（含已延后的批量方向）
├── gallery.html          # 早期 23 屏总览
├── build_screens.py      # 生成 screens.html
├── build_gallery.py      # 生成 gallery.html
├── render.mjs            # Playwright 截图
├── render-gallery.mjs    # 总览截图
├── assets/
│   ├── design.css        # token + 组件 + 画面样式
│   ├── icons.svg         # Material Symbols 子集
│   ├── Roboto.woff2      # 拉丁可变字体
│   └── NOTICE.md         # 字体/图标许可证
└── screens/*.png         # 早期 23 张 2× 视觉稿
```

重新生成：

```bash
cd docs/design/0.3.0
python3 build_screens.py
node render.mjs
```

> 截图依赖本机 Chrome 与 Playwright；仅用于设计评审，不进入产品构建。
