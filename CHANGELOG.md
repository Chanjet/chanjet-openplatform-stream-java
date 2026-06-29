# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-06-29

### Added
- 初始化 Java 版本 SDK，支持基础长连接、自动重连与 Token 续期。
- 提供内置 `MessageDispatcher` 进行多事件分发，集成 AES-128 自动加解密。
- 内置好生意、好业财系列业务的语义化订阅监听能力。
- 从主网关架构解耦为完全独立的标准化 Java 项目模块。
- 增加默认的 Gateway URL 处理逻辑，若未提供自动采用生产环境地址。
