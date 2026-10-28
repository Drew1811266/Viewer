# 离线语音模型调研（Viewer 0.3.0）

> Status: Research draft（2026-10-28）
>
> 目标：为 Viewer 选择一个可随 macOS 应用分发、完全离线、中文优先、Rust/Tauri 可集成的
> 语音转文字模型。本文只做选型与验证方案，不锁定最终模型；最终结论以 S0 真机 bake-off 为准。
>
> 相关：[0.3.x 开发方案](../milestones/viewer-0.3x-development-plan.md) ·
> [0.3.x 目标需求](../milestones/viewer-0.3x-target-requirements.md)

## 1. 约束

| 约束 | 要求 |
| --- | --- |
| 平台 | macOS 13+，Apple Silicon 为主要目标；不排除 Intel 但只做降级可用 |
| 集成 | Rust/Tauri，避免引入 Python 运行时；模型文件随 `.app` 分发 |
| 语言 | 中文普通话优先，英文可用；最好支持中英混合与自动语言识别 |
| 隐私 | 音频不落盘、不上传；模型本地推理 |
| 许可证 | 模型权重与推理代码允许再分发；不能只支持云端 API |
| 体积 | 默认内置模型尽量 ≤600 MB；安装包可接受上限需产品确认 |
| 延迟 | 10 秒语音 p95 ≤3 秒；不阻塞 UI |
| 体验 | 按住/点击说话，停止后插入整段文字；不要求实时逐字上屏 |

## 2. 候选模型

### 2.1 总表

| 模型 | 参数/体积（量化） | 语言 | 权重许可证 | 运行时 | 结论 |
| --- | --- | --- | --- | --- | --- |
| Whisper small | 244M / 181 MB q5_1 | 99 语言，中文可用 | Apache-2.0 | whisper.cpp / whisper-rs | 体积基线，成熟 |
| Whisper large-v3-turbo | 809M / 547 MB q5_0 | 99 语言，中文明显更好 | MIT | whisper.cpp / whisper-rs | 高精度首选候选 |
| Qwen3-ASR-0.6B | 0.6B / 约 937 MB int8（sherpa 三文件合计） | 30 语言 + 22 种中文方言 | Apache-2.0 | sherpa-onnx（Rust API） | **中文/方言首选候选** |
| Qwen3-ASR-1.7B | 1.7B / 约 2.4 GB int8 | 同上 | Apache-2.0 | sherpa-onnx / vLLM | 精度更高，体积过大 |
| SenseVoiceSmall | 234M / 226 MB int8 | 中、粤、英、日、韩 | FunASR Model License（自定义，非 OSI） | sherpa-onnx / FunASR | 体积/速度好，许可证需确认 |
| Paraformer-zh | 220M / 约 230 MB int8 | 中文、英文 | Apache-2.0 | sherpa-onnx / FunASR | 轻量中文备选 |
| FireRedASR2-AED | 约 1.1B / 原始 4.5 GB，量化后约 1.5 GB | 中、英 | Apache-2.0 | sherpa-onnx | 精度强但过重 |
| Parakeet TDT 0.6B v3 | 0.6B / 约 600 MB | 25 种欧洲语言 | CC-BY-4.0 | sherpa-onnx / MLX | 不支持中文，排除 |
| Moonshine | tiny/base | 英为主，部分中文变体 | 混合（部分 other） | sherpa-onnx / ONNX | 中文生态弱，排除 |
| Apple Speech | 系统 | 多语 | 闭源 | Speech framework | 不满足“开源模型”，排除 |

> 体积说明：Whisper 数字来自 `ggerganov/whisper.cpp` 的 GGML 文件；
> Qwen3-ASR 来自 `csukuangfj2/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25`
> 的 `conv_frontend + encoder.int8 + decoder.int8` 合计；SenseVoice 来自 sherpa-onnx
> int8 导出。不同量化/导出方式会有差异。

### 2.2 Whisper small / large-v3-turbo

- **优点**：生态最成熟；whisper.cpp 纯 C/C++、无 Python；whisper-rs 提供 Rust 绑定；
  Metal/Core ML 加速路径明确；模型卡与社区语料多；turbo 比 large 快很多。
- **缺点**：Whisper 系列中文标点与专有名词一般；30 秒窗口需要自己分段；small 对中文短句
  容易漏字；turbo 体积 547 MB，内存占用高于 small。
- **许可证**：whisper.cpp MIT；Whisper 权重 MIT；`whisper-rs` 为 Unlicense（宽松但不在当前
  `deny.toml` 允许列表，需要登记例外或改用 sherpa-onnx）。
- **集成成本**：低。`whisper-rs` API 直接吃 `&[f32]` 16 kHz mono，适合现有
  `AudioCapture → Resampler → Recognizer` 设计。

### 2.3 Qwen3-ASR-0.6B

- **优点**：Apache-2.0；官方模型卡称支持 30 种语言 + 22 种中文方言，包含粤语、四川话、
  吴语、闽南语等；支持流式/离线统一推理；模型卡声称在开源 ASR 中达到 SOTA 水平；
  sherpa-onnx 已提供 Qwen3-ASR 的 Rust 示例与 int8 导出。
- **缺点**：0.6B 是 decoder-only 结构，CPU int8 的实时率需要实测；sherpa 导出文件约 937 MB；
  sherpa-onnx 的静态构建体积/编译时间明显大于 whisper-rs；生态较新，社区案例少。
- **许可证**：权重 Apache-2.0；sherpa-onnx Apache-2.0；`sherpa-onnx` Rust crate Apache-2.0。
  这是目前**许可证最干净、中文覆盖最好**的组合。
- **集成成本**：中。需要接 sherpa-onnx 的 `OfflineRecognizer`（Qwen3-ASR 配置）并处理
  `conv_frontend + encoder + decoder + tokenizer` 四个文件；但不需要自己实现模型。

### 2.4 SenseVoiceSmall

- **优点**：226 MB int8，非自回归，速度快；中文、粤语、英文、日文、韩文；带情感/事件标签
  （Viewer 不需要，可忽略）；sherpa-onnx 支持成熟。
- **缺点**：权重使用 FunASR 自定义 Model License，不是 OSI 许可证；允许使用、复制、修改、
  分享，但要求署名/保留模型名，并含禁止贬损条款。需要法律/产品确认能否随闭源应用分发。
- **结论**：作为“体积/速度优先”的备选；如果许可证可接受，它可能比 Whisper small 更适合
  短中文口述。不要在未确认前把它写进发布物。

### 2.5 Paraformer / FireRedASR

- **Paraformer-zh**：Apache-2.0，中文/英文，sherpa-onnx 支持；比 SenseVoice 老，标点和
  口语鲁棒性一般，但体积小、速度快，可作为低配回退。
- **FireRedASR2-AED**：Apache-2.0，中文/英文，论文指标强；原始权重 4.5 GB，量化后仍约
  1.5 GB，超出 0.3.0 的合理安装包预算；不优先。

### 2.6 明确排除

- **Parakeet / Canary**：主打英语/欧洲语言，中文不支持或不稳定。
- **Moonshine**：英文优先，中文变体许可证不统一，生态不足。
- **Apple Speech**：闭源，不满足“内置开源模型”。
- **云端 API（OpenAI/阿里/讯飞）**：不满足离线与隐私要求。

## 3. 运行时对比

| 运行时 | 许可证 | 支持模型 | Rust 集成 | 优点 | 风险 |
| --- | --- | --- | --- | --- | --- |
| whisper-rs 0.16 | Unlicense | Whisper | 直接 | API 简单、Metal/Core ML | 许可证例外；不支持 Qwen/SenseVoice |
| sherpa-onnx 2.0.1 | Apache-2.0 | Whisper、Qwen3-ASR、SenseVoice、Paraformer、FireRed、Parakeet | 官方 Rust crate | 一套运行时覆盖多个候选；Apache | 静态库体积/编译时间；Qwen3 延迟待测 |
| FunASR Python | MIT（代码） | SenseVoice/Paraformer | 需 sidecar | 功能全 | 引入 Python 运行时，不符合“单一 `.app`”目标 |
| MLX / Core ML 自建 | 各模型不同 | 需转换 | Swift/ObjC | Apple Silicon 快 | 工程量大，模型转换链路复杂 |

**建议**：以 **sherpa-onnx** 作为统一运行时做 bake-off，这样同一套 C/Rust 集成可以比较
Qwen3-ASR、SenseVoice、Whisper、Paraformer；若 sherpa 的体积/延迟不可接受，再退回
whisper-rs + Whisper turbo。

## 4. 推荐路线

### 4.1 首选组合（待实测确认）

**Qwen3-ASR-0.6B int8 + sherpa-onnx**

理由：

1. Apache-2.0 权重 + Apache-2.0 运行时，发布阻力最小。
2. 中文方言覆盖最好，符合“真实复杂情况”和中文口述评审。
3. 同一运行时可以无缝回退到 SenseVoice/Paraformer/Whisper，不被单一模型锁死。
4. 0.6B 比 1.7B 更适合桌面应用；如果 0.6B 延迟不达标，再评估 1.7B 或 Whisper turbo。

风险：937 MB 安装包增量（比 600 MB 软约束高约 337 MB）、CPU 实时率、sherpa 静态构建。
S0 必须先测这三项；如果体积不可接受，直接落到回退顺序。

### 4.2 回退顺序

```text
Qwen3-ASR-0.6B（Apache，中文最好）
  ├─ 延迟不达标 → Whisper large-v3-turbo（547 MB，成熟、快）
  ├─ 体积不允许 → Whisper small（181 MB）或 Paraformer（约 230 MB）
  └─ 许可证可接受且追求速度 → SenseVoiceSmall（226 MB）
```

### 4.3 不推荐“先 small 再换 turbo”作为发布策略

如果 0.3.0 已经内置 small，后续换 turbo 会改变安装包、模型路径、验收指标和用户预期。
建议 S0 一次 bake-off 定稿，0.3.0 只内置一个默认模型；高精度模型作为 0.3.x 可选包。

## 5. S0 Bake-off 方案

### 5.1 参赛

| 编号 | 模型 | 运行时 | 目的 |
| --- | --- | --- | --- |
| A | Qwen3-ASR-0.6B int8 | sherpa-onnx | 中文/方言首选 |
| B | Whisper large-v3-turbo q5_0 | whisper-rs 或 sherpa-onnx | 成熟高精度基线 |
| C | Whisper small q5_1 | whisper-rs 或 sherpa-onnx | 体积下限 |
| D | SenseVoiceSmall int8 | sherpa-onnx | 速度/体积备选（许可证待确认） |

### 5.2 语料

- 50 条中文评审短句（5–20 秒），覆盖：间距、对齐、层级、颜色、对比度、交互状态、错别字、
  英文术语（CTA、Hover、Design Token）。
- 20 条英文短句。
- 10 条中英混合。
- 10 条噪声/远场/低音量。
- 语料不提交仓库；只记录统计结果、日期、机器型号、模型 hash。需要用户授权录制。

### 5.3 指标

| 指标 | 目标 |
| --- | --- |
| 中文 CER | ≤12%（首选）；≤15% 可接受 |
| 英文 WER | ≤18% |
| 10s p50 / p95 | ≤1.5s / ≤3s |
| 30s p50 / p95 | ≤4s / ≤8s |
| 冷启动模型加载 | ≤2s（SSD，缓存后） |
| 峰值 RSS | ≤1.5 GB（0.6B） |
| 安装包增量 | 记录，供 D1 决策 |
| 取消/超时 | 100% 可取消，无 UI 卡死 |
| 断网 | 全流程可用，无外发请求 |

### 5.4 通过条件

- 准确率达标且 p95 延迟达标：选该模型。
- 准确率达标但延迟不达标：先优化线程/量化/分段；仍不达标则换 Whisper turbo。
- 准确率不达标：换 turbo；若 turbo 仍不达标，考虑 Qwen3-ASR-1.7B 或接受“语音仅辅助，
  必须人工校对”的产品定位。

## 6. 打包与许可证

- 模型不提交 Git；`scripts/asr/stage-model.sh` 下载固定版本、校验 SHA256、复制到
  `target/viewer-asr-model/`；Tauri `bundle.resources` 映射进 `.app`。
- 构建时校验模型存在与 hash；缺失时失败并输出修复命令。
- `docs/product/ASR_MODEL.md` 记录：模型名、版本、来源 URL、SHA256、许可证、体积、
  量化方式、导出工具版本。
- `THIRD_PARTY_NOTICES.md` 增加模型与运行时；`deny.toml` 处理 whisper-rs 的 Unlicense
  （或改用 sherpa-onnx 规避）。
- 不做运行时下载；0.3.0 默认模型随包分发，保证断网可用。

## 7. 待确认

1. 安装包增加约 1 GB 是否可接受？如果不可接受，优先 Whisper small/Paraformer。
2. 是否接受 FunASR Model License（SenseVoice）？如果不接受，排除 SenseVoice。
3. 是否要求粤语/方言？如果只要求普通话+英文，Whisper turbo 更稳。
4. 是否接受 S0 用本地真实语料录制（不提交仓库）？

## 8. 来源

- Qwen3-ASR-0.6B 模型卡：<https://huggingface.co/Qwen/Qwen3-ASR-0.6B>
- Qwen3-ASR-1.7B 模型卡：<https://huggingface.co/Qwen/Qwen3-ASR-1.7B>
- sherpa-onnx Qwen3-ASR 文档：<https://k2-fsa.github.io/sherpa/onnx/qwen3-asr/pretrained.html>
- sherpa-onnx 仓库：<https://github.com/k2-fsa/sherpa-onnx>
- sherpa-onnx Rust crate：<https://crates.io/crates/sherpa-onnx>
- Whisper large-v3-turbo：<https://huggingface.co/openai/whisper-large-v3-turbo>
- whisper.cpp：<https://github.com/ggml-org/whisper.cpp>
- whisper-rs：<https://crates.io/crates/whisper-rs>
- SenseVoiceSmall：<https://huggingface.co/FunAudioLLM/SenseVoiceSmall>
- FunASR Model License：<https://github.com/modelscope/FunASR/blob/main/MODEL_LICENSE>
- Paraformer-zh：<https://huggingface.co/funasr/paraformer-zh>
- FireRedASR2-AED：<https://huggingface.co/FireRedTeam/FireRedASR2-AED>
- Parakeet TDT 0.6B v3：<https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3>
