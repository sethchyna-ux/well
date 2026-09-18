// well_ffi.dart - Dart FFI bindings for Well Terminal Engine on Android & Mobile
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

// Opaque session handle
final class WellSessionHandle extends Opaque {}

// C function signatures
typedef WellSessionCreateNative = Pointer<WellSessionHandle> Function(Uint16 rows, Uint16 cols);
typedef WellSessionCreateDart = Pointer<WellSessionHandle> Function(int rows, int cols);

typedef WellSessionWriteNative = Int32 Function(Pointer<WellSessionHandle> handle, Pointer<Uint8> bytes, IntPtr len);
typedef WellSessionWriteDart = int Function(Pointer<WellSessionHandle> handle, Pointer<Uint8> bytes, int len);

typedef WellSessionResizeNative = Int32 Function(Pointer<WellSessionHandle> handle, Uint16 rows, Uint16 cols);
typedef WellSessionResizeDart = int Function(Pointer<WellSessionHandle> handle, int rows, int cols);

typedef WellSessionIsDirtyNative = Int32 Function(Pointer<WellSessionHandle> handle);
typedef WellSessionIsDirtyDart = int Function(Pointer<WellSessionHandle> handle);

typedef WellSessionReadScreenNative = Int32 Function(Pointer<WellSessionHandle> handle, Pointer<Uint8> outBuf, IntPtr maxLen);
typedef WellSessionReadScreenDart = int Function(Pointer<WellSessionHandle> handle, Pointer<Uint8> outBuf, int maxLen);

typedef WellSessionDestroyNative = Void Function(Pointer<WellSessionHandle> handle);
typedef WellSessionDestroyDart = void Function(Pointer<WellSessionHandle> handle);

class WellTerminalBridge {
  static DynamicLibrary? _dylib;

  static DynamicLibrary get dylib {
    if (_dylib != null) return _dylib!;
    if (Platform.isAndroid) {
      _dylib = DynamicLibrary.open('libwell_ffi.so');
    } else if (Platform.isMacOS) {
      _dylib = DynamicLibrary.process();
    } else {
      _dylib = DynamicLibrary.open('libwell_ffi.so');
    }
    return _dylib!;
  }

  static WellSessionCreateDart get sessionCreate =>
      dylib.lookupFunction<WellSessionCreateNative, WellSessionCreateDart>('well_session_create');

  static WellSessionWriteDart get sessionWrite =>
      dylib.lookupFunction<WellSessionWriteNative, WellSessionWriteDart>('well_session_write');

  static WellSessionResizeDart get sessionResize =>
      dylib.lookupFunction<WellSessionResizeNative, WellSessionResizeDart>('well_session_resize');

  static WellSessionIsDirtyDart get sessionIsDirty =>
      dylib.lookupFunction<WellSessionIsDirtyNative, WellSessionIsDirtyDart>('well_session_is_dirty');

  static WellSessionReadScreenDart get sessionReadScreen =>
      dylib.lookupFunction<WellSessionReadScreenNative, WellSessionReadScreenDart>('well_session_read_screen');

  static WellSessionDestroyDart get sessionDestroy =>
      dylib.lookupFunction<WellSessionDestroyNative, WellSessionDestroyDart>('well_session_destroy');
}
