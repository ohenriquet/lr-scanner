import 'dart:io';
import 'dart:isolate';
import 'package:flutter/services.dart';
import 'l10n.dart';
import 'bridge.dart';

/// Ligação com o carro via ponte nativa (FFI). Cada operação roda num
/// `Isolate.run` para não travar a UI; a sessão é global na DLL (um processo),
/// então persiste entre as chamadas. Opcionalmente recebe o caminho da DLL.
class CarLink {
  static bool get available => true;

  final String? path;
  CarLink([this.path]);

  // Canal com o lado Android (Kotlin) para abrir o Mongoose via USB-OTG e
  // devolver o file descriptor do UsbManager.
  static const _usb = MethodChannel('jlr/usb');

  Future<Map<String, dynamic>> connect(String port, {int baud = 500000}) async {
    final p = path;
    if (Platform.isAndroid) {
      // No Android não há porta serial: pede o fd do Mongoose ao UsbManager
      // (com permissão) e conecta por ele. EXPERIMENTAL.
      int? fd;
      try {
        fd = await _usb
            .invokeMethod<int>('openMongoose')
            .timeout(const Duration(seconds: 40), onTimeout: () => -5);
      } catch (e) {
        return {'ok': false, 'error': 'USB: $e'};
      }
      if (fd == null || fd < 0) {
        return {
          'ok': false,
          'error': L.t('Mongoose not found or USB permission denied')
        };
      }
      final theFd = fd;
      return Isolate.run(() => JlrBridge.open(p).connectFd(theFd, baud: baud));
    }
    return Isolate.run(() => JlrBridge.open(p).connect(port, baud: baud));
  }

  Future<void> disconnect() {
    final p = path;
    return Isolate.run(() => JlrBridge.open(p).disconnect());
  }

  Future<String?> vin() {
    final p = path;
    return Isolate.run(() => JlrBridge.open(p).vin());
  }

  Future<List<List<dynamic>>> scan(List<List<dynamic>> modules) {
    final p = path;
    return Isolate.run(() => JlrBridge.open(p).scan(modules));
  }

  Future<Map<String, List<(String, int)>>> dtcs(
      List<List<dynamic>> present) {
    final p = path;
    return Isolate.run(() => JlrBridge.open(p).dtcs(present));
  }

  /// Apaga os DTCs dos módulos informados (UDS 0x14). ESCREVE no veículo.
  Future<Map<String, dynamic>> clearDtcs(List<List<dynamic>> modules) {
    final p = path;
    return Isolate.run(() => JlrBridge.open(p).clearDtcs(modules));
  }

  /// Lê vários DIDs numa tacada (um isolate). `reqs` = [[req,resp,did], ...].
  /// Devolve "req:resp:did" -> hex (ou null).
  Future<Map<String, String?>> readMany(List<List<int>> reqs) {
    final p = path;
    return Isolate.run(() {
      final b = JlrBridge.open(p);
      final out = <String, String?>{};
      for (final r in reqs) {
        out['${r[0]}:${r[1]}:${r[2]}'] = b.readDid(r[0], r[1], r[2]);
      }
      return out;
    });
  }
}
