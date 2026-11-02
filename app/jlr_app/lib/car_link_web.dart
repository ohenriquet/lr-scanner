/// Stub da ligação com o carro para a web (não há FFI no navegador).
class CarLink {
  static bool get available => false;

  Map<String, dynamic> connect(String port, {int baud = 500000}) =>
      {'ok': false, 'error': 'A ligação com o carro exige o app nativo (FFI).'};

  void disconnect() {}

  String? vin() => null;

  List<List<dynamic>> scan(List<List<dynamic>> modules) => const [];

  Map<String, List<(String, int)>> dtcs(List<List<dynamic>> present) => const {};

  String? readDid(int req, int resp, int did) => null;
}
