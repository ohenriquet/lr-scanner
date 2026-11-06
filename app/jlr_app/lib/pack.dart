import 'l10n.dart';
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
  final Ccf ccf;

  Pack({
    required this.vehicle,
    required this.modules,
    required this.live,
    required this.dtcText,
    required this.dtcHelp,
    required this.ccf,
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
      ccf: Ccf.fromJson(j['ccf'] as Map<String, dynamic>?),
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

/// Unidade como o usuário lê (o pacote traz `degC`, `pct`, `int`...).
String prettyUnit(String? unit) => switch (unit) {
      null || 'int' => '',
      'degC' => '°C',
      'pct' => '%',
      _ => unit,
    };

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
      final u = prettyUnit(unit);
      final m = mult ?? 1;
      final o = offset ?? 0;
      final mm = (m == 1) ? '' : '× $m ';
      final oo = (o == 0) ? '' : '(offset $o) ';
      return '$mm$oo$u'.trim();
    }
    return prettyUnit(unit);
  }
}

/// Configuração do veículo (CCF): offsets de bloco + parâmetros editáveis.
class Ccf {
  final Map<String, int> blockBase; // block_id -> offset na imagem do DID
  final List<CcfParam> params;
  const Ccf({required this.blockBase, required this.params});

  factory Ccf.fromJson(Map<String, dynamic>? j) {
    if (j == null) return const Ccf(blockBase: {}, params: []);
    final bb = <String, int>{};
    (j['block_base'] as Map<String, dynamic>? ?? const {})
        .forEach((k, v) => bb[k] = (v as num).toInt());
    final ps = (j['params'] as List? ?? const [])
        .map((p) => CcfParam.fromJson(p as Map<String, dynamic>))
        .toList();
    return Ccf(blockBase: bb, params: ps);
  }
}

class CcfParam {
  final String block;
  final String? grp;
  final String name;
  final String? title;
  final int byteStart;
  final int mask;
  final bool display;
  final bool editable;
  final List<CcfOption> options;

  CcfParam({
    required this.block,
    this.grp,
    required this.name,
    this.title,
    required this.byteStart,
    required this.mask,
    required this.display,
    required this.editable,
    required this.options,
  });

  factory CcfParam.fromJson(Map<String, dynamic> j) => CcfParam(
        block: j['block'].toString(),
        grp: j['grp'] as String?,
        name: j['name'] as String,
        title: j['title'] as String?,
        byteStart: (j['byte_start'] as num).toInt(),
        mask: (j['mask'] as num?)?.toInt() ?? 0xFF,
        display: j['display'] as bool? ?? false,
        editable: j['editable'] as bool? ?? false,
        options: (j['options'] as List? ?? const [])
            .map((o) => CcfOption.fromJson(o as Map<String, dynamic>))
            .toList(),
      );

  String get label => (title != null && title!.isNotEmpty) ? title! : name;

  /// Rótulo amigável do grupo (ex.: "Exterior lights").
  String get groupLabel {
    var g = (grp ?? '').replaceFirst('GROUP_CCF_', '').replaceFirst('GROUP_', '');
    g = g.replaceAll('EUCD_', '').replaceAll('_', ' ').trim().toLowerCase();
    if (g.isEmpty) return 'Outros';
    return g[0].toUpperCase() + g.substring(1);
  }
}

class CcfOption {
  final int value;
  final String label;
  const CcfOption(this.value, this.label);
  factory CcfOption.fromJson(Map<String, dynamic> j) => CcfOption(
        (j['value'] as num).toInt(),
        (j['label'] as String?) ?? (j['name'] as String?) ?? '0x${j['value']}',
      );
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
        MapEntry(L.t('Additional description'), suppDesc),
        MapEntry(L.t('Symptoms'), symptoms),
        MapEntry(L.t('Probable causes'), causes),
        MapEntry(L.t('Recommended actions'), actions),
      ].where((e) => e.value.isNotEmpty).toList();
}
