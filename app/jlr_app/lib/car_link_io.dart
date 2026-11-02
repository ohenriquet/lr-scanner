import 'bridge.dart';

/// Ligação com o carro via ponte nativa (FFI). Alvos nativos.
class CarLink {
  static bool get available => true;

  JlrBridge? _b;

  JlrBridge get _lib => _b ??= JlrBridge.open();

  Map<String, dynamic> connect(String port, {int baud = 500000}) =>
      _lib.connect(port, baud: baud);

  void disconnect() => _b?.disconnect();

  String? vin() => _b?.vin();

  List<List<dynamic>> scan(List<List<dynamic>> modules) =>
      _b?.scan(modules) ?? const [];

  Map<String, List<(String, int)>> dtcs(List<List<dynamic>> present) =>
      _b?.dtcs(present) ?? const {};

  String? readDid(int req, int resp, int did) => _b?.readDid(req, resp, did);
}
