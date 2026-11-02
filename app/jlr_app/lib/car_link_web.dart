/// Stub da ligação com o carro para a web (não há FFI no navegador).
class CarLink {
  static bool get available => false;

  CarLink([String? path]);

  Future<Map<String, dynamic>> connect(String port, {int baud = 500000}) async =>
      {'ok': false, 'error': 'A ligação com o carro exige o app nativo (FFI).'};

  Future<void> disconnect() async {}

  Future<String?> vin() async => null;

  Future<List<List<dynamic>>> scan(List<List<dynamic>> modules) async => const [];

  Future<Map<String, List<(String, int)>>> dtcs(
          List<List<dynamic>> present) async =>
      const {};

  Future<Map<String, String?>> readMany(List<List<int>> reqs) async => const {};
}
