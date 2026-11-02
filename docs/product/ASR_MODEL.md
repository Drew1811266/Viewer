# 听写模型与隐私

> Status: 0.3.0 development
>
> 结论：Viewer 0.3.0 **不打包、不下载任何 ASR 模型**。听写使用 macOS 系统自带的
> Speech 框架，并且强制要求 on-device 识别；没有网络回退。

## 使用哪套识别

- 框架：`SFSpeechRecognizer` + `SFSpeechURLRecognitionRequest`（系统 Speech.framework）。
- 语言：简体中文（`zh-CN`）；0.3.0 不提供其它语言入口。
- 设备端约束：`SFSpeechRecognizer.supportsOnDeviceRecognition` 为 false 时，
  平台服务直接返回“不可用”；请求始终设置 `requiresOnDeviceRecognition = true`。
- 模型归属：模型由 macOS 安装并随系统更新，Viewer 不感知具体版本、不提供 SHA256，
  也不参与模型分发。因此本页没有可审计的权重文件或再分发许可证。

## 音频生命周期

1. 开始录音时在系统临时目录创建 `viewer-dictation-<uuid>.wav`。
2. 停止后读取文件并送入系统识别；无论成功、失败、取消、录音过短，都会删除该文件。
3. 音频不写入项目目录、不进入评审协议、不进入缓存、不上传。
4. 只有用户在意见框里确认并保存的文字进入评审数据。

## 权限

- 麦克风：`AVCaptureDevice.requestAccess(for: .audio)`。
- 语音识别：`SFSpeechRecognizer.requestAuthorization`。
- 两个权限分别判断；拒绝时给出“打开系统设置 / 重试 / 改用键盘”，并保留已输入草稿。
- 权限状态不写入项目数据。

## 为什么不用打包模型

0.3.0 计划阶段比较过 Whisper、Qwen3-ASR、SenseVoice、Paraformer 等方案。它们的模型体积
约 180 MB–1 GB，需要额外的下载/校验/打包/许可证流程，冷启动和内存也会挤压图片评审。
系统 Speech 在 macOS 13+ 已提供中文离线识别，适合短意见听写；代价是可用性依赖系统资产，
Viewer 无法通过自带模型兜底。该取舍记录在
[ADR 0010](../adr/0010-offline-speech-recognition.md)。

## 已知限制

- 没有系统听写资产的机器上，听写入口显示不可用；这是隐私约束，不是缺陷。
- 识别质量由系统决定；0.3.0 不做自定义热词、说话人分离或模型参数。
- 自动化测试覆盖权限/状态/超时/临时文件清理，不覆盖真实识别准确率。
