import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import '../engine/wasm_engine.dart';
import '../ffi/bindings.dart';

final class AbpComponent {
  AbpComponent._(this._handle, this._bindings);

  final int _handle;
  final ComponentBindings _bindings;
  bool _disposed = false;

  int get handle => _handle;
  bool get isDisposed => _disposed;

  static bool get isRuntimeLoaded => ComponentBindings.isLoaded;
  static Map<String, Object?> get runtimeDiagnostics =>
      ComponentBindings.instance.diagnostics;

  static void configureCache(String? directory) =>
      ComponentBindings.instance.configureCache(directory);

  static AbpComponent compile(Uint8List componentBytes, {int apiLevel = 3}) {
    final bindings = ComponentBindings.instance;
    final nativeBytes = calloc<Uint8>(componentBytes.length);
    nativeBytes.asTypedList(componentBytes.length).setAll(0, componentBytes);
    try {
      final handle = bindings.componentCompileForApi(
        nativeBytes,
        componentBytes.length,
        apiLevel,
      );
      if (handle < 0) {
        final err = bindings.readLastError() ?? 'Component compilation failed';
        throw WasmAbpException(err);
      }
      return AbpComponent._(handle, bindings);
    } finally {
      calloc.free(nativeBytes);
    }
  }

  static Uint8List precompile(Uint8List componentBytes) {
    final bindings = ComponentBindings.instance;
    final nativeBytes = calloc<Uint8>(componentBytes.length);
    nativeBytes.asTypedList(componentBytes.length).setAll(0, componentBytes);
    final outPtr = calloc<Pointer<Uint8>>();
    final outLen = calloc<IntPtr>();
    try {
      final res = bindings.componentPrecompile(
        nativeBytes,
        componentBytes.length,
        outPtr,
        outLen,
      );
      if (res < 0) {
        final err =
            bindings.readLastError() ?? 'Component precompilation failed';
        throw WasmAbpException(err);
      }
      final ptr = outPtr.value;
      final len = outLen.value;
      try {
        return Uint8List.fromList(ptr.asTypedList(len));
      } finally {
        bindings.freeBytes(ptr, len);
      }
    } finally {
      calloc.free(nativeBytes);
      calloc.free(outPtr);
      calloc.free(outLen);
    }
  }

  static AbpComponent deserialize(Uint8List cwasmBytes) {
    final bindings = ComponentBindings.instance;
    final nativeBytes = calloc<Uint8>(cwasmBytes.length);
    nativeBytes.asTypedList(cwasmBytes.length).setAll(0, cwasmBytes);
    try {
      final handle = bindings.componentDeserialize(
        nativeBytes,
        cwasmBytes.length,
      );
      if (handle < 0) {
        final err =
            bindings.readLastError() ?? 'Component deserialization failed';
        throw WasmAbpException(err);
      }
      return AbpComponent._(handle, bindings);
    } finally {
      calloc.free(nativeBytes);
    }
  }

  /// [preopens] 将宿主目录挂载到组件，键为 guest 路径，值为宿主目录
  AbpComponentInstance instantiate({Map<String, String> preopens = const {}}) {
    _ensureActive();
    final json = preopens.isEmpty
        ? nullptr.cast<Utf8>()
        : jsonEncode([
            for (final e in preopens.entries) {'guest': e.key, 'host': e.value},
          ]).toNativeUtf8();
    final int instanceHandle;
    try {
      instanceHandle = _bindings.componentInstantiate(_handle, json);
    } finally {
      if (json != nullptr) calloc.free(json);
    }
    if (instanceHandle < 0) {
      final err = _bindings.readLastError() ?? 'Component instantiation failed';
      throw WasmAbpException(err);
    }
    return AbpComponentInstance._(instanceHandle, _bindings);
  }

  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _bindings.componentDrop(_handle);
  }

  void _ensureActive() {
    if (_disposed) throw StateError('AbpComponent is already disposed');
  }
}

final class AbpComponentInstance {
  AbpComponentInstance._(this._handle, this._bindings) {
    _poller = Timer.periodic(const Duration(milliseconds: 16), (_) => _poll());
  }

  final int _handle;
  final ComponentBindings _bindings;
  final _pending = <int, Completer<String>>{};
  late final Timer _poller;
  int _nextCall = 0;
  bool _disposed = false;
  String? _renderedUi;
  void Function(String)? _onUi;
  Future<Object?> Function(String, List<Object?>)? hostCall;
  void Function(Object)? onError;

  int get handle => _handle;
  bool get isDisposed => _disposed;

  void setUiListener(void Function(String)? listener) => _onUi = listener;

  Future<void> callOnLoad() async {
    await callExport('on-load', const []);
  }

  Future<String> callOnUiRender({String elementId = ''}) => callExport(
    'on-ui-render',
    [elementId],
  ).then((value) => value.isEmpty ? '{}' : value);

  Future<String> callOnUiEvent({
    required String eventId,
    String eventType = 'CLICK',
    String payload = '{}',
  }) => callExport('on-ui-event', [eventId, eventType, payload]);

  Future<String> callOnEvent({
    required String eventName,
    String payload = '{}',
  }) => callExport('on-event', [eventName, payload]);

  Future<String> callExport(String name, List<Object?> args) {
    if (_disposed) return Future.error(StateError('Plugin component disposed'));
    final id = ++_nextCall;
    final completer = Completer<String>();
    _pending[id] = completer;
    try {
      _message(_bindings.componentSubmit, {
        'id': id,
        'name': name,
        'args': args,
      });
    } catch (error, stack) {
      _pending.remove(id);
      completer.completeError(error, stack);
    }
    return completer.future;
  }

  void _message(ComponentMessageDart send, Map<String, Object?> message) {
    final json = jsonEncode(message).toNativeUtf8();
    try {
      if (send(_handle, json) < 0) {
        throw WasmAbpException(
          _bindings.readLastError() ?? 'Plugin message failed',
        );
      }
    } finally {
      calloc.free(json);
    }
  }

  void _poll() {
    if (_disposed) return;
    final pointer = _bindings.componentPoll(_handle);
    if (pointer == nullptr) {
      _fail(
        WasmAbpException(_bindings.readLastError() ?? 'Plugin driver failed'),
      );
      return;
    }
    final List events;
    try {
      events = jsonDecode(pointer.toDartString()) as List;
    } finally {
      _bindings.freeString(pointer);
    }
    for (final event in events.cast<Map>()) {
      switch (event['type']) {
        case 'ui':
          _renderedUi = event['json'] as String;
          _onUi?.call(_renderedUi!);
        case 'host':
          unawaited(_serveHost(event));
        case 'host-notify':
          unawaited(_serveHost(event, respond: false));
        case 'result':
          final pending = _pending.remove(event['id']);
          if (event.containsKey('error')) {
            pending?.completeError(WasmAbpException(event['error'].toString()));
          } else {
            pending?.complete(event['value'] as String);
          }
        case 'error':
          _fail(WasmAbpException(event['error'].toString()));
      }
    }
  }

  Future<void> _serveHost(Map event, {bool respond = true}) async {
    Map<String, Object?> response;
    try {
      final handler = hostCall;
      if (handler == null) throw StateError('Plugin host unavailable');
      response = {
        'id': event['id'],
        'value': await handler(
          event['method'] as String,
          (event['args'] as List).cast<Object?>(),
        ),
      };
    } catch (error) {
      response = {'id': event['id'], 'error': error.toString()};
    }
    if (!_disposed && respond) {
      try {
        _message(_bindings.componentRespond, response);
      } catch (error) {
        onError?.call(error);
      }
    }
  }

  void _fail(Object error) {
    for (final pending in _pending.values) {
      pending.completeError(error);
    }
    _pending.clear();
    onError?.call(error);
  }

  String? getRenderedUi() => _renderedUi;

  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _poller.cancel();
    _onUi = null;
    _renderedUi = null;
    hostCall = null;
    _bindings.componentInstanceDrop(_handle);
    onError = null;
    _fail(StateError('Plugin component disposed'));
  }
}
