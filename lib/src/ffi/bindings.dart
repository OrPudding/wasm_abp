import 'dart:ffi';
import 'dart:convert';
import 'dart:io';

import 'package:ffi/ffi.dart';

typedef WasmAbpInitC = Int32 Function();
typedef WasmAbpInitDart = int Function();

typedef WasmAbpVersionC = Pointer<Utf8> Function();
typedef WasmAbpVersionDart = Pointer<Utf8> Function();

typedef WasmAbpGetLastErrorC = Pointer<Utf8> Function();
typedef WasmAbpGetLastErrorDart = Pointer<Utf8> Function();

typedef WasmAbpFreeStringC = Void Function(Pointer<Utf8> ptr);
typedef WasmAbpFreeStringDart = void Function(Pointer<Utf8> ptr);

typedef WasmAbpFreeBytesC = Void Function(Pointer<Uint8> ptr, IntPtr len);
typedef WasmAbpFreeBytesDart = void Function(Pointer<Uint8> ptr, int len);

typedef ComponentPrecompileC = Int32 Function(
  Pointer<Uint8> bytes,
  IntPtr len,
  Pointer<Pointer<Uint8>> outPtr,
  Pointer<IntPtr> outLen,
);
typedef ComponentPrecompileDart = int Function(
  Pointer<Uint8> bytes,
  int len,
  Pointer<Pointer<Uint8>> outPtr,
  Pointer<IntPtr> outLen,
);

typedef ComponentDeserializeC = Int64 Function(
    Pointer<Uint8> bytes, IntPtr len);
typedef ComponentDeserializeDart = int Function(Pointer<Uint8> bytes, int len);

typedef ComponentCompileForApiC = Int64 Function(
    Pointer<Uint8> bytes, IntPtr len, Uint32 apiLevel);
typedef ComponentCompileForApiDart = int Function(
    Pointer<Uint8> bytes, int len, int apiLevel);

typedef ComponentInstantiateC = Int64 Function(
    Int64 componentHandle, Pointer<Utf8> preopensJson);
typedef ComponentInstantiateDart = int Function(
    int componentHandle, Pointer<Utf8> preopensJson);

typedef ModuleCompileC = Int64 Function(Pointer<Uint8> bytes, IntPtr len);
typedef ModuleCompileDart = int Function(Pointer<Uint8> bytes, int len);

typedef ModuleInstantiateC = Int64 Function(Int64 moduleHandle, Int32 withWasi);
typedef ModuleInstantiateDart = int Function(int moduleHandle, int withWasi);

typedef InstanceCallC = Int32 Function(
  Int64 instanceHandle,
  Pointer<Utf8> funcName,
  Pointer<Int64> args,
  IntPtr argsLen,
  Pointer<Int64> results,
  IntPtr maxResults,
  Pointer<IntPtr> outCount,
);
typedef InstanceCallDart = int Function(
  int instanceHandle,
  Pointer<Utf8> funcName,
  Pointer<Int64> args,
  int argsLen,
  Pointer<Int64> results,
  int maxResults,
  Pointer<IntPtr> outCount,
);

typedef ReadMemoryC = Int32 Function(
  Int64 instanceHandle,
  Pointer<Utf8> memName,
  IntPtr offset,
  Pointer<Uint8> outBuf,
  IntPtr len,
);
typedef ReadMemoryDart = int Function(
  int instanceHandle,
  Pointer<Utf8> memName,
  int offset,
  Pointer<Uint8> outBuf,
  int len,
);

typedef WriteMemoryC = Int32 Function(
  Int64 instanceHandle,
  Pointer<Utf8> memName,
  IntPtr offset,
  Pointer<Uint8> inBuf,
  IntPtr len,
);
typedef WriteMemoryDart = int Function(
  int instanceHandle,
  Pointer<Utf8> memName,
  int offset,
  Pointer<Uint8> inBuf,
  int len,
);

typedef DropHandleC = Int32 Function(Int64 handle);
typedef DropHandleDart = int Function(int handle);

typedef ComponentPollC = Pointer<Utf8> Function(Int64 instanceHandle);
typedef ComponentPollDart = Pointer<Utf8> Function(int instanceHandle);

typedef ComponentMessageC = Int32 Function(Int64 handle, Pointer<Utf8> json);
typedef ComponentMessageDart = int Function(int handle, Pointer<Utf8> json);

typedef ConfigureCacheC = Int32 Function(Pointer<Utf8> path);
typedef ConfigureCacheDart = int Function(Pointer<Utf8> path);

abstract class NativeRuntimeBindings {
  NativeRuntimeBindings(DynamicLibrary library, String prefix) {
    final abi = library.lookupFunction<Uint32 Function(), int Function()>(
        '${prefix}_abi_version')();
    if (abi != 1) throw StateError('Unsupported $prefix ABI $abi');
    init =
        library.lookupFunction<WasmAbpInitC, WasmAbpInitDart>('${prefix}_init');
    version = library.lookupFunction<WasmAbpVersionC, WasmAbpVersionDart>(
        '${prefix}_version');
    getLastError =
        library.lookupFunction<WasmAbpGetLastErrorC, WasmAbpGetLastErrorDart>(
            '${prefix}_get_last_error');
    freeString =
        library.lookupFunction<WasmAbpFreeStringC, WasmAbpFreeStringDart>(
            '${prefix}_free_string');
    freeBytes = library.lookupFunction<WasmAbpFreeBytesC, WasmAbpFreeBytesDart>(
        '${prefix}_free_bytes');
    _diagnostics =
        library.lookupFunction<WasmAbpGetLastErrorC, WasmAbpGetLastErrorDart>(
            '${prefix}_diagnostics');
    _configureCache =
        library.lookupFunction<ConfigureCacheC, ConfigureCacheDart>(
            '${prefix}_configure_cache');
    if (init() != 0) throw StateError('Unable to initialize $prefix');
  }

  late final ConfigureCacheDart _configureCache;

  void configureCache(String? directory) {
    final path = (directory ?? '').toNativeUtf8();
    try {
      if (_configureCache(path) != 0) {
        throw StateError(
            readLastError() ?? 'Unable to configure compilation cache');
      }
    } finally {
      calloc.free(path);
    }
  }

  late final WasmAbpInitDart init;
  late final WasmAbpVersionDart version;
  late final WasmAbpGetLastErrorDart getLastError;
  late final WasmAbpFreeStringDart freeString;
  late final WasmAbpFreeBytesDart freeBytes;
  late final WasmAbpGetLastErrorDart _diagnostics;

  Map<String, Object?> get diagnostics {
    final pointer = _diagnostics();
    if (pointer == nullptr) {
      throw StateError(readLastError() ?? 'Runtime diagnostics unavailable');
    }
    try {
      return (jsonDecode(pointer.toDartString()) as Map)
          .cast<String, Object?>();
    } finally {
      freeString(pointer);
    }
  }

  String? readLastError() {
    final pointer = getLastError();
    if (pointer == nullptr) return null;
    try {
      return pointer.toDartString();
    } finally {
      freeString(pointer);
    }
  }
}

DynamicLibrary _openLibrary(String environmentKey) {
  final override = Platform.environment[environmentKey];
  if (override != null && override.isNotEmpty) {
    return DynamicLibrary.open(override);
  }
  return DynamicLibrary.codeAsset('package:wasm_abp/wasm_abp');
}

final class CoreBindings extends NativeRuntimeBindings {
  CoreBindings(DynamicLibrary library) : super(library, 'wasm_core') {
    moduleCompile = library.lookupFunction<ModuleCompileC, ModuleCompileDart>(
        'wasm_core_module_compile');
    moduleInstantiate =
        library.lookupFunction<ModuleInstantiateC, ModuleInstantiateDart>(
            'wasm_core_module_instantiate');
    moduleInstantiateConfig = library.lookupFunction<
        Int64 Function(Int64, Int32, Pointer<Utf8>),
        int Function(
            int, int, Pointer<Utf8>)>('wasm_core_module_instantiate_config');
    instanceCallJson = library.lookupFunction<
        Pointer<Utf8> Function(Int64, Pointer<Utf8>, Pointer<Utf8>),
        Pointer<Utf8> Function(
            int, Pointer<Utf8>, Pointer<Utf8>)>('wasm_core_instance_call_json');
    moduleDrop = library
        .lookupFunction<DropHandleC, DropHandleDart>('wasm_core_module_drop');

    instanceCall = library.lookupFunction<InstanceCallC, InstanceCallDart>(
        'wasm_core_instance_call');
    readMemory = library.lookupFunction<ReadMemoryC, ReadMemoryDart>(
        'wasm_core_instance_read_memory');
    writeMemory = library.lookupFunction<WriteMemoryC, WriteMemoryDart>(
        'wasm_core_instance_write_memory');
    instanceDrop = library
        .lookupFunction<DropHandleC, DropHandleDart>('wasm_core_instance_drop');
  }
  static CoreBindings? _instance;
  static bool get isLoaded => _instance != null;
  static CoreBindings get instance => _instance ??=
      CoreBindings(_openLibrary('ORONBOX_WASM_ABP_LIBRARY'));

  late final ModuleCompileDart moduleCompile;
  late final ModuleInstantiateDart moduleInstantiate;
  late final int Function(int, int, Pointer<Utf8>) moduleInstantiateConfig;
  late final Pointer<Utf8> Function(int, Pointer<Utf8>, Pointer<Utf8>)
      instanceCallJson;
  late final DropHandleDart moduleDrop;

  late final InstanceCallDart instanceCall;
  late final ReadMemoryDart readMemory;
  late final WriteMemoryDart writeMemory;
  late final DropHandleDart instanceDrop;
}

final class ComponentBindings extends NativeRuntimeBindings {
  ComponentBindings(DynamicLibrary library) : super(library, 'wasm_abp') {
    componentCompile =
        library.lookupFunction<ModuleCompileC, ModuleCompileDart>(
            'wasm_abp_component_compile');
    componentCompileForApi = library.lookupFunction<ComponentCompileForApiC,
        ComponentCompileForApiDart>('wasm_abp_component_compile_for_api');
    componentPrecompile =
        library.lookupFunction<ComponentPrecompileC, ComponentPrecompileDart>(
            'wasm_abp_component_precompile');
    componentDeserialize =
        library.lookupFunction<ComponentDeserializeC, ComponentDeserializeDart>(
            'wasm_abp_component_deserialize');
    componentInstantiate =
        library.lookupFunction<ComponentInstantiateC, ComponentInstantiateDart>(
            'wasm_abp_component_instantiate');
    componentInstanceDrop = library.lookupFunction<DropHandleC, DropHandleDart>(
        'wasm_abp_component_instance_drop');
    componentDrop = library
        .lookupFunction<DropHandleC, DropHandleDart>('wasm_abp_component_drop');

    componentSubmit =
        library.lookupFunction<ComponentMessageC, ComponentMessageDart>(
            'wasm_abp_component_submit');
    componentRespond =
        library.lookupFunction<ComponentMessageC, ComponentMessageDart>(
            'wasm_abp_component_respond');
    componentPoll = library.lookupFunction<ComponentPollC, ComponentPollDart>(
        'wasm_abp_component_poll');
  }
  static ComponentBindings? _instance;
  static bool get isLoaded => _instance != null;
  static ComponentBindings get instance => _instance ??=
      ComponentBindings(_openLibrary('ORONBOX_WASM_ABP_LIBRARY'));

  late final ModuleCompileDart componentCompile;
  late final ComponentCompileForApiDart componentCompileForApi;
  late final ComponentPrecompileDart componentPrecompile;
  late final ComponentDeserializeDart componentDeserialize;
  late final ComponentInstantiateDart componentInstantiate;
  late final DropHandleDart componentInstanceDrop;
  late final DropHandleDart componentDrop;

  late final ComponentMessageDart componentSubmit;
  late final ComponentMessageDart componentRespond;
  late final ComponentPollDart componentPoll;
}
