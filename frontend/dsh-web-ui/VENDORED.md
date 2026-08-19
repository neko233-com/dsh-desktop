# Vendored DSH Web UI

这是 DSH Desktop 使用的前端源码副本，来源为 `zhu1090093659/dsh-web-ui` 的公开源码快照。

- 源码在本仓库内维护，不是 Git submodule。
- 桌面运行时只链接本目录构建产物，不从参考仓库导入前端，也不启用远程自更新。
- 官方 DeepSeek Harness 仍作为执行时运行；它与本前端源码是两个独立层次。
- 保留本目录原有许可证和各子包许可证。

构建：

```sh
pnpm install --config.minimumReleaseAge=0
pnpm build
```
