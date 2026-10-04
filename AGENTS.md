# AGENTS.md

## 提交规则

- **未经用户明确允许，不得执行 `git commit` 或 `git push`**。
- 修改完成后，先说明改动内容和预览，等用户确认后再提交。

## 工程约定

- `libs/` 为运行时资源目录（模型/引擎/ffmpeg），由打包配置映射，请勿改名。
- Rust 端资源均以 `include_bytes!` 相对路径编译或按 `../libs` 资源目录解析，移动文件需同步修改。
- 前端与 Rust 交互统一走 `src/api.ts` 封装的 invoke 命令与 `gen://*` 事件。
