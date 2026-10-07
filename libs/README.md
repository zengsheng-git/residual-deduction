# 运行时资源目录

本目录存放运行时二进制。模型与引擎文件已随仓库分发, 仅 `ffmpeg.exe`(超过 GitHub 单文件 100MB 限制)不入库, 克隆后需手动准备:

```
libs/
├── large.onnx                        # YOLO 棋子识别模型 (~28MB)
├── pikafish/
│   ├── pikafish-windows.exe          # Pikafish 引擎
│   └── pikafish.nnue                 # NNUE 权重 (~45MB)
├── windows-cpu/
│   ├── onnxruntime.dll               # ONNX Runtime CPU 运行时
│   └── onnxruntime_providers_shared.dll
└── ffmpeg/
    └── ffmpeg.exe                    # 视频编码 (x264 + aac) — 不入库
```

## ffmpeg 获取方式

从 [gyan.dev](https://www.gyan.dev/ffmpeg/builds/) 下载 `ffmpeg-release-essentials.zip`, 解压取出 `bin/ffmpeg.exe` 放到 `libs/ffmpeg/`。

> 若缺少 ffmpeg, 应用启动时会明确报错提示。
