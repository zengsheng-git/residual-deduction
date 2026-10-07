# 残局推演 · 解说视频生产线 🎬

上传一张象棋残局截图 → 系统自动识别局面 → AI 引擎推演必胜路线 → 自动生成带解说、有分支分析的视频。

**定位**:一个全自动的象棋残局解说视频生产线,观众看完能明白"为什么这样走能赢"。

## ✨ 功能

- 📷 **残局识别**:上传对局平台/棋谱网站的截图,YOLOv8(ONNX Runtime)自动定位棋盘并识别棋子,方向自动校正,布局合法性校验。
- 🤖 **引擎推演**:集成 Pikafish 引擎,沿主线逐Walk推演,在将军/吃子/绝杀等关键节点用 MultiPV 做分支分析,解释"为什么其他走法不行"。
- 🗣 **中文解说**:着法转纵线记法(炮二平五),规则模板生成解说词(吃子/将军/绝杀/分差解读),Edge TTS 神经语音配音。
- 🎞 **视频合成**:1920×1080@30fps,棋盘走子动画 + 评分面板 + 分支卡片 + 字幕,ffmpeg 编码合成,自动生成封面。
- 📚 **视频库**:成片按时间归档,支持站内播放、删除、打开目录,每个视频附带完整剧本 JSON。

## 🏗 技术架构

### 前端
- **框架**:Vue 3 + TypeScript
- **UI 组件**:Naive UI
- **构建**:Vite
- **打包**:Tauri v2(与后端同窗口运行)

### 后端(Rust,`server/`)
- **棋盘识别**:YOLOv8 + ONNX Runtime(ort,`load-dynamic` 加载 `libs/windows-cpu/onnxruntime.dll`)
- **象棋引擎**:Pikafish(UCI 协议,MultiPV 分支分析)
- **解说配音**:Edge TTS WebSocket(24kHz mp3,按句切分合成)
- **视频渲染**:`image` + `imageproc` + `ab_glyph` 逐帧绘制,ffmpeg(`libs/ffmpeg`)管道编码
- **视频管理**:应用数据目录 `videos/<id>/`(video.mp4 / thumb.jpg / meta.json / script.json)

### 资源目录 `libs/`
| 文件 | 用途 |
| --- | --- |
| `large.onnx` | YOLO 棋子识别模型(来自参考项目 chessboard) |
| `pikafish/pikafish-windows.exe` + `pikafish.nnue` | Pikafish 引擎与 NNUE 权重 |
| `windows-cpu/onnxruntime.dll` | ONNX Runtime CPU 运行时 |
| `ffmpeg/ffmpeg.exe` | 视频编码(x264 + aac) |

## 🚀 开发

```bash
pnpm install
pnpm tauri dev      # 桌面应用(前端 + Rust 后端)
```

> **首次克隆需准备 `libs/` 运行时资源**(模型/引擎/ffmpeg, 不随仓库分发), 见 [libs/README.md](libs/README.md)。

> **网络备注**: 若 `github.com:443` 直连超时, 可用可达的 GitHub IP 固定解析后推送:
> `git config http.curloptResolve "github.com:443:140.82.112.3"`(IP 失效时换 `140.82.114.3` 等可达节点, 或走代理)。

### 测试

```bash
cd server
cargo test --lib             # 纯逻辑单测(棋规/中文着法/评分解读)
cargo test --lib -- --ignored --nocapture   # 集成测试: 引擎/识别往返/TTS/端到端出片
```

集成测试依赖 `libs/` 资源与外网(TTS),端到端测试在 `scripts/out/appdata` 下产出视频。

## 📺 视频结构

1. **开场**:局面全貌、双方阵容、引擎结论先行
2. **主线推演**:逐走子动画 + 中文着法 + 实时评分/胜率 + 解说(吃子/将军/威胁解读)
3. **分支分析**:在防守方(对方)的决策点插入假设演示——"假如不走正着, 改走 X", 并把引擎备选线给出的进攻方反制应手 Y 一并动画演示
4. **结尾**:终局回顾、制胜要点总结

## 📜 许可

基于参考项目 [chessboard](https://github.com/atopx/chessboard)(Apache-2.0)构建,仅供学习与研究使用,严禁任何商业化或非法用途。
