import 'dart:async';
import 'package:flutter/foundation.dart';
import 'car_link.dart';
import 'pack.dart';

/// Estado da ligação com o carro e das leituras ao vivo. Partilhado pelas telas.
class CarState extends ChangeNotifier {
  final Pack pack;
  final CarLink link;

  CarState(this.pack, {String? dllPath}) : link = CarLink(dllPath);

  bool get supported => CarLink.available;

  bool connecting = false;
  bool connected = false;
  bool demo = false;
  String? connectError;
  String? vin;

  /// Siglas de módulos que responderam ao scan.
  final Set<String> online = {};

  /// Falhas ativas lidas do carro: módulo -> [(código, status)].
  Map<String, List<(String, int)>> activeDtcs = {};
  bool reading = false;

  // ─── live ───
  // chave "MODULO|DID" -> (módulo, parâmetro)
  final Map<String, (Module, LiveParam)> _watched = {};
  final Map<String, LiveSample> live = {}; // mesma chave -> amostra
  Timer? _poll;

  List<List<dynamic>> _moduleList() => pack.modules
      .where((m) => m.tx != null && m.rx != null)
      .map((m) => [m.acronym, m.tx, m.rx])
      .toList();

  Future<void> connect(String port, int baud) async {
    if (!supported) {
      connectError = 'Disponível apenas no app nativo.';
      notifyListeners();
      return;
    }
    connecting = true;
    connectError = null;
    notifyListeners();
    try {
      final r = await link.connect(port, baud: baud);
      if (r['ok'] == true) {
        connected = true;
        await refresh();
      } else {
        connectError = r['error']?.toString() ?? 'falha ao conectar';
      }
    } catch (e) {
      connectError = '$e';
    } finally {
      connecting = false;
      notifyListeners();
    }
  }

  /// Pré-visualização do estado conectado com dados de EXEMPLO (não é o carro).
  /// Acionada por `?demo=1`. Útil para ver a UI de conexão sem hardware.
  void loadDemo() {
    demo = true;
    connected = true;
    connectError = null;
    vin = 'SALLAAAF4CA606217';
    online
      ..clear()
      ..addAll(const [
        'PCM', 'TCM', 'RLM', 'ABS', 'BCM', 'IPC', 'SASM', 'ATCM', 'RDCM', 'PBM'
      ]);
    activeDtcs = {
      'RLM': const [('C1A13-08', 0x2F), ('C1A20-00', 0x2F)],
      'PCM': const [('P0671', 0x2F)],
      'SASM': const [('C0051', 0x2F)],
    };
    _demoSample('RLM', '0x3B03', 0.012, '0.01 m');
    _demoSample('RLM', '0x3B04', 0.004, '0.00 m');
    _demoSample('PCM', '0x0314', 11.4, '11.4 g/s');
    _demoSample('PCM', '0x0301', 1.31, '1.31 V');
    notifyListeners();
  }

  void _demoSample(String module, String did, double value, String display) {
    final params = pack.live[module];
    if (params == null) return;
    LiveParam? p;
    for (final x in params) {
      if (x.did == did) {
        p = x;
        break;
      }
    }
    if (p == null) return;
    final m = pack.modules.firstWhere((x) => x.acronym == module,
        orElse: () => Module(acronym: module));
    final key = '$module|$did';
    _watched[key] = (m, p);
    live[key] = LiveSample(value, display);
  }

  /// Relê VIN, varre módulos e lê as falhas ativas.
  Future<void> refresh() async {
    if (!connected) return;
    reading = true;
    notifyListeners();
    try {
      vin = await link.vin();
      final present = await link.scan(_moduleList());
      online
        ..clear()
        ..addAll(present.map((e) => e[0].toString()));
      activeDtcs = await link.dtcs(present);
    } catch (e) {
      connectError = '$e';
    } finally {
      reading = false;
      notifyListeners();
    }
  }

  void disconnect() {
    _poll?.cancel();
    _poll = null;
    _watched.clear();
    live.clear();
    unawaited(link.disconnect());
    connected = false;
    online.clear();
    activeDtcs = {};
    vin = null;
    notifyListeners();
  }

  bool isWatched(String module, LiveParam p) =>
      _watched.containsKey('$module|${p.did}');

  void toggleWatch(String module, LiveParam p) {
    final key = '$module|${p.did}';
    if (_watched.remove(key) != null) {
      live.remove(key);
    } else {
      final m = pack.modules.firstWhere((x) => x.acronym == module,
          orElse: () => Module(acronym: module));
      _watched[key] = (m, p);
    }
    _syncPolling();
    notifyListeners();
  }

  void _syncPolling() {
    if (_watched.isEmpty) {
      _poll?.cancel();
      _poll = null;
      return;
    }
    _poll ??= Timer.periodic(const Duration(milliseconds: 500), (_) => _tick());
  }

  Future<void> _tick() async {
    if (!connected || _watched.isEmpty) return;
    final reqs = <List<int>>[];
    final keys = <String>[];
    _watched.forEach((key, v) {
      final (m, p) = v;
      if (m.tx == null || m.rx == null) return;
      final did = int.tryParse(p.did.replaceFirst('0x', ''), radix: 16);
      if (did == null) return;
      reqs.add([m.tx!, m.rx!, did]);
      keys.add(key);
    });
    if (reqs.isEmpty) return;
    try {
      final res = await link.readMany(reqs);
      for (var i = 0; i < keys.length; i++) {
        final (_, p) = _watched[keys[i]]!;
        final hex = res['${reqs[i][0]}:${reqs[i][1]}:${reqs[i][2]}'];
        live[keys[i]] = decodeLive(p, hex);
      }
      notifyListeners();
    } catch (_) {
      // ignora um tick com erro
    }
  }

  @override
  void dispose() {
    _poll?.cancel();
    super.dispose();
  }
}

/// Uma amostra decodificada de um parâmetro ao vivo.
class LiveSample {
  final double? value;
  final String display;
  const LiveSample(this.value, this.display);
}

/// Decodifica os bytes (hex) de um DID num valor + texto, pela definição do pack.
LiveSample decodeLive(LiveParam p, String? hex) {
  if (hex == null || hex.length < 2) return const LiveSample(null, '—');
  final bytes = <int>[];
  for (var i = 0; i + 1 < hex.length; i += 2) {
    bytes.add(int.parse(hex.substring(i, i + 2), radix: 16));
  }
  if (bytes.isEmpty) return const LiveSample(null, '—');

  if (p.source == 'mdl') {
    // lsb..msb são índices de BIT sobre o inteiro big-endian de todos os bytes.
    final lsb = p.lsb, msb = p.msb;
    if (lsb == null || msb == null || msb < lsb) {
      return const LiveSample(null, '—');
    }
    var full = 0;
    for (final b in bytes) {
      full = (full << 8) | b;
    }
    final width = (msb - lsb + 1).clamp(1, 32);
    final mask = (1 << width) - 1;
    final raw = (full >> lsb) & mask;
    return LiveSample(raw.toDouble(), '$raw');
  }

  // fmt: bytes [lsb..msb] big-endian.
  final lo = (p.lsb ?? 0);
  var hi = (p.msb ?? (bytes.length - 1));
  if (lo < 0 || hi < 0 || lo >= bytes.length) {
    return const LiveSample(null, '—');
  }
  if (hi >= bytes.length) hi = bytes.length - 1;
  var raw = 0;
  for (var i = lo; i <= hi; i++) {
    raw = (raw << 8) | bytes[i];
  }
  if (p.kind == 'linear') {
    final m = p.mult ?? 1;
    final o = p.offset ?? 0;
    final val = (p.offsetFirst ?? true) ? (raw + o) * m : raw * m + o;
    final a = val.abs();
    final disp =
        a >= 100 ? val.toStringAsFixed(0) : (a >= 10 ? val.toStringAsFixed(1) : val.toStringAsFixed(2));
    return LiveSample(val, '$disp ${p.unit ?? ''}'.trim());
  }
  return LiveSample(raw.toDouble(), '$raw');
}
