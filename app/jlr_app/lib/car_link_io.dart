import 'dart:isolate';
import 'bridge.dart';

/// Ligação com o carro via ponte nativa (FFI). Cada operação roda num
/// `Isolate.run` para não travar a UI; a sessão é global na DLL (um processo),
/// então persiste entre as chamadas. Opcionalmente recebe o caminho da DLL.
class CarLink {
  static bool get available => true;

  final String? path;
  CarLink([this.path]);

  Future<Map<String, dynamic>> connect(String port, {int baud = 500000}) {
    final p = path;
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
