import 'dart:convert';
import 'package:flutter/services.dart' show rootBundle;

/// Pacote do veículo, exportado pelo núcleo Rust (`sdd-import pack-export`).
///
/// No app real, estes dados virão do pacote SQLite via a ponte nativa (FFI);
/// aqui (web) vêm de um JSON embutido. As leituras ao vivo (valores de sensor,
/// falhas ativas, CCF atual) dependem do carro — virão pela ponte.
class Pack {
  final Map<String, dynamic> vehicle;
  final List<Module> modules;
  final Map<String, List<LiveParam>> live; // módulo -> parâmetros
  final Map<String, String> dtcText; // código -> descrição
  final Map<String, Map<String, DtcHelp>> dtcHelp; // código -> módulo -> help

  Pack({
    required this.vehicle,
    required this.modules,
    required this.live,
    required this.dtcText,
    required this.dtcHelp,
  });

  static Future<Pack> load() async {
    final raw = await rootBundle.loadString('assets/pack.json');
    final j = jsonDecode(raw) as Map<String, dynamic>;

    final modules = (j['modules'] as List)
        .map((m) => Module.fromJson(m as Map<String, dynamic>))
        .toList();

    final live = <String, List<LiveParam>>{};
    (j['live'] as Map<String, dynamic>).forEach((mod, list) {
      live[mod] = (list as List)
          .map((p) => LiveParam.fromJson(p as Map<String, dynamic>))
          .toList();
    });

    final dtcText = <String, String>{};
    (j['dtc_text'] as Map<String, dynamic>).forEach((k, v) {
      dtcText[k] = v as String;
    });

    final dtcHelp = <String, Map<String, DtcHelp>>{};
    (j['dtc_help'] as Map<String, dynamic>).forEach((code, byMod) {
      final mm = <String, DtcHelp>{};
      (byMod as Map<String, dynamic>).forEach((mod, secs) {
        mm[mod] = DtcHelp.fromJson(secs as Map<String, dynamic>);
      });
      dtcHelp[code] = mm;
    });

    return Pack(
      vehicle: j['vehicle'] as Map<String, dynamic>,
      modules: modules,
      live: live,
      dtcText: dtcText,
      dtcHelp: dtcHelp,
    );
  }

  /// Módulos (siglas) que têm live data no catálogo, em ordem.
  List<String> get liveModules => live.keys.toList()..sort();
}

class Module {
  final String acronym;
  final String? name;
  final int? tx;
  final int? rx;
  final String? network;
  final String? fitment;

  Module({
    required this.acronym,
    this.name,
    this.tx,
    this.rx,
    this.network,
    this.fitment,
  });

  factory Module.fromJson(Map<String, dynamic> j) => Module(
        acronym: j['acronym'] as String,
        name: j['name'] as String?,
        tx: j['tx'] as int?,
        rx: j['rx'] as int?,
        network: j['network'] as String?,
        fitment: j['fitment'] as String?,
      );

  String get txHex => tx == null ? '—' : '0x${tx!.toRadixString(16).toUpperCase()}';
  String get rxHex => rx == null ? '—' : '0x${rx!.toRadixString(16).toUpperCase()}';
}

class LiveParam {
  final String did; // "0x3B03"
  final String name;
  final String? unit;
  final String? kind; // linear | enum | null
  final double? mult;
  final double? offset;
  final bool? offsetFirst;
  final bool? bit;
  final int? lsb;
  final int? msb;
  final String? source; // fmt | mdl

  LiveParam({
    required this.did,
    required this.name,
    this.unit,
    this.kind,
    this.mult,
    this.offset,
    this.offsetFirst,
    this.bit,
    this.lsb,
    this.msb,
    this.source,
  });

  factory LiveParam.fromJson(Map<String, dynamic> j) => LiveParam(
        did: j['did'] as String,
        name: j['name'] as String? ?? '',
        unit: j['unit'] as String?,
        kind: j['kind'] as String?,
        mult: (j['mult'] as num?)?.toDouble(),
        offset: (j['offset'] as num?)?.toDouble(),
        offsetFirst: j['offset_first'] as bool?,
        bit: j['bit'] as bool?,
        lsb: j['lsb'] as int?,
        msb: j['msb'] as int?,
        source: j['source'] as String?,
      );

  /// Descrição legível da escala (ex.: "× 0.001 m", "enum", "cru").
  String get scaleLabel {
    if (source == 'mdl') return 'valor cru';
    if (kind == 'enum') return 'enum';
    if (kind == 'linear') {
      final u = unit ?? '';
      final m = mult ?? 1;
      final o = offset ?? 0;
      final mm = (m == 1) ? '' : '× $m ';
      final oo = (o == 0) ? '' : '(offset $o) ';
      return '$mm$oo$u'.trim();
    }
    return unit ?? '';
  }
}

class DtcHelp {
  final List<String> suppDesc;
  final List<String> symptoms;
  final List<String> causes;
  final List<String> actions;

  DtcHelp({
    required this.suppDesc,
    required this.symptoms,
    required this.causes,
    required this.actions,
  });

  factory DtcHelp.fromJson(Map<String, dynamic> j) {
    List<String> g(String k) =>
        (j[k] as List?)?.map((e) => e as String).toList() ?? const [];
    return DtcHelp(
      suppDesc: g('supp_desc'),
      symptoms: g('symptoms'),
      causes: g('causes'),
      actions: g('actions'),
    );
  }

  bool get isEmpty =>
      suppDesc.isEmpty && symptoms.isEmpty && causes.isEmpty && actions.isEmpty;

  /// Seções não vazias como (título, itens).
  List<MapEntry<String, List<String>>> get sections => [
        MapEntry('Descrição complementar', suppDesc),
        MapEntry('Sintomas', symptoms),
        MapEntry('Causas prováveis', causes),
        MapEntry('Ações recomendadas', actions),
      ].where((e) => e.value.isNotEmpty).toList();
}
