import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';

// Assinaturas C <-> Dart da ponte `jlr_bridge` (cdylib do núcleo Rust).
typedef _StrFnC = Pointer<Utf8> Function();
typedef _StrFnD = Pointer<Utf8> Function();
typedef _ConnectC = Pointer<Utf8> Function(Pointer<Utf8>, Int32);
typedef _ConnectD = Pointer<Utf8> Function(Pointer<Utf8>, int);
typedef _StrArgC = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _StrArgD = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _DidC = Pointer<Utf8> Function(Int32, Int32, Int32);
typedef _DidD = Pointer<Utf8> Function(int, int, int);
typedef _VoidC = Void Function();
typedef _VoidD = void Function();
typedef _FreeC = Void Function(Pointer<Utf8>);
typedef _FreeD = void Function(Pointer<Utf8>);

/// Acesso à ponte nativa de diagnóstico (leitura do carro via Mongoose).
///
/// Nativo-only (Windows/macOS/Linux/Android): este arquivo importa `dart:ffi`
/// e não deve ser importado no build web.
class JlrBridge {
  final DynamicLibrary _lib;

  late final _version = _lib.lookupFunction<_StrFnC, _StrFnD>('jlr_version');
  late final _connect = _lib.lookupFunction<_ConnectC, _ConnectD>('jlr_connect');
  late final _disconnect = _lib.lookupFunction<_VoidC, _VoidD>('jlr_disconnect');
  late final _vin = _lib.lookupFunction<_StrFnC, _StrFnD>('jlr_vin');
  late final _scan = _lib.lookupFunction<_StrArgC, _StrArgD>('jlr_scan');
  late final _dtcs = _lib.lookupFunction<_StrArgC, _StrArgD>('jlr_dtcs');
  late final _readDid = _lib.lookupFunction<_DidC, _DidD>('jlr_read_did');
  late final _free = _lib.lookupFunction<_FreeC, _FreeD>('jlr_free');

  JlrBridge._(this._lib);

  /// Abre a biblioteca nativa. `path` opcional; senão procura em locais por
  /// plataforma (ao lado do executável, no bundle .app, etc.).
  factory JlrBridge.open([String? path]) {
    if (path != null) return JlrBridge._(DynamicLibrary.open(path));
    Object? lastErr;
    for (final cand in _candidates()) {
      try {
        return JlrBridge._(DynamicLibrary.open(cand));
      } catch (e) {
        lastErr = e;
      }
    }
    throw Exception('ponte nativa não encontrada (jlr_bridge): $lastErr');
  }

  /// Candidatos de caminho da lib, por plataforma. No Android a .so vem do APK
  /// (nome puro); no desktop fica relativa ao executável/bundle.
  static List<String> _candidates() {
    if (Platform.isAndroid) return const ['libjlr_bridge.so'];
    final dir = File(Platform.resolvedExecutable).parent.path;
    if (Platform.isWindows) {
      return ['$dir\\jlr_bridge.dll', 'jlr_bridge.dll'];
    }
    if (Platform.isMacOS) {
      // flutter macOS: libs ficam em <App>.app/Contents/Frameworks/.
      return [
        '$dir/../Frameworks/libjlr_bridge.dylib',
        '$dir/libjlr_bridge.dylib',
        'libjlr_bridge.dylib',
      ];
    }
    // Linux: flutter bundla em lib/ ao lado do executável.
    return ['$dir/lib/libjlr_bridge.so', '$dir/libjlr_bridge.so', 'libjlr_bridge.so'];
  }

  Map<String, dynamic> _call(Pointer<Utf8> p) {
    if (p == nullptr) return {'error': 'retorno nulo'};
    try {
      return jsonDecode(p.toDartString()) as Map<String, dynamic>;
    } finally {
      _free(p);
    }
  }

  Map<String, dynamic> version() => _call(_version());

  Map<String, dynamic> connect(String port, {int baud = 500000}) {
    final cp = port.toNativeUtf8();
    try {
      return _call(_connect(cp, baud));
    } finally {
      calloc.free(cp);
    }
  }

  void disconnect() => _disconnect();

  String? vin() => _call(_vin())['vin'] as String?;

  /// `modules` = lista de [sigla, req, resp].
  List<List<dynamic>> scan(List<List<dynamic>> modules) {
    final cp = jsonEncode(modules).toNativeUtf8();
    try {
      final r = _call(_scan(cp));
      return ((r['present'] as List?) ?? const [])
          .map((e) => (e as List).toList())
          .toList();
    } finally {
      calloc.free(cp);
    }
  }

  /// DTCs ativos dos módulos presentes. Devolve módulo -> [(code, status)].
  Map<String, List<(String, int)>> dtcs(List<List<dynamic>> present) {
    final cp = jsonEncode(present).toNativeUtf8();
    try {
      final r = _call(_dtcs(cp));
      final out = <String, List<(String, int)>>{};
      for (final m in (r['modules'] as List? ?? const [])) {
        final mm = m as Map<String, dynamic>;
        out[mm['module'] as String] = ((mm['dtcs'] as List?) ?? const [])
            .map((d) => (d['code'] as String, d['status'] as int))
            .toList();
      }
      return out;
    } finally {
      calloc.free(cp);
    }
  }

  /// Lê um DID cru (hex dos bytes de dado), ou null se sem resposta.
  String? readDid(int req, int resp, int did) =>
      _call(_readDid(req, resp, did))['hex'] as String?;
}
