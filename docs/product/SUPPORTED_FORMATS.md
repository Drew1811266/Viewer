# Viewer 0.1.6 支持格式与限制

> Status: Current
>
> 当前开发版本：`0.1.6`

> **重要规则：被扫描识别不代表文件内容一定可解码；实际结果取决于文件内容、权限、完整性和
> 打包运行时。**

Viewer 先按不区分大小写的扩展名把普通文件归类，再由图片、文本或视频管线读取实际内容。
因此本页把“扫描候选”“应用内能力”和“失败表现”分开说明。

## 1. 图片

| 扩展名或类别 | 扫描分类 | 0.1.6 应用内能力 | 失败与限制 |
| --- | --- | --- | --- |
| `.jpg`、`.jpeg` | JPEG | 缩略图、适合窗口预览、原图 100%、缩放/平移/旋转、放大镜、2–8 图对比 | 损坏、不可读或内容与扩展名不符时显示局部图片错误 |
| `.png` | PNG | 与 JPEG 相同，包含带透明度 PNG 的表示管线 | 同上；成功识别不保证每个异常 PNG 都可解码 |
| `.gif`、`.webp`、`.avif`、`.heic`、`.heif`、`.bmp`、`.tif`、`.tiff`、`.svg`、`.svgz`、`.ico`、`.jxl`、`.jfif`、`.apng`、`.psd`、`.dng`、`.cr2`、`.cr3`、`.nef`、`.nrw`、`.arw`、`.srf`、`.sr2`、`.raf`、`.orf`、`.rw2`、`.pef`、`.srw`、`.x3f` | `unsupported_image` | 作为图片类条目显示并可审阅、收藏、搜索或整理 | 不承诺应用内解码或原图预览；进入预览/对比时可能只显示明确的不支持状态 |

图片表示通过会话限定的 `viewer-image://` URL 交给界面，不直接把任意文件路径暴露给 Web
层。当前图片主支持集是 JPEG 和 PNG，不应把 macOS Quick Look 偶然能显示的其他格式写成
Viewer 的稳定产品承诺。

维护证据：`crates/viewer-infrastructure/src/scan/file_classifier.rs`、
`crates/viewer-domain/src/file.rs`、`crates/viewer-platform-macos/src/image/`、
`ui/src/components/ImagePreview.test.tsx`。

## 2. 文本

| 扩展名 | 扫描分类 | 预览 | 内容处理 |
| --- | --- | --- | --- |
| `.md`、`.markdown` | Markdown | 只读 Markdown 预览，最多同时两栏 | 支持表格和删除线；删除原始 HTML；仅解析为受控本地表示的图片可显示 |
| `.txt` | Text | 只读纯文本预览，最多同时两栏 | 规范化 CRLF/CR 换行；不编辑、不写回 |

支持的手动编码选择为：

- UTF-8；
- UTF-16 LE；
- UTF-16 BE；
- GB18030。

读取器会先识别 UTF-8/UTF-16 BOM，然后尝试 UTF-8，最后尝试 GB18030。无法无损解码时
要求用户显式选择编码。每个文本文件最多读取前 `10 × 1024 × 1024` 字节，即 10 MiB；
更大的文件仍可预览前 10 MiB，并明确标记为截断。

Markdown 原始 HTML 被丢弃并再次净化；未解析图片和远程图片不会自动加载。用户点击合法的
`http`/`https` 链接后，Viewer 才把该链接交给系统浏览器；`file:`、`javascript:`、包含
用户名/密码或超过边界的 URL 会被拒绝。

维护证据：`crates/viewer-application/src/text.rs`、
`crates/viewer-infrastructure/src/text/preview.rs`、`src-tauri/src/markdown.rs`、
`ui/src/components/TextPreview.test.tsx`。

## 3. 视频

### 3.1 扫描候选扩展名

以下扩展名按不区分大小写的规则归类为视频候选：

| 扩展名 | 扩展名 | 扩展名 | 扩展名 | 扩展名 |
| --- | --- | --- | --- | --- |
| `.mp4` | `.m4v` | `.mov` | `.mkv` | `.webm` |
| `.avi` | `.wmv` | `.mpg` | `.mpeg` | `.ts` |
| `.mts` | `.m2ts` | `.flv` | `.ogv` | `.3gp` |

这是一份容器候选列表，不是编解码器兼容矩阵。文件必须通过本地探测和实际解码后才能播放。
0.1.6 不声称某个扩展名下的所有视频/音频编码组合都可用。

### 3.2 当前播放与派生能力

| 能力 | 当前行为 | 边界 |
| --- | --- | --- |
| 元数据探测 | 读取时长、显示尺寸、旋转、帧率和视频/音频编解码器名称 | 探测失败按不支持、损坏、不可读、缺失或引擎初始化失败分类 |
| 浏览封面 | 从早期帧中选择首个有用画面并生成 PNG 封面 | 找不到可解码帧时封面不可用，但仍允许用户尝试播放 |
| 时间轴缩略图 | 指针停留后按半秒桶请求 PNG 预览；过时请求被取消或丢弃 | 播放活跃时派生任务让路，缩略图失败不停止主播放 |
| 主播放 | 本地原生视频表面；播放/暂停、定位、逐帧、音量、静音、速度和全屏 | 实际结果取决于内容、权限、窗口表面和已准备的媒体运行时 |
| 播放结束 | 保留结束状态；再次播放从头开始 | 不提供播放列表连续播放 |

### 3.3 媒体运行时基线

开发仓库锁定的本地运行时为：

- mpv `v0.41.0`（提交短号 `41f6a64`）；
- FFmpeg `n8.0` / `8.0`；
- 目标 `universal-apple-darwin`；
- FFmpeg 构建显式使用 `--disable-network`，且不包含 FFplay、设备/avdevice、GPL 或非自由组件；
- 运行时依赖和下载源以 `scripts/video/runtime.lock.json` 的哈希锁定内容为准。

视频失败类型包括：不支持、损坏、不可读、缺失、引擎初始化失败、解码回退失败、渲染表面
失败和缩略图不可用。前七类按当前视频局部处理；缩略图不可用不等于主视频不可播放。

维护证据：`crates/viewer-infrastructure/src/scan/file_classifier.rs`、
`crates/viewer-infrastructure/src/video_probe.rs`、`crates/viewer-domain/src/video.rs`、
`scripts/video/runtime.lock.json`、`ui/src/components/VideoPreview.test.tsx`、
`crates/viewer-infrastructure/tests/video_thumbnail.rs`。

## 4. 其他文件与目录

| 类别 | 扫描/显示 | 应用内预览 |
| --- | --- | --- |
| 没有列入上述规则的普通文件 | 归类为 `other`，显示名称、路径和可取得的文件元数据，可参与选择、标记、搜索和文件整理 | 显示“不支持预览”的明确状态，不尝试按未知内容执行 |
| 目录 | 保留为项目树和内容文件夹投影，可作为复制/移动的项目内目标 | 不作为文件预览 |
| 点开头条目、`Thumbs.db`、`Desktop.ini` | 扫描时忽略 | 不显示 |
| `.viewer` | 因点开头规则不显示为项目内容；由 Viewer 用于项目身份和标记元数据 | 不作为用户素材预览 |

## 5. 明确不支持的产品行为

Viewer 0.1.6 不提供：

- 图片、Markdown、TXT 或视频内容编辑与保存；
- 网络视频、直播地址、在线播放或远程媒体流；
- 播放列表、媒体库、连续自动播放；
- 字幕选择、外部字幕加载、音轨选择或复杂音频路由；
- 云盘连接、云素材源、账户同步或在线协作；
- 用未知文件扩展名自动猜测并执行内容；
- 把某个容器扩展名当作全部编解码器组合的兼容承诺。

遇到格式相关问题时，参阅[故障排除](TROUBLESHOOTING.md)；涉及缓存与网络边界时参阅
[数据、隐私与安全](DATA_PRIVACY.md)。
