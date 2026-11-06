import 'dart:async';
import 'package:flutter/material.dart';
import 'app_core.dart';
import 'auto_bridge.dart';
import 'car_link.dart';
import 'car_state.dart';
import 'pack.dart';
import 'screens.dart';
import 'theme.dart';
import 'l10n.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  // Android Auto: a mesma engine atende a tela do carro (ver auto_bridge.dart).
  unawaited(AutoBridge.init());
  runApp(const JlrApp());
}

/// Nome do app.
const appName = 'LR Scanner';

String _langName(AppLang l) => switch (l) {
      AppLang.en => 'English',
      AppLang.es => 'Español',
      AppLang.pt => 'Português',
    };

class JlrApp extends StatefulWidget {
  const JlrApp({super.key});
  @override
  State<JlrApp> createState() => _JlrAppState();
}

class _JlrAppState extends State<JlrApp> with WidgetsBindingObserver {
  // null = segue o tema do sistema; senão, override do usuário pelo toggle.
  Brightness? _override;
  // null = segue o idioma do sistema; senão, escolha do usuário no seletor.
  AppLang? _langOverride;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangePlatformBrightness() {
    if (_override == null) setState(() {}); // segue o SO ao vivo
  }

  @override
  void didChangeLocales(List<Locale>? locales) {
    if (_langOverride == null) setState(() {}); // segue o idioma do SO ao vivo
  }

  Brightness get _effective =>
      _override ??
      WidgetsBinding.instance.platformDispatcher.platformBrightness;

  void _toggleTheme() => setState(() =>
      _override = _effective == Brightness.dark ? Brightness.light : Brightness.dark);

  AppLang get _effectiveLang {
    if (_langOverride != null) return _langOverride!;
    return switch (
        WidgetsBinding.instance.platformDispatcher.locale.languageCode) {
      'es' => AppLang.es,
      'pt' => AppLang.pt,
      _ => AppLang.en,
    };
  }

  void _setLang(AppLang l) => setState(() => _langOverride = l);

  @override
  Widget build(BuildContext context) {
    final b = _effective;
    C.brightness = b; // a paleta global (C.*) segue o tema ativo
    L.lang = _effectiveLang; // o idioma global (L.*) segue a escolha/sistema
    final scheme = ColorScheme.fromSeed(seedColor: C.accent, brightness: b).copyWith(
      surface: C.panel,
      surfaceContainerHighest: C.panel2,
      primary: C.accent,
      onPrimary: b == Brightness.dark ? const Color(0xFF07201E) : Colors.white,
      outline: C.line,
    );
    return MaterialApp(
      title: appName,
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        colorScheme: scheme,
        scaffoldBackgroundColor: C.bg,
        cardTheme: CardThemeData(
          color: C.panel,
          elevation: 0,
          shape: RoundedRectangleBorder(
            borderRadius: const BorderRadius.all(Radius.circular(14)),
            side: BorderSide(color: C.line),
          ),
        ),
        textTheme: (b == Brightness.dark
                ? Typography.whiteMountainView
                : Typography.blackMountainView)
            .apply(bodyColor: C.ink, displayColor: C.ink),
      ),
      home: HomeShell(
        brightness: b,
        onToggleTheme: _toggleTheme,
        lang: _effectiveLang,
        onSetLang: _setLang,
      ),
    );
  }
}

class HomeShell extends StatefulWidget {
  const HomeShell({
    super.key,
    required this.brightness,
    required this.onToggleTheme,
    required this.lang,
    required this.onSetLang,
  });
  final Brightness brightness;
  final VoidCallback onToggleTheme;
  final AppLang lang;
  final void Function(AppLang) onSetLang;
  @override
  State<HomeShell> createState() => _HomeShellState();
}

class _HomeShellState extends State<HomeShell> {
  final Future<Pack> _pack = AppCore.instance.pack;
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
    (Icons.dashboard_outlined, Icons.dashboard, 'Panel'),
    (Icons.show_chart_outlined, Icons.show_chart, 'Live'),
    (Icons.error_outline, Icons.error, 'Faults'),
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
                child: Text('${L.t('Error loading the pack:')}\n${snap.error}',
                    textAlign: TextAlign.center,
                    style: TextStyle(color: C.crit)),
              ),
            ),
          );
        }
        if (!snap.hasData) {
          return Scaffold(
            body: Center(child: CircularProgressIndicator(color: C.accent)),
          );
        }
        final pack = snap.data!;
        final car = AppCore.instance.carFor(pack);
        if (_demo && !car.demo && !car.connected) {
          _demo = false; // uma vez só (desconectar não recarrega o demo)
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
                      label: Text(L.t(d.$3)),
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
            titleSpacing: 12,
            title: const _Brand(),
            actions: [
              PopupMenuButton<AppLang>(
                tooltip: 'Language',
                icon: Icon(Icons.translate, color: C.ink2, size: 20),
                onSelected: widget.onSetLang,
                itemBuilder: (_) => [
                  for (final l in AppLang.values)
                    PopupMenuItem(
                      value: l,
                      child: Row(children: [
                        Icon(
                          widget.lang == l
                              ? Icons.radio_button_checked
                              : Icons.radio_button_unchecked,
                          size: 16,
                          color: widget.lang == l ? C.accent : C.ink3,
                        ),
                        const SizedBox(width: 10),
                        Text(_langName(l)),
                      ]),
                    ),
                ],
              ),
              IconButton(
                tooltip: widget.brightness == Brightness.dark
                    ? L.t('Light theme')
                    : L.t('Dark theme'),
                onPressed: widget.onToggleTheme,
                icon: Icon(
                  widget.brightness == Brightness.dark
                      ? Icons.light_mode_outlined
                      : Icons.dark_mode_outlined,
                  color: C.ink2,
                  size: 20,
                ),
              ),
              Padding(
                padding: const EdgeInsets.only(right: 12),
                child: ListenableBuilder(
                  listenable: car,
                  builder: (context, _) {
                    if (car.connecting || car.reading) {
                      return Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 14),
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
                        label: Text(L.t('Disconnect')),
                        style: OutlinedButton.styleFrom(
                          foregroundColor: C.good,
                          side: BorderSide(color: C.line),
                        ),
                      );
                    }
                    return OutlinedButton.icon(
                      onPressed: () => _openConnect(context, car),
                      icon: const Icon(Icons.usb, size: 16),
                      label: Text(L.t('Connect')),
                      style: OutlinedButton.styleFrom(
                        foregroundColor: C.ink2,
                        side: BorderSide(color: C.line),
                      ),
                    );
                  },
                ),
              ),
            ],
            bottom: PreferredSize(
              preferredSize: const Size.fromHeight(1),
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
                        label: L.t(d.$3),
                      ),
                  ],
                ),
        );
      },
    );
  }

  void _openConnect(BuildContext context, CarState car) {
    if (!CarLink.available) {
      showDialog(
        context: context,
        builder: (_) => AlertDialog(
          backgroundColor: C.panel,
          title: Text(L.t('Car connection')),
          content: Text(
            L.t('This is the web version (no hardware access). Reading the car (VIN, active faults, sensor values) needs the native app (Windows/macOS/Android), which talks to the Mongoose over the FFI bridge. Here you can see the pack data: modules, live-data catalog and the fault reference with the SDD help per module.'),
            style: TextStyle(color: C.ink2),
          ),
          actions: [
            TextButton(
                onPressed: () => Navigator.pop(context),
                child: Text(L.t('Got it'))),
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
        title: Text(L.t('Connect to the Mongoose')),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: portCtrl,
              decoration: InputDecoration(
                  labelText: L.t('Port (e.g. COM5, /dev/ttyACM0)')),
            ),
            const SizedBox(height: 8),
            TextField(
              controller: baudCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Baud (HS-CAN)'),
            ),
            const SizedBox(height: 10),
            Text(
              L.t('Reads VIN, scans the modules and reads the faults — reading writes nothing. Clearing faults is a separate action, always confirmed. Ignition on, engine off.'),
              style: TextStyle(fontSize: 11.5, color: C.ink3),
            ),
          ],
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(context),
              child: Text(L.t('Cancel'))),
          FilledButton(
            onPressed: () {
              final port = portCtrl.text.trim();
              final baud = int.tryParse(baudCtrl.text.trim()) ?? 500000;
              Navigator.pop(context);
              car.connect(port, baud);
            },
            child: Text(L.t('Connect')),
          ),
        ],
      ),
    );
  }
}

/// Marca do app na AppBar: ícone da D4 + "LR Scanner" + o veículo.
class _Brand extends StatelessWidget {
  const _Brand();

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        ClipRRect(
          borderRadius: BorderRadius.circular(9),
          child: Image.asset('assets/brand/logo.png',
              width: 34, height: 34, filterQuality: FilterQuality.medium),
        ),
        const SizedBox(width: 10),
        Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text.rich(
              TextSpan(children: [
                TextSpan(
                    text: 'LR',
                    style: TextStyle(
                        color: C.accent,
                        fontWeight: FontWeight.w800,
                        letterSpacing: 0.6)),
                TextSpan(
                    text: ' Scanner',
                    style: TextStyle(
                        color: C.ink,
                        fontWeight: FontWeight.w500,
                        letterSpacing: 0.2)),
              ]),
              style: const TextStyle(fontSize: 17, height: 1.1),
            ),
            Text('Discovery 4 · L319',
                style: TextStyle(
                    color: C.ink3, fontSize: 11, letterSpacing: 0.3, height: 1.2)),
          ],
        ),
      ],
    );
  }
}
