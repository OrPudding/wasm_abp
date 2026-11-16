# 第三方声明

简体中文 | [English](THIRD_PARTY_NOTICES.en.md)

## AstroBox-NG PluginSystem

AstroBox 宿主适配中的资源所有权处理改编自 AstroBox-NG PluginSystem，作者为 AstralSightStudios 与项目贡献者

上游采用 AGPL-3.0，并附加署名要求，具体条款见上游仓库

来源 [AstralSightStudios/AstroBox-NG-Module-PluginSystem](https://github.com/AstralSightStudios/AstroBox-NG-Module-PluginSystem)

## AstroBox-Plugin-WIT

插件接口定义来自 AstroBox-Plugin-WIT，采用 MIT 许可证

Copyright (c) 2025 Searchstars

许可证原文保留在 `wit/LICENSE` 和 `native/wit/LICENSE`

来源 [AstralSightStudios/AstroBox-Plugin-WIT](https://github.com/AstralSightStudios/AstroBox-Plugin-WIT)

## Wasmtime

Wasmtime 采用 Apache-2.0 WITH LLVM-exception 许可证，本仓库对其 Component 调用行为进行了兼容性修改

许可证原文保留在 `native/vendor/wasmtime/LICENSE`，修改内容见 `native/patches/wasmtime-50-legacy-sync.patch`

来源 [bytecodealliance/wasmtime](https://github.com/bytecodealliance/wasmtime)

## 其他依赖

Rust 依赖及版本记录在 `native/Cargo.lock`，原生发布包附带依赖清单与许可证文件
