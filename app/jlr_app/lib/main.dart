import 'package:flutter/material.dart';
import 'car_link.dart';
import 'car_state.dart';
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
  CarState? _car;
  int _tab = 0;

  // Deep-link opcional por URL: ?tab=0|1|2, ?q=<busca> e ?demo=1 (pré-visualização).
  String? _initialQuery;
  bool _demo = false;
  int _auto = 0; // 0=nada, 1=conecta+observa, 2=conecta sem observar

  @override
  void initState() {
    super.initState();
    final q = Uri.base.queryParameters;
    final t = int.tryParse(q['tab'] ?? '');
    if (t != null && t >= 0 && t <= 3) _tab = t;
    _initialQuery = q['q'];
    _demo = q['demo'] == '1';
    _auto = int.tryParse(q['auto'] ?? '0') ?? 0;
  }

  static const _dests = [
    (Icons.dashboard_outlined, Icons.dashboard, 'Painel'),
    (Icons.show_chart_outlined, Icons.show_chart, 'Ao vivo'),
    (Icons.error_outline, Icons.error, 'Falhas'),
    (Icons.tune_outlined, Icons.tune, 'Config'),
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
        final car = _car ??= CarState(pack);
        if (_demo && !car.demo && !car.connected) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (mounted) car.loadDemo();
          });
        }
        if (_auto > 0 && !car.connected && !car.connecting) {
          final watch = _auto == 1; // auto=2 conecta sem observar (poll parado)
          _auto = 0; // dispara uma vez
          WidgetsBinding.instance.addPostFrameCallback((_) async {
            if (!mounted) return;
            await car.connect('COM5', 500000);
            if (!mounted || !watch) return;
            for (final w in const [
              ('PCM', '0x0504'), // fluxo de ar
              ('PCM', '0x0301'), // pressão coletor (tensão)
              ('RLM', '0x3B03'), // altura diant. esq.
              ('RLM', '0x3B04'), // altura diant. dir.
            ]) {
              car.watchByDid(w.$1, w.$2);
            }
          });
        }
        final pages = [
          PainelScreen(pack: pack, car: car),
          LiveScreen(pack: pack, car: car),
          FalhasScreen(pack: pack, car: car, initialQuery: _initialQuery),
          ConfigScreen(pack: pack, car: car, initialQuery: _initialQuery),
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
                child: ListenableBuilder(
                  listenable: car,
                  builder: (context, _) {
                    if (car.connecting || car.reading) {
                      return const Padding(
                        padding: EdgeInsets.symmetric(horizontal: 14),
                        child: SizedBox(
                            width: 18,
                            height: 18,
                            child: CircularProgressIndicator(
                                strokeWidth: 2, color: C.accent)),
                      );
                    }
                    if (car.connected) {
                      return OutlinedButton.icon(
                        onPressed: () => car.disconnect(),
                        icon: const Icon(Icons.link_off, size: 16),
                        label: const Text('Desconectar'),
                        style: OutlinedButton.styleFrom(
                          foregroundColor: C.good,
                          side: const BorderSide(color: C.line),
                        ),
                      );
                    }
                    return OutlinedButton.icon(
                      onPressed: () => _openConnect(context, car),
                      icon: const Icon(Icons.usb, size: 16),
                      label: const Text('Conectar'),
                      style: OutlinedButton.styleFrom(
                        foregroundColor: C.ink2,
                        side: const BorderSide(color: C.line),
                      ),
                    );
                  },
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

  @override
  void dispose() {
    _car?.dispose();
    super.dispose();
  }

  void _openConnect(BuildContext context, CarState car) {
    if (!CarLink.available) {
      showDialog(
        context: context,
        builder: (_) => AlertDialog(
          backgroundColor: C.panel,
          title: const Text('Conexão com o carro'),
          content: const Text(
            'Esta é a versão web (sem acesso a hardware). A leitura do carro '
            '(VIN, falhas ativas, valores de sensor) exige o app nativo '
            '(Windows/macOS/Android), que fala com o Mongoose pela ponte FFI.\n\n'
            'Aqui você vê os dados do pacote: módulos, catálogo de live data e a '
            'referência de falhas com o help do SDD por módulo.',
            style: TextStyle(color: C.ink2),
          ),
          actions: [
            TextButton(
                onPressed: () => Navigator.pop(context),
                child: const Text('Entendi')),
          ],
        ),
      );
      return;
    }
    final portCtrl = TextEditingController(text: 'COM5');
    final baudCtrl = TextEditingController(text: '500000');
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        backgroundColor: C.panel,
        title: const Text('Conectar ao Mongoose'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: portCtrl,
              decoration: const InputDecoration(
                  labelText: 'Porta (ex.: COM5, /dev/ttyACM0)'),
            ),
            const SizedBox(height: 8),
            TextField(
              controller: baudCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Baud (HS-CAN)'),
            ),
            const SizedBox(height: 10),
            const Text(
              'Lê VIN, varre os módulos e lê as falhas ativas. Ignição ligada, '
              'motor desligado. Nada é gravado.',
              style: TextStyle(fontSize: 11.5, color: C.ink3),
            ),
          ],
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Cancelar')),
          FilledButton(
            onPressed: () {
              final port = portCtrl.text.trim();
              final baud = int.tryParse(baudCtrl.text.trim()) ?? 500000;
              Navigator.pop(context);
              car.connect(port, baud);
            },
            child: const Text('Conectar'),
          ),
        ],
      ),
    );
  }
}
