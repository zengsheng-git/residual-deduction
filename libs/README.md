# 运行时资源目录(不入库)

本目录存放体积较大的运行时二进制,因超过 GitHub 单文件限制(ffmpeg.exe > 100MB)与授权原因**不纳入版本管理**。克隆后需手动准备以下文件,应用才能运行:

```
libs/
├── large.onnx                        # YOLO 棋子识别模型 (~28MB)
├── pikafish/
│   ├── pikafish-windows.exe          # Pikafish 引擎 (Linux/macOS 版见参考项目)
│   └── pikafish.nnue                 # NNUE 权重 (~45MB)
├── windows-cpu/
│   ├── onnxruntime.dll               # ONNX Runtime CPU 运行时
│   └── onnxruntime_providers_shared.dll
└── ffmpeg/
    └── ffmpeg.exe                    # 视频编码 (x264 + aac)
```

## 获取方式

1. **large.onnx / pikafish / onnxruntime.dll**:从参考项目 [atopx/chessboard](https://github.com/atopx/chessboard) 的 `libs/` 目录复制(或其 Release 安装包内提取)。
2. **ffmpeg.exe**:从 [gyan.dev](https://www.gyan.dev/ffmpeg/builds/) 下载 `ffmpeg-release-essentials.zip`,解压取出 `bin/ffmpeg.exe` 放到 `libs/ffmpeg/`。

> 若缺少 ffmpeg,应用启动时会明确报错提示;缺少模型时识别功能不可用。
