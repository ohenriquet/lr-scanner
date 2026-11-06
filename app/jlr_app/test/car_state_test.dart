import 'dart:io';
import 'package:flutter_test/flutter_test.dart';
import 'package:jlr_sdd/auto_bridge.dart';
import 'package:jlr_sdd/car_state.dart';
import 'package:jlr_sdd/pack.dart';

LiveParam _lin(String did, String name, String unit) => LiveParam(
    did: did, name: name, unit: unit, kind: 'linear', mult: 0.001, lsb: 0, msb: 1);

/// Pacote mínimo, só com o que os testes usam.
Pack testPack() => Pack(
      vehicle: const {'model': 'L319'},
      modules: [
        Module(acronym: 'PCM', tx: 0x7E0, rx: 0x7E8),
        Module(acronym: 'RLM', tx: 0x744, rx: 0x74C),
        Module(acronym: 'SASM', tx: 0x737, rx: 0x73F),
      ],
      live: {
        'PCM': [
          _lin('0x0301', 'Manifold absolute pressure sensor voltage', 'V'),
          _lin('0x0305', 'Engine coolant temperature', '°C'),
          _lin('0x0504', 'Mass air flow', 'g/s'),
        ],
        'RLM': [
          _lin('0x3B03', 'Front left height', 'm'),
          _lin('0x3B04', 'Front right height', 'm'),
          _lin('0x3B05', 'Rear left height', 'm'),
          _lin('0x3B10', 'Gallery (line) pressure', 'bar'),
        ],
      },
      dtcText: const {'C1A13': 'Front left height sensor', 'P0671': 'Glow plug 1'},
      dtcHelp: const {},
      ccf: const Ccf(blockBase: {}, params: []),
    );

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  late CarState car;

  setUp(() => car = CarState(testPack())..loadDemo());
  tearDown(() => car.dispose());

  test('o histórico guarda as amostras e respeita o tamanho máximo', () async {
    car.watchKeys(watchAuto, ['RLM|0x3B03']);
    for (var i = 0; i < historyLength + 15; i++) {
      await car.tickNow();
    }
    expect(car.history['RLM|0x3B03']!.length, historyLength);
    expect(car.live['RLM|0x3B03']!.value, isNotNull);
  });

  test('o carro largar os sinais dele não desliga os do celular', () {
    // O demo já observa RLM|0x3B03 em nome do usuário.
    expect(car.userWatchedKeys, contains('RLM|0x3B03'));
    car.watchKeys(watchAuto, ['RLM|0x3B03', 'RLM|0x3B05']);
    expect(car.watchedKeys, containsAll(['RLM|0x3B03', 'RLM|0x3B05']));

    car.releaseWatch(watchAuto);
    expect(car.watchedKeys, contains('RLM|0x3B03')); // continua (é do usuário)
    expect(car.watchedKeys, isNot(contains('RLM|0x3B05')));
  });

  test('desmarcar no celular mantém o sinal se o carro ainda observa', () {
    final p = testPack().live['RLM']!.first; // 0x3B03
    car.watchKeys(watchAuto, ['RLM|0x3B03']);
    car.toggleWatch('RLM', p); // usuário desmarca
    expect(car.isWatched('RLM', p), isFalse);
    expect(car.watchedKeys, contains('RLM|0x3B03'));
  });

  test('limpar um módulo (demo) apaga só as falhas dele', () async {
    expect(car.activeDtcs.keys, containsAll(['RLM', 'PCM', 'SASM']));
    final r = await car.clearDtcs(module: 'RLM');
    expect(r.error, isNull);
    expect(r.cleared, ['RLM']);
    expect(car.activeDtcs.containsKey('RLM'), isFalse);
    expect(car.activeDtcs.keys, containsAll(['PCM', 'SASM']));
    expect(r.remaining, car.dtcTotal);
  });

  test('limpar tudo (demo) zera a memória de falhas', () async {
    final r = await car.clearDtcs();
    expect(r.error, isNull);
    expect(r.remaining, 0);
    expect(car.activeDtcs, isEmpty);
    expect(car.lastClear, same(r));
  });

  test('limpar desconectado devolve erro sem tocar no carro', () async {
    car.disconnect();
    final r = await car.clearDtcs();
    expect(r.error, isNotNull);
  });

  group('Android Auto', () {
    test('grupos resolvem por DID e por nome', () {
      final pack = testPack();
      final susp = autoGroups.firstWhere((g) => g.id == 'suspension');
      expect(resolveGroup(pack, susp),
          ['RLM|0x3B03', 'RLM|0x3B04', 'RLM|0x3B05', 'RLM|0x3B10']);
      final engine = autoGroups.firstWhere((g) => g.id == 'engine');
      // 0x0504 pelo DID; a temperatura entra pelo nome (DID fixo ausente aqui).
      expect(resolveGroup(pack, engine), ['PCM|0x0504', 'PCM|0x0305']);
    });

    test('grupos no pacote real do L319 (se presente)', () async {
      final pack = await Pack.load();
      final got = {for (final g in autoGroups) g.id: resolveGroup(pack, g)};
      expect(got['suspension'], [
        'RLM|0x3B03', 'RLM|0x3B04', 'RLM|0x3B05', 'RLM|0x3B06', 'RLM|0xD11A'
      ]);
      expect(got['engine'],
          ['PCM|0xF405', 'PCM|0xF40C', 'PCM|0x0322', 'PCM|0x0504']);
      expect(got['electrical'], ['PCM|0x1172', 'BCM|0x402A', 'BCM|0x4028']);
      expect(got['transmission'], ['TCM|0x1E69', 'TCM|0xD904']);
      // Todos decodificáveis (linear, com unidade): mostram número no carro.
      for (final keys in got.values) {
        for (final k in keys) {
          final i = k.indexOf('|');
          final p = pack.live[k.substring(0, i)]!
              .firstWhere((x) => x.did == k.substring(i + 1));
          expect(p.kind, 'linear', reason: k);
          expect(prettyUnit(p.unit), isNotEmpty, reason: k);
        }
      }
    }, skip: !File('assets/pack.json').existsSync());

    test('snapshot: falhas confirmadas primeiro, com descrição e sinais', () async {
      car.watchKeys(watchAuto, ['RLM|0x3B05']);
      await car.tickNow();
      final s = autoSnapshot(car, {'suspension': ['RLM|0x3B05']});
      final dtcs = (s['dtcs'] as List).cast<Map<String, dynamic>>();
      final firstHistory = dtcs.indexWhere((d) => d['confirmed'] == false);
      final lastConfirmed = dtcs.lastIndexWhere((d) => d['confirmed'] == true);
      expect(lastConfirmed, lessThan(firstHistory));
      expect(dtcs.firstWhere((d) => d['code'] == 'C1A13-08')['desc'],
          'Front left height sensor');
      expect((s['groups'] as Map)['phone'], contains('RLM|0x3B03'));
      final live = (s['live'] as List).cast<Map<String, dynamic>>();
      final sig = live.firstWhere((l) => l['key'] == 'RLM|0x3B05');
      expect(sig['history'], isNotEmpty);
      expect(sig['min'], isNotNull);
      expect(s['connected'], isTrue);
      expect(s['demo'], isTrue);
    });
  });
}
