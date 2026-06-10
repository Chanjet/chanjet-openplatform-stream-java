# cowen-wasm-auth-selfbuilt Design Notes

本文档用于记录该模块在开发过程中的架构设计考量、选型决策记录 (ADR) 以及任何特殊的历史包袱说明。

## 设计决策 (Architecture Decisions)
- **沙箱无系统权限**：由于 Wasm 处于被严格管控的 Wasmtime 虚拟机中，它的鉴权算法不能直接读取宿主机的环境变量和文件，所有的信息必须由宿主主动分配（或者通过 WASI 暴露挂载点）。

## 时序流或关系图
*(暂无时序流图表)*
