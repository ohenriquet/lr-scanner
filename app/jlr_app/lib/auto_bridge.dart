import 'dart:async';
import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'app_core.dart';
import 'car_state.dart';
import 'pack.dart';

/// Um sinal escolhido para um grupo do Android Auto: pelo DID exato ou, quando
/// o DID muda entre pacotes, pelo primeiro nome que casa com [name].
class SignalPick {
  final String module;
  final String? did;
  final RegExp? name;
  const SignalPick.did(this.module, String this.did) : name = null;
  SignalPick.name(this.module, String pattern)
      : did = null,
        name = RegExp(pattern, caseSensitive: false);
}

/// Grupo de sinais da tela "Ao vivo" do carro. O título é traduzido no lado
/// Android (strings.xml) pelo [id].
class AutoGroup {
  final String id;
  final List<SignalPick> picks;
  const AutoGroup(this.id, this.picks);
}

/// Grupos de sinais do Android Auto (suspensão, motor, elétrica, câmbio).
/// O grupo extra `phone` traz os sinais que o usuário marcou no celular.
///
/// DIDs conferidos no pacote do L319 MY12 (só sinais decodificáveis, com
/// unidade). Os nomes do pacote vêm no idioma do SDD (PT ou EN), então os
/// padrões de nome, usados quando o DID não existe em outro pacote, aceitam os
/// dois. O RLM do pacote não expõe sensor de pressão da suspensão (line/gallery
/// pressure); se um pacote tiver, o padrão `press` pega.
final autoGroups = <AutoGroup>[
  AutoGroup('suspension', [
    const SignalPick.did('RLM', '0x3B03'), // altura dianteira esquerda
    const SignalPick.did('RLM', '0x3B04'), // altura dianteira direita
    const SignalPick.did('RLM', '0x3B05'), // altura traseira esquerda
    const SignalPick.did('RLM', '0x3B06'), // altura traseira direita
    SignalPick.name('RLM', r'(gallery|line|reservoir|linha|galeria|reservat).*press|press.*(suspens|galeria|reservat)'),
    const SignalPick.did('RLM', '0xD11A'), // temperatura do compressor
  ]),
  AutoGroup('engine', [
    const SignalPick.did('PCM', '0xF405'), // temperatura do líquido de arrefecimento
    const SignalPick.did('PCM', '0xF40C'), // rotação do motor
    const SignalPick.did('PCM', '0x0322'), // pressão absoluta do coletor (turbo)
    const SignalPick.did('PCM', '0x0504'), // vazão de ar (MAF)
    SignalPick.name('PCM', r'coolant.*temp|arrefecimento do motor$'),
    SignalPick.name('PCM', r'^engine speed|^rota[cç][aã]o do motor|^rpm'),
  ]),
  AutoGroup('electrical', [
    const SignalPick.did('PCM', '0x1172'), // tensão da bateria (PCM)
    const SignalPick.did('BCM', '0x402A'), // tensão da bateria do veículo
    const SignalPick.did('BCM', '0x4028'), // carga da bateria (estimada)
    SignalPick.name('PCM', r'battery.*volt|tens[aã]o da bateria'),
  ]),
  AutoGroup('transmission', [
    const SignalPick.did('TCM', '0x1E69'), // temperatura do óleo da transmissão
    const SignalPick.did('TCM', '0xD904'), // idem (interpretação de software)
    SignalPick.name('TCM', r'(fluid|oil).*temp|temperatura do [oó]leo da transmiss'),
  ]),
];

/// Máximo de sinais por grupo (a lista do carro mostra ~6 linhas dirigindo).
const maxGroupSignals = 6;

/// Chaves `MODULO|DID` de um grupo, resolvidas contra o pacote do veículo.
/// Um padrão de nome só entra se nenhum sinal já escolhido casar com ele (é a
/// reserva para quando o DID fixo não existe naquele pacote).
List<String> resolveGroup(Pack pack, AutoGroup g) {
  final out = <String>[];
  final names = <String>[];
  for (final pick in g.picks) {
    if (out.length >= maxGroupSignals) break;
    final params = pack.live[pick.module];
    if (params == null) continue;
    if (pick.name != null && names.any(pick.name!.hasMatch)) continue;
    for (final p in params) {
      final ok = pick.did != null
          ? p.did.toLowerCase() == pick.did!.toLowerCase()
          : pick.name!.hasMatch(p.name);
      final key = '${pick.module}|${p.did}';
      if (ok && !out.contains(key)) {
        out.add(key);
        names.add(p.name);
        break;
      }
    }
  }
  return out;
}

/// Monta o estado que a tela do carro desenha. Puro (testável).
Map<String, dynamic> autoSnapshot(CarState car, Map<String, List<String>> groups) {
  final dtcs = <Map<String, dynamic>>[];
  car.activeDtcs.forEach((mod, list) {
    for (final (code, status) in list) {
      final base = code.split('-').first;
      dtcs.add({
        'module': mod,
        'code': code,
        'desc': car.pack.dtcText[base] ?? '',
        'status': status,
        'confirmed': (status & 0x08) != 0, // confirmedDTC
      });
    }
  });
  // Confirmadas primeiro, depois por código (mesma regra da tela do celular).
  dtcs.sort((a, b) {
    final ca = a['confirmed'] as bool, cb = b['confirmed'] as bool;
    if (ca != cb) return ca ? -1 : 1;
    return (a['code'] as String).compareTo(b['code'] as String);
  });

  final live = <Map<String, dynamic>>[];
  for (final key in car.watchedKeys) {
    final w = car.watchedParam(key);
    if (w == null) continue;
    final (m, p) = w;
    final s = car.live[key];
    final h = car.history[key] ?? const <double>[];
    live.add({
      'key': key,
      'module': m.acronym,
      'did': p.did,
      'name': p.name,
      'unit': prettyUnit(p.unit),
      'value': s?.value,
      'display': s?.display ?? '…',
      'min': h.isEmpty ? null : h.reduce((a, b) => a < b ? a : b),
      'max': h.isEmpty ? null : h.reduce((a, b) => a > b ? a : b),
      'history': h,
    });
  }

  return {
    'connected': car.connected,
    'connecting': car.connecting,
    'reading': car.reading,
    'demo': car.demo,
    'clearing': car.clearing,
    'error': car.connectError,
    'vin': car.vin,
    'online': car.online.length,
    'modules': car.pack.modules.length,
    'scanning': car.dtcsScanning,
    'scanDone': car.dtcScanDone,
    'scanTotal': car.dtcScanTotal,
    'dtcs': dtcs,
    'groups': {
      ...groups,
      'phone': car.userWatchedKeys,
    },
    'live': live,
    'lastClear': car.lastClear?.toJson(),
  };
}

/// Ponte com a tela do carro (Android Auto), canal `lr/auto`.
///
/// O lado Kotlin (`auto/AutoBridge.kt`) desenha os templates; aqui fica a
/// lógica. Kotlin → Dart: `active`, `connect`, `disconnect`, `refresh`,
/// `clearDtcs`, `watch`, `unwatch`, `dtcHelp`, `demo`. Dart → Kotlin: `ready`
/// (pacote carregado) e `state` (snapshot JSON, com throttle, só com o carro ativo).
class AutoBridge {
  static const _ch = MethodChannel('lr/auto');
  static bool _active = false;
  static Timer? _pending;
  static CarState? _car;
  static Map<String, List<String>> _groups = const {};

  /// Liga a ponte. Só no Android (no desktop/web não há Android Auto).
  static Future<void> init() async {
    if (kIsWeb || defaultTargetPlatform != TargetPlatform.android) return;
    _ch.setMethodCallHandler(_handle);
    try {
      final pack = await AppCore.instance.pack;
      final car = AppCore.instance.carFor(pack);
      _car = car;
      _groups = {for (final g in autoGroups) g.id: resolveGroup(pack, g)};
      car.addListener(_schedule);
      await _ch.invokeMethod('ready', {'ok': true});
    } catch (e) {
      await _ch.invokeMethod('ready', {'ok': false, 'error': '$e'});
    }
  }

  static String _snapshot() => jsonEncode(autoSnapshot(_car!, _groups));

  /// Envia o estado no máximo a cada ~300 ms enquanto o carro está ativo.
  static void _schedule() {
    if (!_active || _pending != null) return;
    _pending = Timer(const Duration(milliseconds: 300), () {
      _pending = null;
      if (_active && _car != null) {
        unawaited(_ch.invokeMethod('state', _snapshot()));
      }
    });
  }

  static Future<Object?> _handle(MethodCall call) async {
    final car = _car;
    if (car == null) return null;
    final args = (call.arguments as Map?)?.cast<String, Object?>() ?? const {};
    switch (call.method) {
      case 'active':
        _active = args['active'] == true;
        if (!_active) car.releaseWatch(watchAuto);
        return _snapshot();
      case 'snapshot':
        return _snapshot();
      case 'connect':
        unawaited(car.connect('', 500000));
        return null;
      case 'disconnect':
        car.disconnect();
        return null;
      case 'refresh':
        unawaited(car.refresh());
        return null;
      case 'demo':
        car.loadDemo();
        return null;
      case 'clearDtcs':
        final r = await car.clearDtcs(module: args['module'] as String?);
        return jsonEncode(r.toJson());
      case 'watch':
        final id = args['group'] as String?;
        final keys = id == 'phone'
            ? const <String>[] // já observados pelo celular
            : (_groups[id] ?? const <String>[]);
        car.watchKeys(watchAuto, keys);
        return null;
      case 'unwatch':
        car.releaseWatch(watchAuto);
        return null;
      case 'dtcHelp':
        return jsonEncode(_help(car.pack, '${args['module']}', '${args['code']}'));
    }
    throw MissingPluginException('lr/auto: ${call.method}');
  }

  static Map<String, dynamic> _help(Pack pack, String module, String code) {
    final base = code.split('-').first;
    final h = pack.dtcHelp[base]?[module];
    return {
      'desc': pack.dtcText[base] ?? '',
      'supp': h?.suppDesc ?? const [],
      'symptoms': h?.symptoms ?? const [],
      'causes': h?.causes ?? const [],
      'actions': h?.actions ?? const [],
    };
  }
}
