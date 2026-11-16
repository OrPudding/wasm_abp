import 'dart:ffi';
import 'dart:convert';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import '../ffi/bindings.dart';

class WasmAbpException implements Exception {
  WasmAbpException(this.message);
  final String message;

  @override
  String toString() => 'WasmAbpException: $message';
}

final class WasmAbpEngine {
  WasmAbpEngine._();

  static final WasmAbpEngine shared = WasmAbpEngine._();
  final CoreBindings _bindings = CoreBindings.instance;

  static bool get isLoaded => CoreBindings.isLoaded;
  Map<String, Object?> get diagnostics => _bindings.diagnostics;

  void configureCache(String? directory) => _bindings.configureCache(directory);

  Future<WasmAbpModule> compileAsync(Uint8List bytes) async {
    final handle = await Isolate.run(() => _compileHandle(bytes));
    return WasmAbpModule._(handle, _bindings);
  }

  static int _compileHandle(Uint8List bytes) => shared.compile(bytes).handle;

  String get version => _bindings.version().toDartString();

  WasmAbpModule compile(Uint8List wasmBytes) {
    final nativeBytes = calloc<Uint8>(wasmBytes.length);
    nativeBytes.asTypedList(wasmBytes.length).setAll(0, wasmBytes);
    try {
      final handle = _bindings.moduleCompile(nativeBytes, wasmBytes.length);
      if (handle < 0) {
        final err =
            _bindings.readLastError() ?? 'Unknown WASM compilation failure';
        throw WasmAbpException(err);
      }
      return WasmAbpModule._(handle, _bindings);
    } finally {
      calloc.free(nativeBytes);
    }
  }
}

final class WasmAbpModule {
  WasmAbpModule._(this._handle, this._bindings);

  final int _handle;
  final CoreBindings _bindings;
  bool _disposed = false;

  int get handle => _handle;
  bool get isDisposed => _disposed;

  WasmAbpInstance instantiate(
      {bool withWasi = false, Map<String, String> preopens = const {}}) {
    _ensureActive();
    final Pointer<Utf8> config =
        preopens.isEmpty ? nullptr : jsonEncode(preopens).toNativeUtf8();
    late final int instanceHandle;
    try {
      instanceHandle = config == nullptr
          ? _bindings.moduleInstantiate(_handle, withWasi ? 1 : 0)
          : _bindings.moduleInstantiateConfig(
              _handle, withWasi ? 1 : 0, config);
    } finally {
      if (config != nullptr) calloc.free(config);
    }
    if (instanceHandle < 0) {
      final err =
          _bindings.readLastError() ?? 'Unknown WASM instantiation failure';
      throw WasmAbpException(err);
    }
    return WasmAbpInstance._(instanceHandle, _bindings);
  }

  Future<WasmAbpInstance> instantiateAsync({
    bool withWasi = false,
    Map<String, String> preopens = const {},
  }) async {
    _ensureActive();
    final moduleHandle = _handle;
    final instanceHandle = await Isolate.run(
      () => _instantiateHandle(moduleHandle, withWasi, preopens),
    );
    return WasmAbpInstance._(instanceHandle, _bindings);
  }

  static int _instantiateHandle(
          int handle, bool withWasi, Map<String, String> preopens) =>
      WasmAbpModule._(handle, CoreBindings.instance)
          .instantiate(withWasi: withWasi, preopens: preopens)
          .handle;

  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _bindings.moduleDrop(_handle);
  }

  void _ensureActive() {
    if (_disposed) throw StateError('WasmAbpModule is already disposed');
  }
}

final class WasmAbpInstance {
  WasmAbpInstance._(this._handle, this._bindings);

  final int _handle;
  final CoreBindings _bindings;
  bool _disposed = false;

  int get handle => _handle;
  bool get isDisposed => _disposed;

  Future<List<num>> callNumbersAsync(String name, List<num> arguments) {
    _ensureActive();
    final handle = _handle;
    return Isolate.run(() => _callNumbers(handle, name, arguments));
  }

  static List<num> _callNumbers(int handle, String name, List<num> arguments) {
    final bindings = CoreBindings.instance;
    final namePtr = name.toNativeUtf8();
    final argsPtr = jsonEncode(arguments).toNativeUtf8();
    Pointer<Utf8> result = nullptr;
    try {
      result = bindings.instanceCallJson(handle, namePtr, argsPtr);
      if (result == nullptr) {
        throw WasmAbpException(bindings.readLastError() ?? 'Wasm call failed');
      }
      return (jsonDecode(result.toDartString()) as List).cast<num>();
    } finally {
      calloc.free(namePtr);
      calloc.free(argsPtr);
      if (result != nullptr) bindings.freeString(result);
    }
  }

  List<int> call(String funcName, [List<int> args = const []]) {
    _ensureActive();
    final namePtr = funcName.toNativeUtf8();
    Pointer<Int64> argsPtr = nullptr;
    if (args.isNotEmpty) {
      argsPtr = calloc<Int64>(args.length);
      final list = argsPtr.asTypedList(args.length);
      for (var i = 0; i < args.length; i++) {
        list[i] = args[i];
      }
    }

    const maxResults = 8;
    final resultsPtr = calloc<Int64>(maxResults);
    final countPtr = calloc<IntPtr>(1);

    try {
      final rc = _bindings.instanceCall(
        _handle,
        namePtr,
        argsPtr,
        args.length,
        resultsPtr,
        maxResults,
        countPtr,
      );
      if (rc != 0) {
        final err =
            _bindings.readLastError() ?? 'Function "$funcName" call failed';
        throw WasmAbpException(err);
      }
      final count = countPtr.value;
      final results = <int>[];
      final resList = resultsPtr.asTypedList(count);
      for (var i = 0; i < count; i++) {
        results.add(resList[i]);
      }
      return results;
    } finally {
      calloc.free(namePtr);
      if (argsPtr != nullptr) calloc.free(argsPtr);
      calloc.free(resultsPtr);
      calloc.free(countPtr);
    }
  }

  WasmAbpMemory getMemory([String name = 'memory']) {
    _ensureActive();
    return WasmAbpMemory._(_handle, name, _bindings);
  }

  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _bindings.instanceDrop(_handle);
  }

  void _ensureActive() {
    if (_disposed) throw StateError('WasmAbpInstance is already disposed');
  }
}

final class WasmAbpMemory {
  WasmAbpMemory._(this._instanceHandle, this._name, this._bindings);

  final int _instanceHandle;
  final String _name;
  final CoreBindings _bindings;

  Uint8List read(int offset, int length) {
    final namePtr = _name.toNativeUtf8();
    final buf = calloc<Uint8>(length);
    try {
      final rc =
          _bindings.readMemory(_instanceHandle, namePtr, offset, buf, length);
      if (rc != 0) {
        final err = _bindings.readLastError() ?? 'Memory read failed';
        throw WasmAbpException(err);
      }
      return Uint8List.fromList(buf.asTypedList(length));
    } finally {
      calloc.free(namePtr);
      calloc.free(buf);
    }
  }

  void write(int offset, Uint8List data) {
    final namePtr = _name.toNativeUtf8();
    final buf = calloc<Uint8>(data.length);
    buf.asTypedList(data.length).setAll(0, data);
    try {
      final rc = _bindings.writeMemory(
          _instanceHandle, namePtr, offset, buf, data.length);
      if (rc != 0) {
        final err = _bindings.readLastError() ?? 'Memory write failed';
        throw WasmAbpException(err);
      }
    } finally {
      calloc.free(namePtr);
      calloc.free(buf);
    }
  }
}
