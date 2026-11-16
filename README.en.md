# wasm_abp

[简体中文](README.md) | English

Core WebAssembly and AstroBox-NG plugin execution for Flutter applications, provided by one native library, `libwasm_abp`

## Integration

Add this repository as a Git dependency in `pubspec.yaml` and pin `ref` to the required commit

The Flutter build hook downloads the platform library specified in `native_release.json`, verifies the archive and library SHA-256 hashes, and bundles the library with the application. Using prebuilt libraries requires no Rust toolchain

Import `package:wasm_abp/wasm_abp.dart`. Core Wasm and Component engines initialize on demand

## Platforms and artifacts

| Platform | Architectures | Native library |
|---|---|---|
| Android | arm64-v8a, armeabi-v7a, x86_64 | `libwasm_abp.so` |
| Linux | arm64, x86_64 | `libwasm_abp.so` |
| macOS | arm64, x86_64 | `libwasm_abp.dylib` |
| Windows | x86_64 | `wasm_abp.dll` |
| iOS devices | arm64 | `libwasm_abp.a` |
| iOS Simulator | arm64, x86_64 | `libwasm_abp.a` |

Flutter bundles libraries automatically on Android, Linux, macOS, and Windows. The iOS artifacts are static libraries for native linking by the host application

Android armeabi-v7a and iOS use the Pulley interpreter

The Component engine enables a compatibility patch that allows asynchronous suspension during synchronous exports in historical plugins. Core Wasm has a separate configuration

Each target ZIP includes the native library, build manifest, dependency inventory, lockfile, and licenses, with an adjacent SHA-256 file. The manifest records the source commit, workspace state, target, ABI version, and library hash

## Local development

Flutter and Rust versions are specified in `.fvmrc` and `rust-toolchain.toml`

Build the native library for the current platform

~~~sh
python3 tool/build.py --output dist/local --archive
~~~

Use `--target` for cross-compilation. Android builds also require `ANDROID_NDK_HOME` or `ANDROID_NDK_ROOT`

~~~sh
ANDROID_NDK_HOME=/path/to/ndk python3 tool/build.py --target aarch64-linux-android --archive
~~~

Select a local library in the consuming application’s `pubspec.yaml`

~~~yaml
hooks:
  user_defines:
    wasm_abp:
      native_library: /absolute/path/to/library
~~~

Apply the same configuration to this package’s `pubspec.yaml` when running its checks and tests

~~~sh
fvm flutter pub get
fvm flutter analyze
fvm flutter test
cargo test --locked --manifest-path native/Cargo.toml --lib --test compiled_cache --test component_compatibility
~~~

## License

AGPL-3.0. Third-party sources, attribution, and licenses are listed in [THIRD_PARTY_NOTICES.en.md](THIRD_PARTY_NOTICES.en.md)
