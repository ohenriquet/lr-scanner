import 'dart:async';
import 'dart:math' as math;
import 'package:flutter/foundation.dart';
import 'car_link.dart';
import 'pack.dart';
import 'l10n.dart';

/// Quem pediu para observar um sinal ao vivo. O celular ([watchUser]) e a tela
/// do carro ([watchAuto]) observam de forma independente: o carro largar os
/// sinais dele não desliga os que o usuário marcou no celular.
const watchUser = 'user';
const watchAuto = 'auto';

/// Amostras guardadas por sinal para os gráficos (60 s a ~2 Hz).
const historyLength = 120;

/// Resultado de uma limpeza de DTCs.
class ClearSummary {
  final List<String> cleared;
  final List<(String, int)> refused; // (módulo, NRC)
  final List<String> noResponse;
  final int remaining; // falhas na memória depois de reler
  final String? error;

  const ClearSummary({
    this.cleared = const [],
    this.refused = const [],
    this.noResponse = const [],
    this.remaining = 0,
    this.error,
  });

  Map<String, dynamic> toJson() => {
        'cleared': cleared,
        'refused': [
          for (final (m, nrc) in refused) {'module': m, 'nrc': nrc}
        ],
        'noResponse': noResponse,
        'remaining': remaining,
        'error': error,
      };
}

/// Estado da ligação com o carro e das leituras ao vivo. Partilhado pelas telas
/// do celular e pela tela do carro (Android Auto) — ver `app_core.dart`.
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

  /// Módulos presentes `[sigla, tx, rx]` do último scan (alvo de releitura e limpeza).
  List<List<dynamic>> _present = const [];

  /// Falhas lidas do carro: módulo -> [(código, status)].
  Map<String, List<(String, int)>> activeDtcs = {};
  bool reading = false;

  /// Leitura de DTCs em segundo plano (por módulo), com progresso.
  bool dtcsScanning = false;
  int dtcScanDone = 0;
  int dtcScanTotal = 0;

  /// Limpeza de DTCs em andamento e o resultado da última.
  bool clearing = false;
  ClearSummary? lastClear;

  /// Imagem do CCF lida do BCM (DID 0xDE00), ou null se não lida.
  List<int>? ccfImage;

  // ─── live ───
  // chave "MODULO|DID" -> (módulo, parâmetro)
  final Map<String, (Module, LiveParam)> _watched = {};
  final Map<String, Set<String>> _owners = {}; // mesma chave -> quem observa
  final Map<String, LiveSample> live = {}; // mesma chave -> amostra
  final Map<String, List<double>> history = {}; // mesma chave -> últimas amostras
  final Map<String, double> _demoCenter = {};
  Timer? _poll;

  List<List<dynamic>> _moduleList() => pack.modules
      .where((m) => m.tx != null && m.rx != null)
      .map((m) => [m.acronym, m.tx, m.rx])
      .toList();

  int get dtcTotal => activeDtcs.values.fold(0, (a, l) => a + l.length);

  Future<void> connect(String port, int baud) async {
    if (!supported) {
      connectError = L.t('Available only in the native app.');
      notifyListeners();
      return;
    }
    if (connecting) return;
    connecting = true;
    connectError = null;
    notifyListeners();
    try {
      final r = await link.connect(port, baud: baud);
      if (r['ok'] == true) {
        connected = true;
        demo = false;
        await refresh();
      } else {
        connectError = r['error']?.toString() ?? L.t('connection failed');
      }
    } catch (e) {
      connectError = '$e';
    } finally {
      connecting = false;
      notifyListeners();
    }
  }

  /// Pré-visualização do estado conectado com dados de EXEMPLO (não é o carro).
  /// Acionada por `?demo=1` ou pelo botão Demonstração do Android Auto. Os
  /// sinais observados oscilam para os gráficos se moverem.
  void loadDemo() {
    demo = true;
    connected = true;
    connectError = null;
    lastClear = null;
    vin = 'SALLAAAF4CA606217';
    online
      ..clear()
      ..addAll(const [
        'PCM', 'TCM', 'RLM', 'ABS', 'BCM', 'IPC', 'SASM', 'ATCM', 'RDCM', 'PBM'
      ]);
    _present = _moduleList().where((e) => online.contains(e[0])).toList();
    activeDtcs = {
      'RLM': const [('C1A13-08', 0x2F), ('C1A20-00', 0x2F)],
      'PCM': const [('P0671', 0x2F)],
      'SASM': const [('C0051', 0x24)], // pendente, não confirmada
    };
    _demoSample('RLM', '0x3B03', 0.012, '0.01 m');
    _demoSample('RLM', '0x3B04', 0.004, '0.00 m');
    _demoSample('PCM', '0x0314', 11.4, '11.4 g/s');
    _demoSample('PCM', '0x0301', 1.31, '1.31 V');
    // CCF de exemplo: liga a luz de acesso (byte 47 = 4) e a puddle (282 = 2).
    final img = List<int>.filled(1024, 0);
    img[47] = 0x04; // approach lamp "De série"
    img[196 + 86] = 0x02; // puddle "Luzes de cortesia"
    ccfImage = img;
    _syncPolling();
    notifyListeners();
  }

  void _demoSample(String module, String did, double value, String display) {
    final p = _param(module, did);
    if (p == null) return;
    final key = '$module|$did';
    _watched[key] = (_module(module), p);
    (_owners[key] ??= {}).add(watchUser);
    live[key] = LiveSample(value, display);
    _demoCenter[key] = value;
  }

  LiveParam? _param(String module, String did) {
    for (final x in pack.live[module] ?? const <LiveParam>[]) {
      if (x.did.toLowerCase() == did.toLowerCase()) return x;
    }
    return null;
  }

  Module _module(String acronym) => pack.modules
      .firstWhere((x) => x.acronym == acronym, orElse: () => Module(acronym: acronym));

  /// Relê VIN, varre módulos e lê as falhas ativas.
  /// Conexão RÁPIDA: lê VIN + varre módulos e já libera o "conectado" (spinner
  /// sai). As falhas (por módulo) e o CCF são lidos EM SEGUNDO PLANO, sem travar
  /// a UI — a tela de falhas mostra o progresso e vai preenchendo.
  Future<void> refresh() async {
    if (!connected) return;
    if (demo) {
      loadDemo();
      return;
    }
    reading = true;
    activeDtcs = {};
    notifyListeners();
    List<List<dynamic>> present = const [];
    try {
      vin = await link.vin();
      notifyListeners(); // VIN na hora
      present = await link.scan(_moduleList());
      _present = present;
      online
        ..clear()
        ..addAll(present.map((e) => e[0].toString()));
    } catch (e) {
      connectError = '$e';
    } finally {
      reading = false;
      notifyListeners(); // conectado: VIN + módulos visíveis, spinner sai
    }
    if (!connected) return;
    // Em segundo plano, sem await: não bloqueiam o "conectado".
    unawaited(readCcf());
    unawaited(_scanDtcs(present));
  }

  /// Um item por par tx/rx (siglas diferentes podem dividir o mesmo endereço).
  static List<List<dynamic>> _dedup(List<List<dynamic>> mods) {
    final seen = <String>{};
    return mods.where((e) => seen.add('${e[1]}:${e[2]}')).toList();
  }

  /// Lê os DTCs um módulo por vez, atualizando a UI a cada módulo.
  Future<void> _scanDtcs(List<List<dynamic>> present) async {
    final uniq = _dedup(present);
    dtcsScanning = true;
    dtcScanTotal = uniq.length;
    dtcScanDone = 0;
    notifyListeners();
    for (final m in uniq) {
      if (!connected) break;
      try {
        final one = await link.dtcs([m]);
        one.forEach((mod, list) {
          if (list.isNotEmpty) activeDtcs[mod] = list;
        });
      } catch (_) {
        // módulo que falhou não trava o resto
      }
      dtcScanDone++;
      notifyListeners();
    }
    dtcsScanning = false;
    notifyListeners();
  }

  /// Apaga as falhas (UDS 0x14) de todos os módulos presentes, ou só de
  /// [module], e relê esses módulos. ESCREVE no veículo: só chamar depois da
  /// confirmação do usuário.
  Future<ClearSummary> clearDtcs({String? module}) async {
    if (!connected) {
      return lastClear = ClearSummary(error: L.t('Not connected'));
    }
    if (clearing || dtcsScanning || reading) {
      return ClearSummary(error: L.t('Busy — try again in a moment'));
    }
    clearing = true;
    lastClear = null;
    notifyListeners();
    try {
      final targets = _dedup(module == null
          ? _present
          : _present.where((e) => e[0] == module).toList());
      if (targets.isEmpty) {
        return lastClear = ClearSummary(
            error: L.t('No module to clear'), remaining: dtcTotal);
      }
      if (demo) return lastClear = await _clearDemo(targets);

      final r = await link.clearDtcs(targets);
      if (r['error'] != null) {
        return lastClear =
            ClearSummary(error: r['error'].toString(), remaining: dtcTotal);
      }
      final cleared = <String>[], noResp = <String>[];
      final refused = <(String, int)>[];
      for (final m in (r['modules'] as List? ?? const [])) {
        final mm = m as Map<String, dynamic>;
        final name = mm['module'].toString();
        switch (mm['result']) {
          case 'cleared':
            cleared.add(name);
          case 'refused':
            refused.add((name, (mm['nrc'] as num?)?.toInt() ?? 0));
          default:
            noResp.add(name);
        }
      }
      // Relê os módulos limpos: a falha que continua presente volta na hora.
      for (final t in targets) {
        _dropDtcsAt(t[1], t[2]);
      }
      notifyListeners();
      await _scanDtcs(targets);
      return lastClear = ClearSummary(
        cleared: cleared,
        refused: refused,
        noResponse: noResp,
        remaining: dtcTotal,
      );
    } catch (e) {
      return lastClear = ClearSummary(error: '$e', remaining: dtcTotal);
    } finally {
      clearing = false;
      notifyListeners();
    }
  }

  /// Remove as falhas guardadas de todas as siglas que usam o endereço tx/rx.
  void _dropDtcsAt(Object? tx, Object? rx) {
    for (final m in pack.modules) {
      if (m.tx == tx && m.rx == rx) activeDtcs.remove(m.acronym);
    }
  }

  Future<ClearSummary> _clearDemo(List<List<dynamic>> targets) async {
    await Future<void>.delayed(const Duration(milliseconds: 1200));
    final cleared = <String>[];
    for (final t in targets) {
      cleared.add(t[0].toString());
      _dropDtcsAt(t[1], t[2]);
    }
    return ClearSummary(cleared: cleared, remaining: dtcTotal);
  }

  /// Lê o CCF do BCM (DID 0xDE00) via a mesma leitura 0x22 usada nos DIDs.
  Future<void> readCcf() async {
    final bcms = pack.modules.where((m) => m.acronym == 'BCM');
    if (bcms.isEmpty) return;
    final bcm = bcms.first;
    if (bcm.tx == null || bcm.rx == null) return;
    final hex = await link.readMany([
      [bcm.tx!, bcm.rx!, 0xDE00]
    ]).then((m) => m['${bcm.tx}:${bcm.rx}:${0xDE00}']);
    if (hex == null || hex.length < 2) return;
    final bytes = <int>[];
    for (var i = 0; i + 1 < hex.length; i += 2) {
      bytes.add(int.parse(hex.substring(i, i + 2), radix: 16));
    }
    ccfImage = bytes;
  }

  /// Decodifica um parâmetro do CCF: (valor cru mascarado, rótulo da opção).
  (int?, String) decodeCcf(CcfParam p) {
    final img = ccfImage;
    if (img == null) return (null, '—');
    final base = pack.ccf.blockBase[p.block];
    if (base == null) return (null, '—');
    final off = base + p.byteStart;
    if (off < 0 || off >= img.length) return (null, '—');
    final raw = img[off] & p.mask;
    for (final o in p.options) {
      if ((o.value & p.mask) == raw) return (raw, o.label);
    }
    return (raw, '0x${raw.toRadixString(16).padLeft(2, '0')}');
  }

  void disconnect() {
    _poll?.cancel();
    _poll = null;
    _watched.clear();
    _owners.clear();
    live.clear();
    history.clear();
    _demoCenter.clear();
    if (!demo) unawaited(link.disconnect());
    demo = false;
    connected = false;
    online.clear();
    _present = const [];
    activeDtcs = {};
    lastClear = null;
    vin = null;
    ccfImage = null;
    notifyListeners();
  }

  /// Observado pelo usuário no celular (o destaque da aba Ao vivo).
  bool isWatched(String module, LiveParam p) =>
      _owners['$module|${p.did}']?.contains(watchUser) ?? false;

  /// Chaves `MODULO|DID` observadas pelo usuário no celular.
  List<String> get userWatchedKeys => [
        for (final e in _owners.entries)
          if (e.value.contains(watchUser)) e.key
      ];

  /// Definição (módulo, parâmetro) de uma chave observada, se houver.
  (Module, LiveParam)? watchedParam(String key) => _watched[key];

  /// Todas as chaves observadas (por qualquer dono).
  Iterable<String> get watchedKeys => _watched.keys;

  /// Observa um sinal pelo (módulo, DID) — usado pelo auto-connect.
  void watchByDid(String module, String did) {
    final p = _param(module, did);
    if (p != null && !isWatched(module, p)) toggleWatch(module, p);
  }

  void toggleWatch(String module, LiveParam p) {
    final key = '$module|${p.did}';
    if (isWatched(module, p)) {
      _release(key, watchUser);
    } else {
      _watch(key, watchUser, module, p);
    }
    _syncPolling();
    notifyListeners();
  }

  /// Passa a observar exatamente [keys] (`MODULO|DID`) em nome de [owner],
  /// largando o que esse dono observava antes.
  void watchKeys(String owner, List<String> keys) {
    for (final k in _owners.keys.toList()) {
      if (!keys.contains(k)) _release(k, owner);
    }
    for (final k in keys) {
      final i = k.indexOf('|');
      if (i <= 0) continue;
      final module = k.substring(0, i);
      final p = _param(module, k.substring(i + 1));
      if (p != null) _watch('$module|${p.did}', owner, module, p);
    }
    _syncPolling();
    notifyListeners();
  }

  /// Larga tudo o que [owner] observava.
  void releaseWatch(String owner) => watchKeys(owner, const []);

  void _watch(String key, String owner, String module, LiveParam p) {
    _watched[key] ??= (_module(module), p);
    (_owners[key] ??= {}).add(owner);
  }

  void _release(String key, String owner) {
    final o = _owners[key];
    if (o == null) return;
    o.remove(owner);
    if (o.isNotEmpty) return;
    _owners.remove(key);
    _watched.remove(key);
    live.remove(key);
    history.remove(key);
  }

  void _syncPolling() {
    if (_watched.isEmpty) {
      _poll?.cancel();
      _poll = null;
      return;
    }
    _poll ??= Timer.periodic(const Duration(milliseconds: 500), (_) => _tick());
  }

  void _record(String key, LiveSample s) {
    live[key] = s;
    final v = s.value;
    if (v == null || !v.isFinite) return;
    final h = history[key] ??= <double>[];
    h.add(v);
    if (h.length > historyLength) h.removeRange(0, h.length - historyLength);
  }

  /// Uma leitura dos sinais observados agora (testes).
  @visibleForTesting
  Future<void> tickNow() => _tick();

  Future<void> _tick() async {
    if (!connected || _watched.isEmpty) return;
    if (demo) {
      _demoTick();
      notifyListeners();
      return;
    }
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
        final w = _watched[keys[i]];
        if (w == null) continue; // largado durante a leitura
        final hex = res['${reqs[i][0]}:${reqs[i][1]}:${reqs[i][2]}'];
        _record(keys[i], decodeLive(w.$2, hex));
      }
      notifyListeners();
    } catch (_) {
      // ignora um tick com erro
    }
  }

  /// Demo: cada sinal oscila em torno de um centro (onda lenta + ruído).
  final _rnd = math.Random(7);
  void _demoTick() {
    final t = DateTime.now().millisecondsSinceEpoch / 1000.0;
    _watched.forEach((key, v) {
      final p = v.$2;
      final c = _demoCenter[key] ??= _demoBase(p);
      final amp = math.max(c.abs() * 0.08, 0.5);
      final phase = (key.hashCode % 628) / 100.0;
      final val = c +
          amp * math.sin(t / 3.0 + phase) +
          amp * 0.25 * (_rnd.nextDouble() - 0.5);
      _record(key, LiveSample(val, formatLive(val, p.unit)));
    });
  }

  /// Centro plausível para o demo pela unidade do sinal.
  static double _demoBase(LiveParam p) {
    final u = (p.unit ?? '').toLowerCase();
    if (u == 'degc' || u.contains('°c')) return 88;
    if (u == 'pa') return 152000;
    if (u == 'pct') return 78;
    if (u.contains('rpm')) return 780;
    if (u.contains('kpa')) return 105;
    if (u.contains('bar')) return 10;
    if (u == 'v' || u.contains('volt')) return 13.9;
    if (u.contains('mm')) return 40;
    if (u == 'm') return 0.02;
    return 50;
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

/// Texto de um valor com a precisão conforme a magnitude, mais a unidade.
String formatLive(double val, String? unit) {
  final a = val.abs();
  final disp = a >= 100
      ? val.toStringAsFixed(0)
      : (a >= 10 ? val.toStringAsFixed(1) : val.toStringAsFixed(2));
  return '$disp ${prettyUnit(unit)}'.trim();
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
    return LiveSample(val, formatLive(val, p.unit));
  }
  return LiveSample(raw.toDouble(), '$raw');
}
