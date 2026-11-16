import 'dart:typed_data';
import 'dart:async';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wasm_abp/wasm_abp.dart';

void main() {
  test('wasm_abp initialization and version check', () {
    final engine = WasmAbpEngine.shared;
    expect(engine.version, contains('wasm_core'));
    expect(engine.version, contains('WASI Preview 1'));
  });

  test(
    'native UI updates arrive asynchronously and stop after disposal',
    () async {
      final fixture = File('native/tests/fixtures/ui_notification.wasm');
      final component = AbpComponent.compile(fixture.readAsBytesSync());
      final instance = component.instantiate();
      final update = Completer<String>();
      final timerUpdate = Completer<String>();
      var calls = 0;
      instance.setUiListener((json) {
        calls++;
        if (!update.isCompleted) {
          update.complete(json);
        } else if (!timerUpdate.isCompleted) {
          timerUpdate.complete(json);
        }
      });
      try {
        await instance.callOnLoad();
        expect(
          await update.future.timeout(const Duration(seconds: 2)),
          contains('Background'),
        );
        expect(
          await timerUpdate.future.timeout(const Duration(seconds: 2)),
          contains('Background'),
        );
        expect(calls, 2);
        final reloading = instance.callOnLoad();
        final cancelled = expectLater(reloading, throwsStateError);
        instance.dispose();
        await cancelled;
        await Future<void>.delayed(const Duration(milliseconds: 20));
        expect(calls, 2);
      } finally {
        instance.dispose();
        component.dispose();
      }
    },
  );

  test(
    'v3 UI callback uses the current native ABI and uppercase events',
    () async {
      final fixture = File('native/tests/fixtures/event_v3.wasm');
      final component = AbpComponent.compile(fixture.readAsBytesSync());
      final instance = component.instantiate();
      instance.hostCall = (method, arguments) async => 'light';
      try {
        await instance.callOnUiEvent(eventId: 'callback', payload: 'value');
        await instance.callOnUiEvent(
          eventId: 'callback',
          eventType: 'CHANGE',
          payload: 'value',
        );
        await expectLater(
          instance.callOnUiEvent(
            eventId: 'callback',
            eventType: 'change',
            payload: 'value',
          ),
          throwsException,
        );
      } finally {
        instance.dispose();
        component.dispose();
      }
    },
  );

  test('compile and execute core wasm addition', () {
    // Binary WAT:
    // (module
    //   (func (export "add") (param i64 i64) (result i64)
    //     local.get 0
    //     local.get 1
    //     i64.add)
    // )
    final wasmBytes = Uint8List.fromList([
      0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // \0asm 1
      0x01, 0x07, 0x01, 0x60, 0x02, 0x7e, 0x7e, 0x01,
      0x7e, // type: (i64, i64) -> i64
      0x03, 0x02, 0x01, 0x00, // function: type 0
      0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00, // export: "add"
      0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x7c,
      0x0b, // code: local.get 0, local.get 1, i64.add, end
    ]);

    final engine = WasmAbpEngine.shared;
    final module = engine.compile(wasmBytes);
    final instance = module.instantiate();

    final results = instance.call('add', [40, 2]);
    expect(results, equals([42]));

    instance.dispose();
    module.dispose();
  });

  test('wasm linear memory read and write', () {
    // Binary WAT:
    // (module
    //   (memory (export "memory") 1)
    // )
    final wasmBytes = Uint8List.fromList([
      0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
      0x05, 0x03, 0x01, 0x00, 0x01, // memory 1 page min
      0x07, 0x0a, 0x01, 0x06, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02,
      0x00, // export "memory"
    ]);

    final engine = WasmAbpEngine.shared;
    final module = engine.compile(wasmBytes);
    final instance = module.instantiate();
    final mem = instance.getMemory('memory');

    final payload = Uint8List.fromList([1, 2, 3, 4, 5, 42]);
    mem.write(100, payload);

    final readBack = mem.read(100, 6);
    expect(readBack, equals(payload));

    instance.dispose();
    module.dispose();
  });

  test('compile and instantiate wasm component with ab host apis', () async {
    // Binary WebAssembly component header: \0asm with version 0x0001000d
    final compBytes = Uint8List.fromList([
      0x00,
      0x61,
      0x73,
      0x6d,
      0x0d,
      0x00,
      0x01,
      0x00,
    ]);
    final component = AbpComponent.compile(compBytes);
    expect(component.handle, greaterThan(0));

    final instance = component.instantiate();
    expect(instance.handle, greaterThan(0));

    try {
      await instance.callOnLoad();
    } catch (e) {
      expect(e.toString(), contains('not found'));
    }

    final uiJson = await instance.callOnUiRender();
    expect(uiJson, equals('{}'));

    instance.dispose();
    expect(instance.isDisposed, isTrue);

    component.dispose();
    expect(component.isDisposed, isTrue);
  });

  test('precompile component to cwasm and deserialize', () async {
    final compBytes = Uint8List.fromList([
      0x00,
      0x61,
      0x73,
      0x6d,
      0x0d,
      0x00,
      0x01,
      0x00,
    ]);
    final cwasm = AbpComponent.precompile(compBytes);
    expect(cwasm.isNotEmpty, isTrue);

    final component = AbpComponent.deserialize(cwasm);
    expect(component.handle, greaterThan(0));

    final instance = component.instantiate();
    expect(instance.handle, greaterThan(0));

    final uiJson = await instance.callOnUiRender();
    expect(uiJson, equals('{}'));

    instance.dispose();
    expect(instance.isDisposed, isTrue);

    component.dispose();
    expect(component.isDisposed, isTrue);
  });
}
