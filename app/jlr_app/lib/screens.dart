import 'package:flutter/material.dart';
import 'pack.dart';
import 'theme.dart';

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
          style: const TextStyle(
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
  const PainelScreen({super.key, required this.pack});

  @override
  Widget build(BuildContext context) {
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
                  _sectionTitle('VEÍCULO'),
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
                    _kv('Modelo', 'Discovery 4'),
                    _kv('Plataforma', v['model']?.toString() ?? '—'),
                    _kv('Ano', '${v['year_code']} · ${v['model_year']}'),
                    _kv('Motor', '3.0 SDV6 diesel'),
                  ]),
                  const SizedBox(height: 14),
                  Wrap(spacing: 8, runSpacing: 8, children: [
                    _chip('HS-CAN 500k'),
                    _chip('MS-CAN 125k'),
                    _chip('${pack.modules.length} módulos'),
                    _chip('Mongoose Pro JLR'),
                  ]),
                  const SizedBox(height: 12),
                  Text(
                    'VIN, tensão da bateria e odômetro são leituras do carro — '
                    'conecte o adaptador para lê-las.',
                    style: TextStyle(fontSize: 11.5, color: C.ink3),
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),
          _sectionTitle('MÓDULOS (${pack.modules.length})'),
          LayoutBuilder(builder: (context, c) {
            final cols = c.maxWidth > 680 ? 3 : (c.maxWidth > 440 ? 2 : 1);
            return GridView.count(
              crossAxisCount: cols,
              shrinkWrap: true,
              physics: const NeverScrollableScrollPhysics(),
              childAspectRatio: 2.6,
              mainAxisSpacing: 10,
              crossAxisSpacing: 10,
              children: [for (final m in pack.modules) _moduleCard(m)],
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

  Widget _moduleCard(Module m) => Card(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Row(children: [
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
  const LiveScreen({super.key, required this.pack});
  @override
  State<LiveScreen> createState() => _LiveScreenState();
}

class _LiveScreenState extends State<LiveScreen> {
  late String _module = widget.pack.liveModules.contains('PCM')
      ? 'PCM'
      : widget.pack.liveModules.first;
  String _query = '';

  @override
  Widget build(BuildContext context) {
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
                  'Catálogo de live data do pacote — ${_total()} parâmetros em '
                  '${mods.length} módulos. Valores ao vivo exigem o carro.',
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
                      style: const TextStyle(color: C.ink, fontSize: 14),
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
                        hintText: 'Filtrar sinais…',
                        hintStyle: TextStyle(color: C.ink3),
                        filled: true,
                        fillColor: C.panel2,
                        prefixIcon: const Icon(Icons.search, size: 18),
                        border: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(9),
                          borderSide: const BorderSide(color: C.line),
                        ),
                        enabledBorder: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(9),
                          borderSide: const BorderSide(color: C.line),
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
                  const Divider(height: 1, color: C.line),
              itemBuilder: (_, i) => _paramRow(params[i]),
            ),
          ),
        ],
      ),
    );
  }

  int _total() =>
      widget.pack.live.values.fold(0, (a, b) => a + b.length);

  Widget _paramRow(LiveParam p) => Padding(
        padding: const EdgeInsets.symmetric(vertical: 11),
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
            Text('—',
                style: TextStyle(
                    color: C.ink3, fontSize: 15, fontWeight: FontWeight.w600)),
          ],
        ),
      );
}

// ───────────────────────────── Falhas ─────────────────────────────

class FalhasScreen extends StatefulWidget {
  final Pack pack;
  final String? initialQuery;
  const FalhasScreen({super.key, required this.pack, this.initialQuery});
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
  Widget build(BuildContext context) {
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
                  'Referência de falhas: busque um código para ver a descrição e '
                  'o help do SDD por módulo. As falhas ativas vêm do carro.',
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
                    hintText: 'Código do DTC (ex.: C1A13, P0671)…',
                    hintStyle: TextStyle(color: C.ink3),
                    filled: true,
                    fillColor: C.panel2,
                    prefixIcon: const Icon(Icons.search, size: 18),
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(9),
                      borderSide: const BorderSide(color: C.line),
                    ),
                    enabledBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(9),
                      borderSide: const BorderSide(color: C.line),
                    ),
                  ),
                ),
              ],
            ),
          ),
          Expanded(
            child: q.isEmpty
                ? Center(
                    child: Text('Digite um código para buscar',
                        style: TextStyle(color: C.ink3)))
                : ListView(
                    padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
                    children: [
                      for (final c in codes)
                        _dtcTile(c, expanded: codes.length == 1)
                    ],
                  ),
          ),
        ],
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
                      'Sem help de diagnóstico para este código neste pacote.',
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
            child: Text('módulo $module',
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
