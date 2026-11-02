import 'dart:convert';
import 'package:http/http.dart' as http;

/// Na web, a ligação com o carro passa por um servidor local (car-scan serve),
/// que lê o Mongoose e expõe a leitura por HTTP na mesma origem. Sem servidor,
/// o Conectar falha de forma limpa (modo referência continua funcionando).
class CarLink {
  static bool get available => true;

  CarLink([String? path]);

  Uri _u(String p) => Uri.base.resolve('api/$p');

  Future<Map<String, dynamic>> connect(String port, {int baud = 500000}) async {
    try {
      final r = await http.post(_u('connect'), body: '{}');
      if (r.statusCode == 200) {
        return jsonDecode(r.body) as Map<String, dynamic>;
      }
      return {'ok': false, 'error': 'servidor respondeu ${r.statusCode}'};
    } catch (e) {
      return {
        'ok': false,
        'error': 'Sem servidor local. Rode: car-scan --port COM5 serve '
            '--web app/jlr_app/build/web',
      };
    }
  }

  Future<void> disconnect() async {}

  Future<String?> vin() async {
    try {
      final r = await http.get(_u('vin'));
      return (jsonDecode(r.body)['vin']) as String?;
    } catch (_) {
      return null;
    }
  }

  Future<List<List<dynamic>>> scan(List<List<dynamic>> modules) async {
    try {
      final r = await http.post(_u('scan'), body: jsonEncode(modules));
      return ((jsonDecode(r.body)['present'] as List?) ?? const [])
          .map((e) => (e as List).toList())
          .toList();
    } catch (_) {
      return const [];
    }
  }

  Future<Map<String, List<(String, int)>>> dtcs(
      List<List<dynamic>> present) async {
    try {
      final r = await http.post(_u('dtcs'), body: jsonEncode(present));
      final j = jsonDecode(r.body) as Map<String, dynamic>;
      final out = <String, List<(String, int)>>{};
      for (final m in (j['modules'] as List? ?? const [])) {
        final mm = m as Map<String, dynamic>;
        out[mm['module'] as String] = ((mm['dtcs'] as List?) ?? const [])
            .map((d) => (d['code'] as String, d['status'] as int))
            .toList();
      }
      return out;
    } catch (_) {
      return const {};
    }
  }

  Future<Map<String, String?>> readMany(List<List<int>> reqs) async {
    try {
      final r = await http.post(_u('readmany'), body: jsonEncode(reqs));
      final j = jsonDecode(r.body) as Map<String, dynamic>;
      return j.map((k, v) => MapEntry(k, v as String?));
    } catch (_) {
      return const {};
    }
  }
}
