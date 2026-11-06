import 'package:flutter/material.dart';
import 'car_state.dart';
import 'pack.dart';
import 'theme.dart';
import 'l10n.dart';

/// Limita a largura do conteúdo em telas largas.
class _Content extends StatelessWidget {
  final Widget child;
  const _Content({required this.child});
  @override
  Widget build(BuildContext context) => Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1000),
          child: child,
        ),
      );
}

Widget _sectionTitle(String t) => Padding(
      padding: const EdgeInsets.fromLTRB(2, 6, 2, 8),
      child: Text(t,
          style: TextStyle(
              fontSize: 12,
              fontWeight: FontWeight.w700,
              letterSpacing: 0.6,
              color: C.ink3)),
    );

Widget _chip(String t) => Container(
      padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 4),
      decoration: BoxDecoration(
        color: C.panel2,
        borderRadius: BorderRadius.circular(7),
        border: Border.all(color: C.line),
      ),
      child: Text(t, style: C.mono.copyWith(fontSize: 11.5, color: C.ink2)),
    );

// ───────────────────────────── Painel ─────────────────────────────

class PainelScreen extends StatelessWidget {
  final Pack pack;
  final CarState car;
  const PainelScreen({super.key, required this.pack, required this.car});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
        listenable: car, builder: (context, _) => _build(context));
  }

  Widget _build(BuildContext context) {
    final v = pack.vehicle;
    return _Content(
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  _sectionTitle(L.t('VEHICLE')),
                  Container(
                    decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(16),
                      gradient: const RadialGradient(
                        center: Alignment(0, -0.35),
                        radius: 1.1,
                        colors: [Color(0xFFFFFFFF), Color(0xFFDFE4EA)],
                      ),
                      border: Border.all(color: C.line),
                    ),
                    padding: const EdgeInsets.fromLTRB(10, 14, 10, 8),
                    child: Image.asset('assets/car/d4.jpg',
                        fit: BoxFit.contain, height: 180),
                  ),
                  const SizedBox(height: 14),
                  Wrap(spacing: 24, runSpacing: 10, children: [
                    _kv(L.t('Model'), 'Discovery 4'),
                    _kv(L.t('Platform'), v['model']?.toString() ?? '—'),
                    _kv(L.t('Year'), '${v['year_code']} · ${v['model_year']}'),
                    _kv(L.t('Engine'), '3.0 SDV6 diesel'),
                  ]),
                  const SizedBox(height: 14),
                  Wrap(spacing: 8, runSpacing: 8, children: [
                    _chip('HS-CAN 500k'),
                    _chip('MS-CAN 125k'),
                    _chip(car.connected
                        ? '${car.online.length}/${pack.modules.length} ${L.t('online')}'
                        : '${pack.modules.length} ${L.t('modules')}'),
                    _chip('Mongoose Pro JLR'),
                  ]),
                  const SizedBox(height: 12),
                  if (car.vin != null)
                    Row(children: [
                      Text('VIN  ',
                          style: TextStyle(fontSize: 11, color: C.ink3)),
                      Flexible(
                        child: Text(car.vin!,
                            style: C.mono.copyWith(
                                fontSize: 13, fontWeight: FontWeight.w600)),
                      ),
                    ])
                  else if (car.connectError != null)
                    Text('${L.t('Connection failed')}: ${car.connectError}',
                        style: TextStyle(fontSize: 11.5, color: C.crit))
                  else
                    Text(
                      L.t('VIN, active faults and sensor values are read from the car — tap Connect.'),
                      style: TextStyle(fontSize: 11.5, color: C.ink3),
                    ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),
          _sectionTitle('${L.t('MODULES')} (${pack.modules.length})'),
          LayoutBuilder(builder: (context, c) {
            final cols = c.maxWidth > 680 ? 3 : (c.maxWidth > 440 ? 2 : 1);
            return GridView.count(
              crossAxisCount: cols,
              shrinkWrap: true,
              physics: const NeverScrollableScrollPhysics(),
              childAspectRatio: 2.6,
              mainAxisSpacing: 10,
              crossAxisSpacing: 10,
              children: [
                for (final m in pack.modules)
                  _moduleCard(m, car.connected, car.online.contains(m.acronym))
              ],
            );
          }),
          const SizedBox(height: 24),
        ],
      ),
    );
  }

  Widget _kv(String k, String val) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(k, style: TextStyle(fontSize: 11, color: C.ink3)),
          Text(val,
              style: const TextStyle(fontWeight: FontWeight.w600, fontSize: 14)),
        ],
      );

  Widget _moduleCard(Module m, bool connected, bool online) => Card(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Row(children: [
                if (connected) ...[
                  Container(
                    width: 8,
                    height: 8,
                    margin: const EdgeInsets.only(right: 7),
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      color: online ? C.good : C.ink3.withValues(alpha: 0.4),
                    ),
                  ),
                ],
                Text(m.acronym,
                    style: const TextStyle(
                        fontWeight: FontWeight.w700, fontSize: 15)),
                const Spacer(),
                Text('${m.txHex}/${m.rxHex}',
                    style: C.mono.copyWith(fontSize: 10.5, color: C.ink3)),
              ]),
              const SizedBox(height: 2),
              Text(m.name ?? '',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(fontSize: 11.5, color: C.ink2)),
            ],
          ),
        ),
      );
}

// ───────────────────────────── Ao vivo ─────────────────────────────

class LiveScreen extends StatefulWidget {
  final Pack pack;
  final CarState car;
  const LiveScreen({super.key, required this.pack, required this.car});
  @override
  State<LiveScreen> createState() => _LiveScreenState();
}

class _LiveScreenState extends State<LiveScreen> {
  late String _module = widget.pack.liveModules.contains('PCM')
      ? 'PCM'
      : widget.pack.liveModules.first;
  String _query = '';

  @override
  Widget build(BuildContext context) => ListenableBuilder(
      listenable: widget.car, builder: (context, _) => _body(context));

  Widget _body(BuildContext context) {
    final mods = widget.pack.liveModules;
    final all = widget.pack.live[_module] ?? const [];
    final q = _query.toLowerCase();
    final params = q.isEmpty
        ? all
        : all
            .where((p) =>
                p.name.toLowerCase().contains(q) ||
                p.did.toLowerCase().contains(q))
            .toList();

    return _Content(
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  widget.car.connected
                      ? L.liveHintConnected(_total())
                      : L.liveHintOffline(_total(), mods.length),
                  style: TextStyle(fontSize: 11.5, color: C.ink3),
                ),
                const SizedBox(height: 10),
                Row(children: [
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    decoration: BoxDecoration(
                      color: C.panel2,
                      borderRadius: BorderRadius.circular(9),
                      border: Border.all(color: C.line),
                    ),
                    child: DropdownButton<String>(
                      value: _module,
                      underline: const SizedBox(),
                      dropdownColor: C.panel2,
                      style: TextStyle(color: C.ink, fontSize: 14),
                      items: [
                        for (final m in mods)
                          DropdownMenuItem(
                            value: m,
                            child: Text('$m  '
                                '(${widget.pack.live[m]!.length})'),
                          ),
                      ],
                      onChanged: (m) => setState(() => _module = m!),
                    ),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: TextField(
                      onChanged: (v) => setState(() => _query = v),
                      style: const TextStyle(fontSize: 14),
                      decoration: InputDecoration(
                        isDense: true,
                        hintText: L.t('Filter signals…'),
                        hintStyle: TextStyle(color: C.ink3),
                        filled: true,
                        fillColor: C.panel2,
                        prefixIcon: const Icon(Icons.search, size: 18),
                        border: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(9),
                          borderSide: BorderSide(color: C.line),
                        ),
                        enabledBorder: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(9),
                          borderSide: BorderSide(color: C.line),
                        ),
                      ),
                    ),
                  ),
                ]),
              ],
            ),
          ),
          Expanded(
            child: ListView.separated(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
              itemCount: params.length,
              separatorBuilder: (_, _) =>
                  Divider(height: 1, color: C.line),
              itemBuilder: (_, i) => _paramRow(params[i]),
            ),
          ),
        ],
      ),
    );
  }

  int _total() =>
      widget.pack.live.values.fold(0, (a, b) => a + b.length);

  Widget _paramRow(LiveParam p) {
    final car = widget.car;
    final key = '$_module|${p.did}';
    final watched = car.isWatched(_module, p);
    final sample = car.live[key];
    final display = sample?.display ?? (watched ? '…' : '—');
    return InkWell(
      onTap: car.connected ? () => car.toggleWatch(_module, p) : null,
      child: Container(
        padding: const EdgeInsets.symmetric(vertical: 11, horizontal: 4),
        decoration: BoxDecoration(
          border: Border(
            left: BorderSide(
                color: watched ? C.accent : Colors.transparent, width: 2),
          ),
        ),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(p.name, style: const TextStyle(fontSize: 13.5)),
                  const SizedBox(height: 2),
                  Text('$_module · ${p.did} · ${p.scaleLabel}',
                      style: C.mono.copyWith(fontSize: 11, color: C.ink3)),
                ],
              ),
            ),
            Text(display,
                style: TextStyle(
                    color: watched ? C.accent : C.ink3,
                    fontSize: 15,
                    fontWeight: FontWeight.w600)),
          ],
        ),
      ),
    );
  }
}

// ───────────────────────────── Falhas ─────────────────────────────

class FalhasScreen extends StatefulWidget {
  final Pack pack;
  final CarState car;
  final String? initialQuery;
  const FalhasScreen(
      {super.key, required this.pack, required this.car, this.initialQuery});
  @override
  State<FalhasScreen> createState() => _FalhasScreenState();
}

class _FalhasScreenState extends State<FalhasScreen> {
  late String _query = widget.initialQuery ?? '';
  late final _ctrl = TextEditingController(text: widget.initialQuery ?? '');

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
      listenable: widget.car, builder: (context, _) => _body(context));

  Widget _body(BuildContext context) {
    final car = widget.car;
    final q = _query.trim().toUpperCase();
    final codes = q.isEmpty
        ? const <String>[]
        : (widget.pack.dtcText.keys.where((c) => c.contains(q)).toList()
          ..sort())
            .take(60)
            .toList();

    return _Content(
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  car.connected
                      ? L.t('Active faults read from the car. Search a code for the full reference.')
                      : L.t('Fault reference: search a code for the description and the SDD help per module. Active ones come from the car.'),
                  style: TextStyle(fontSize: 11.5, color: C.ink3),
                ),
                const SizedBox(height: 10),
                TextField(
                  controller: _ctrl,
                  onChanged: (v) => setState(() => _query = v),
                  style: const TextStyle(fontSize: 14),
                  textCapitalization: TextCapitalization.characters,
                  decoration: InputDecoration(
                    isDense: true,
                    hintText: L.t('DTC code (e.g. C1A13, P0671)…'),
                    hintStyle: TextStyle(color: C.ink3),
                    filled: true,
                    fillColor: C.panel2,
                    prefixIcon: const Icon(Icons.search, size: 18),
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(9),
                      borderSide: BorderSide(color: C.line),
                    ),
                    enabledBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(9),
                      borderSide: BorderSide(color: C.line),
                    ),
                  ),
                ),
              ],
            ),
          ),
          Expanded(child: _content(car, q, codes)),
        ],
      ),
    );
  }

  Widget _content(CarState car, String q, List<String> codes) {
    if (q.isNotEmpty) {
      return ListView(
        padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
        children: [
          for (final c in codes) _dtcTile(c, expanded: codes.length == 1)
        ],
      );
    }
    if (car.connected) {
      final entries = <(String, String, int)>[]; // módulo, código, status
      car.activeDtcs.forEach((mod, list) {
        for (final (code, status) in list) {
          entries.add((mod, code, status));
        }
      });
      // A leitura usa a máscara 0xFF (tudo que está na memória). Separa as
      // CONFIRMADAS (bit confirmedDTC 0x08) do histórico armazenado, e ordena
      // confirmadas primeiro — é o que de fato interessa como "falha atual".
      entries.sort((a, b) {
        final ca = (a.$3 & 0x08) != 0, cb = (b.$3 & 0x08) != 0;
        if (ca != cb) return ca ? -1 : 1;
        return a.$2.compareTo(b.$2);
      });
      final nConf = entries.where((e) => (e.$3 & 0x08) != 0).length;
      return ListView(
        padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
        children: [
          Padding(
            padding: const EdgeInsets.only(bottom: 8),
            child: Row(children: [
              Expanded(
                child: Text(
                  car.dtcsScanning
                      ? L.readingFaultsProgress(car.dtcScanDone, car.dtcScanTotal)
                      : entries.isEmpty
                          ? L.t('No faults in memory')
                          : L.confirmedSummary(nConf, entries.length),
                  style: const TextStyle(
                      fontWeight: FontWeight.w700, fontSize: 13),
                ),
              ),
              if (car.dtcsScanning || car.clearing)
                Padding(
                  padding: const EdgeInsets.only(right: 10),
                  child: SizedBox(
                      width: 14,
                      height: 14,
                      child: CircularProgressIndicator(
                          strokeWidth: 2, color: C.accent)),
                ),
              TextButton.icon(
                onPressed: (car.reading || car.dtcsScanning || car.clearing)
                    ? null
                    : () => car.refresh(),
                icon: const Icon(Icons.refresh, size: 16),
                label: Text(L.t('Reread')),
              ),
              TextButton.icon(
                onPressed: (entries.isEmpty ||
                        car.reading ||
                        car.dtcsScanning ||
                        car.clearing)
                    ? null
                    : () => _confirmClear(car),
                style: TextButton.styleFrom(foregroundColor: C.crit),
                icon: const Icon(Icons.delete_sweep_outlined, size: 16),
                label: Text(L.t('Clear')),
              ),
            ]),
          ),
          if (entries.isEmpty)
            Padding(
              padding: const EdgeInsets.only(top: 20),
              child: Center(
                  child: Text(
                      car.dtcsScanning
                          ? L.t('Reading faults…')
                          : L.t('No faults in memory.'),
                      style: TextStyle(color: C.ink3))),
            ),
          for (final e in entries) _activeTile(e.$1, e.$2, e.$3),
        ],
      );
    }
    return Center(
        child: Text(L.t('Type a code to search'),
            style: TextStyle(color: C.ink3)));
  }

  /// Confirma e apaga as falhas (UDS 0x14) de todos os módulos; mostra o resumo.
  Future<void> _confirmClear(CarState car) async {
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: C.panel,
        title: Text(L.t('Clear faults?')),
        content: Text(
          L.clearConfirm(car.online.length),
          style: TextStyle(color: C.ink2),
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(ctx, false),
              child: Text(L.t('Cancel'))),
          FilledButton.icon(
            style: FilledButton.styleFrom(
                backgroundColor: C.crit, foregroundColor: Colors.white),
            onPressed: () => Navigator.pop(ctx, true),
            icon: const Icon(Icons.delete_sweep_outlined, size: 18),
            label: Text(L.t('Clear')),
          ),
        ],
      ),
    );
    if (ok != true || !mounted) return;
    final messenger = ScaffoldMessenger.of(context);
    final r = await car.clearDtcs();
    messenger.showSnackBar(SnackBar(
      content: Text(r.error ??
          L.clearResult(r.cleared.length, r.refused.length,
              r.noResponse.length, r.remaining)),
      duration: const Duration(seconds: 6),
    ));
  }

  /// Tile de uma falha ativa (módulo + código + status), com o help do módulo.
  Widget _activeTile(String module, String code, int status) {
    final base = code.split('-').first;
    final desc = widget.pack.dtcText[base] ?? '';
    final help = widget.pack.dtcHelp[base]?[module];
    final confirmed = (status & 0x08) != 0; // confirmedDTC
    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: Theme(
        data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
        child: ExpansionTile(
          tilePadding: const EdgeInsets.symmetric(horizontal: 14),
          title: Row(children: [
            Text(code,
                style: C.mono.copyWith(
                    fontWeight: FontWeight.w700,
                    fontSize: 14,
                    color: confirmed ? C.crit : C.ink2)),
            const SizedBox(width: 12),
            Expanded(
                child: Text(desc,
                    style: TextStyle(
                        fontSize: 12.5,
                        color: confirmed ? C.ink2 : C.ink3))),
          ]),
          subtitle: Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Wrap(spacing: 6, children: [
              _chip(module),
              _chip(confirmed ? L.t('confirmed') : L.t('history')),
              _chip('status 0x${status.toRadixString(16).padLeft(2, '0')}'),
            ]),
          ),
          childrenPadding: const EdgeInsets.fromLTRB(14, 0, 14, 12),
          children: help == null
              ? [
                  Align(
                    alignment: Alignment.centerLeft,
                    child: Text(L.noHelpFor(base, module),
                        style: TextStyle(
                            fontSize: 12,
                            color: C.ink3,
                            fontStyle: FontStyle.italic)),
                  )
                ]
              : [_moduleHelp(module, help)],
        ),
      ),
    );
  }

  Widget _dtcTile(String code, {bool expanded = false}) {
    final desc = widget.pack.dtcText[code] ?? '';
    final help = widget.pack.dtcHelp[code] ?? const {};
    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: Theme(
        data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
        child: ExpansionTile(
          initiallyExpanded: expanded,
          tilePadding: const EdgeInsets.symmetric(horizontal: 14),
          title: Row(
            children: [
              Text(code,
                  style: C.mono.copyWith(
                      fontWeight: FontWeight.w700,
                      fontSize: 14,
                      color: C.accent)),
              const SizedBox(width: 12),
              Expanded(
                child: Text(desc,
                    style: TextStyle(fontSize: 12.5, color: C.ink2)),
              ),
            ],
          ),
          subtitle: help.isEmpty
              ? null
              : Padding(
                  padding: const EdgeInsets.only(top: 4),
                  child: Wrap(
                    spacing: 6,
                    children: [for (final m in help.keys) _chip(m)],
                  ),
                ),
          childrenPadding: const EdgeInsets.fromLTRB(14, 0, 14, 12),
          children: help.isEmpty
              ? [
                  Align(
                    alignment: Alignment.centerLeft,
                    child: Text(
                      L.t('No diagnostic help for this code in this pack.'),
                      style: TextStyle(
                          fontSize: 12, color: C.ink3, fontStyle: FontStyle.italic),
                    ),
                  )
                ]
              : [for (final e in help.entries) _moduleHelp(e.key, e.value)],
        ),
      ),
    );
  }

  Widget _moduleHelp(String module, DtcHelp h) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(top: 6, bottom: 2),
            child: Text(L.moduleLabel(module),
                style: C.mono.copyWith(
                    fontSize: 11, color: C.accent, fontWeight: FontWeight.w700)),
          ),
          for (final sec in h.sections) ...[
            Padding(
              padding: const EdgeInsets.only(top: 8, bottom: 1),
              child: Text(sec.key.toUpperCase(),
                  style: TextStyle(
                      fontSize: 10.5,
                      letterSpacing: 0.5,
                      color: C.ink3,
                      fontWeight: FontWeight.w700)),
            ),
            for (final item in sec.value)
              Padding(
                padding: const EdgeInsets.only(left: 2, top: 2),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('•  ', style: TextStyle(color: C.ink3)),
                    Expanded(
                      child: Text(item,
                          style: TextStyle(
                              fontSize: 12.5, color: C.ink2, height: 1.4)),
                    ),
                  ],
                ),
              ),
          ],
        ],
      );
}

// ───────────────────────────── Config (CCF) ─────────────────────────────

class ConfigScreen extends StatefulWidget {
  final Pack pack;
  final CarState car;
  final String? initialQuery;
  const ConfigScreen(
      {super.key, required this.pack, required this.car, this.initialQuery});
  @override
  State<ConfigScreen> createState() => _ConfigScreenState();
}

class _ConfigScreenState extends State<ConfigScreen> {
  bool _advanced = false;
  late String _query = widget.initialQuery ?? '';
  late final _ctrl = TextEditingController(text: widget.initialQuery ?? '');

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
      listenable: widget.car, builder: (context, _) => _body(context));

  Widget _body(BuildContext context) {
    final car = widget.car;
    final q = _query.trim().toLowerCase();
    final params = widget.pack.ccf.params.where((p) {
      if (!_advanced && !p.display && q.isEmpty) return false;
      if (q.isEmpty) return true;
      return p.label.toLowerCase().contains(q) ||
          p.groupLabel.toLowerCase().contains(q) ||
          p.name.toLowerCase().contains(q);
    }).toList();

    // agrupa por groupLabel preservando ordem
    final groups = <String, List<CcfParam>>{};
    for (final p in params) {
      groups.putIfAbsent(p.groupLabel, () => []).add(p);
    }

    return _Content(
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  car.ccfImage != null
                      ? L.t('Vehicle configuration (CCF) read from the BCM. View only; writing to the BCM depends on a mechanism not yet unlocked.')
                      : L.t('Vehicle configuration (CCF). Connect to read the current values. Writing is not available yet.'),
                  style: TextStyle(fontSize: 11.5, color: C.ink3),
                ),
                const SizedBox(height: 10),
                Row(children: [
                  Expanded(
                    child: TextField(
                      controller: _ctrl,
                      onChanged: (v) => setState(() => _query = v),
                      style: const TextStyle(fontSize: 14),
                      decoration: InputDecoration(
                        isDense: true,
                        hintText: L.t('Filter options (e.g. light, alarm)…'),
                        hintStyle: TextStyle(color: C.ink3),
                        filled: true,
                        fillColor: C.panel2,
                        prefixIcon: const Icon(Icons.search, size: 18),
                        border: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(9),
                          borderSide: BorderSide(color: C.line),
                        ),
                        enabledBorder: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(9),
                          borderSide: BorderSide(color: C.line),
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  Tooltip(
                    message: L.t('Show advanced options (hidden in SDD)'),
                    child: Row(children: [
                      Text(L.t('Advanced'),
                          style: TextStyle(fontSize: 12, color: C.ink2)),
                      Switch(
                        value: _advanced,
                        onChanged: (v) => setState(() => _advanced = v),
                      ),
                    ]),
                  ),
                ]),
              ],
            ),
          ),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
              children: [
                for (final e in groups.entries) ...[
                  Padding(
                    padding: const EdgeInsets.fromLTRB(2, 10, 2, 6),
                    child: Text(e.key.toUpperCase(),
                        style: TextStyle(
                            fontSize: 11,
                            fontWeight: FontWeight.w700,
                            letterSpacing: 0.5,
                            color: C.accent)),
                  ),
                  Card(
                    child: Column(
                      children: [
                        for (var i = 0; i < e.value.length; i++) ...[
                          if (i > 0) Divider(height: 1, color: C.line),
                          _ccfRow(car, e.value[i]),
                        ]
                      ],
                    ),
                  ),
                ],
                if (groups.isEmpty)
                  Padding(
                    padding: const EdgeInsets.only(top: 24),
                    child: Center(
                        child: Text(L.t('Nothing found.'),
                            style: TextStyle(color: C.ink3))),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _ccfRow(CarState car, CcfParam p) {
    final (_, current) = car.decodeCcf(p);
    final known = car.ccfImage != null && current != '—';
    return Theme(
      data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
      child: ExpansionTile(
        tilePadding: const EdgeInsets.symmetric(horizontal: 14),
        title: Text(p.label, style: const TextStyle(fontSize: 13.5)),
        subtitle: Text(p.name,
            style: C.mono.copyWith(fontSize: 10.5, color: C.ink3)),
        trailing: Text(
          current,
          style: TextStyle(
              color: known ? C.accent : C.ink3,
              fontSize: 12.5,
              fontWeight: FontWeight.w600),
        ),
        childrenPadding: const EdgeInsets.fromLTRB(14, 0, 14, 10),
        children: [
          for (final o in p.options)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 3),
              child: Row(children: [
                Icon(
                  known && current == o.label
                      ? Icons.radio_button_checked
                      : Icons.radio_button_unchecked,
                  size: 16,
                  color: known && current == o.label ? C.accent : C.ink3,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(o.label,
                      style: TextStyle(fontSize: 12.5, color: C.ink2)),
                ),
                Text('0x${o.value.toRadixString(16).padLeft(2, '0')}',
                    style: C.mono.copyWith(fontSize: 10.5, color: C.ink3)),
              ]),
            ),
          Padding(
            padding: const EdgeInsets.only(top: 6),
            child: Text(
              L.ccfMeta(p.block, p.byteStart, p.mask.toRadixString(16), p.editable),
              style: TextStyle(fontSize: 10.5, color: C.ink3),
            ),
          ),
        ],
      ),
    );
  }
}
