import 'package:flutter/material.dart';
import 'car_link.dart';
import 'pack.dart';
import 'screens.dart';
import 'theme.dart';

void main() => runApp(const JlrApp());

class JlrApp extends StatelessWidget {
  const JlrApp({super.key});

  @override
  Widget build(BuildContext context) {
    final scheme = ColorScheme.fromSeed(
      seedColor: C.accent,
      brightness: Brightness.dark,
    ).copyWith(
      surface: C.panel,
      surfaceContainerHighest: C.panel2,
      primary: C.accent,
      onPrimary: const Color(0xFF07201E),
      outline: C.line,
    );
    return MaterialApp(
      title: 'JLR OpenDiag',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        colorScheme: scheme,
        scaffoldBackgroundColor: C.bg,
        cardTheme: const CardThemeData(
          color: C.panel,
          elevation: 0,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.all(Radius.circular(14)),
            side: BorderSide(color: C.line),
          ),
        ),
        textTheme: Typography.whiteMountainView.apply(
          bodyColor: C.ink,
          displayColor: C.ink,
        ),
      ),
      home: const HomeShell(),
    );
  }
}

class HomeShell extends StatefulWidget {
  const HomeShell({super.key});
  @override
  State<HomeShell> createState() => _HomeShellState();
}

class _HomeShellState extends State<HomeShell> {
  late final Future<Pack> _pack = Pack.load();
  int _tab = 0;

  // Deep-link opcional por URL: ?tab=0|1|2 e ?q=<busca de falha>.
  String? _initialQuery;

  @override
  void initState() {
    super.initState();
    final q = Uri.base.queryParameters;
    final t = int.tryParse(q['tab'] ?? '');
    if (t != null && t >= 0 && t <= 2) _tab = t;
    _initialQuery = q['q'];
  }

  static const _dests = [
    (Icons.dashboard_outlined, Icons.dashboard, 'Painel'),
    (Icons.show_chart_outlined, Icons.show_chart, 'Ao vivo'),
    (Icons.error_outline, Icons.error, 'Falhas'),
  ];

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<Pack>(
      future: _pack,
      builder: (context, snap) {
        if (snap.hasError) {
          return Scaffold(
            body: Center(
              child: Padding(
                padding: const EdgeInsets.all(24),
                child: Text('Erro ao carregar o pacote:\n${snap.error}',
                    textAlign: TextAlign.center,
                    style: const TextStyle(color: C.crit)),
              ),
            ),
          );
        }
        if (!snap.hasData) {
          return const Scaffold(
            body: Center(child: CircularProgressIndicator(color: C.accent)),
          );
        }
        final pack = snap.data!;
        final pages = [
          PainelScreen(pack: pack),
          LiveScreen(pack: pack),
          FalhasScreen(pack: pack, initialQuery: _initialQuery),
        ];
        final wide = MediaQuery.sizeOf(context).width >= 820;

        final body = Row(
          children: [
            if (wide)
              NavigationRail(
                backgroundColor: C.panel,
                selectedIndex: _tab,
                onDestinationSelected: (i) => setState(() => _tab = i),
                labelType: NavigationRailLabelType.all,
                indicatorColor: C.accentDim,
                destinations: [
                  for (final d in _dests)
                    NavigationRailDestination(
                      icon: Icon(d.$1),
                      selectedIcon: Icon(d.$2, color: C.accent),
                      label: Text(d.$3),
                    ),
                ],
              ),
            Expanded(child: pages[_tab]),
          ],
        );

        return Scaffold(
          appBar: AppBar(
            backgroundColor: C.panel,
            surfaceTintColor: Colors.transparent,
            title: Row(
              children: [
                const Icon(Icons.monitor_heart_outlined, color: C.accent, size: 20),
                const SizedBox(width: 8),
                const Text('JLR OpenDiag',
                    style: TextStyle(fontWeight: FontWeight.w700, fontSize: 16)),
                const SizedBox(width: 10),
                Text('Discovery 4 · L319',
                    style: TextStyle(color: C.ink2, fontSize: 12)),
              ],
            ),
            actions: [
              Padding(
                padding: const EdgeInsets.only(right: 12),
                child: OutlinedButton.icon(
                  onPressed: () => _notConnected(context),
                  icon: const Icon(Icons.usb, size: 16),
                  label: const Text('Conectar'),
                  style: OutlinedButton.styleFrom(
                    foregroundColor: C.ink2,
                    side: const BorderSide(color: C.line),
                  ),
                ),
              ),
            ],
            bottom: const PreferredSize(
              preferredSize: Size.fromHeight(1),
              child: Divider(height: 1, color: C.line),
            ),
          ),
          body: SafeArea(child: body),
          bottomNavigationBar: wide
              ? null
              : NavigationBar(
                  backgroundColor: C.panel,
                  indicatorColor: C.accentDim,
                  selectedIndex: _tab,
                  onDestinationSelected: (i) => setState(() => _tab = i),
                  destinations: [
                    for (final d in _dests)
                      NavigationDestination(
                        icon: Icon(d.$1),
                        selectedIcon: Icon(d.$2, color: C.accent),
                        label: d.$3,
                      ),
                  ],
                ),
        );
      },
    );
  }

  void _notConnected(BuildContext context) {
    final native = CarLink.available;
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        backgroundColor: C.panel,
        title: const Text('Conexão com o carro'),
        content: Text(
          native
              ? 'Ponte nativa disponível neste alvo. Informe a porta do Mongoose '
                  '(ex.: COM5) para conectar e ler VIN, módulos, falhas ativas e '
                  'os valores ao vivo — fluxo em construção.'
              : 'Esta é a versão web (sem acesso a hardware). A leitura do carro '
                  '(VIN, falhas ativas, valores de sensor, CCF atual) exige o app '
                  'nativo (Windows/macOS/Android), que fala com o Mongoose pela '
                  'ponte FFI.\n\nAqui você vê os dados do pacote do veículo: '
                  'módulos, catálogo de live data e a referência de falhas com o '
                  'help do SDD por módulo.',
          style: const TextStyle(color: C.ink2),
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Entendi')),
        ],
      ),
    );
  }
}
