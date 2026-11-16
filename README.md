# wasm_abp

简体中文 | [English](README.en.md)

为 Flutter 应用提供 Core WebAssembly 和 AstroBox-NG 插件运行能力，共用一个原生库 `libwasm_abp`

## 接入

在 `pubspec.yaml` 中添加本仓库的 Git 依赖，并将 `ref` 固定到所需提交

Flutter 构建 hook 根据 `native_release.json` 下载对应平台的预编译库，校验压缩包与库文件的 SHA-256 后随应用打包，使用预编译库无需安装 Rust 工具链

Dart 入口为 `package:wasm_abp/wasm_abp.dart`，Core Wasm 与 Component 引擎按需初始化

## 平台与产物

| 平台 | 架构 | 原生产物 |
|---|---|---|
| Android | arm64-v8a、armeabi-v7a、x86_64 | `libwasm_abp.so` |
| Linux | arm64、x86_64 | `libwasm_abp.so` |
| macOS | arm64、x86_64 | `libwasm_abp.dylib` |
| Windows | x86_64 | `wasm_abp.dll` |
| iOS 真机 | arm64 | `libwasm_abp.a` |
| iOS 模拟器 | arm64、x86_64 | `libwasm_abp.a` |

Flutter 自动打包覆盖 Android、Linux、macOS 和 Windows，iOS 产物供宿主原生静态链接

Android armeabi-v7a 和 iOS 使用 Pulley 解释执行

Component 引擎默认启用历史插件兼容补丁，允许同步导出中的异步调用暂停执行，Core Wasm 使用独立配置

每个目标平台提供 ZIP 与配套 SHA-256 文件

ZIP 包含原生库、构建清单、依赖清单、锁文件和许可证，构建清单记录源码提交、工作区状态、目标平台、ABI 版本及库文件哈希

## 本地开发

Flutter 与 Rust 版本分别由 `.fvmrc` 和 `rust-toolchain.toml` 指定

构建当前平台的原生库

```sh
python3 tool/build.py --output dist/local --archive
```

交叉编译通过 `--target` 指定 Rust target，Android 构建还需设置 `ANDROID_NDK_HOME` 或 `ANDROID_NDK_ROOT`

```sh
ANDROID_NDK_HOME=/path/to/ndk python3 tool/build.py --target aarch64-linux-android --archive
```

在消费项目的 `pubspec.yaml` 中指定本地库后，Flutter 构建与测试使用该库

```yaml
hooks:
  user_defines:
    wasm_abp:
      native_library: /absolute/path/to/library
```

运行包内检查与测试时，同样在本仓库的 `pubspec.yaml` 中配置本地库

```sh
fvm flutter pub get
fvm flutter analyze
fvm flutter test
cargo test --locked --manifest-path native/Cargo.toml --lib --test compiled_cache --test component_compatibility
```

## 许可证

本项目采用 AGPL-3.0，第三方组件的来源、署名与许可证见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)
