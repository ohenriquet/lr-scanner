import 'package:flutter/material.dart';

/// Idiomas suportados pela interface.
enum AppLang { en, es, pt }

/// Internacionalização simples e central: a chave é o texto em **inglês**
/// (idioma padrão); `_es`/`_pt` trazem as traduções e o que faltar cai de volta
/// no inglês. Para adicionar um idioma: criar o mapa e um ramo em [t] e nos
/// métodos com parâmetro. `L.lang` é global e o app reconstrói ao trocar.
class L {
  static AppLang lang = AppLang.en;

  static const List<Locale> supportedLocales = [
    Locale('en'),
    Locale('es'),
    Locale('pt'),
  ];

  /// Define o idioma a partir de um `Locale` (do sistema). Desconhecido → inglês.
  static void setFromLocale(Locale? l) {
    lang = switch (l?.languageCode) {
      'es' => AppLang.es,
      'pt' => AppLang.pt,
      _ => AppLang.en,
    };
  }

  static String get label => switch (lang) {
        AppLang.en => 'English',
        AppLang.es => 'Español',
        AppLang.pt => 'Português',
      };

  /// Traduz uma chave em inglês para o idioma atual (fallback: a própria chave).
  static String t(String en) => switch (lang) {
        AppLang.en => en,
        AppLang.es => _es[en] ?? en,
        AppLang.pt => _pt[en] ?? en,
      };

  // ── strings com parâmetro ────────────────────────────────────────────────

  static String liveHintConnected(int n) => switch (lang) {
        AppLang.es =>
          'Toca una señal para observarla (lectura en vivo ~2 Hz). $n señales en el catálogo.',
        AppLang.pt =>
          'Toque num sinal para observar (leitura ao vivo a ~2 Hz). $n sinais no catálogo.',
        AppLang.en =>
          'Tap a signal to watch it (live read ~2 Hz). $n signals in the catalog.',
      };

  static String liveHintOffline(int n, int mods) => switch (lang) {
        AppLang.es =>
          'Catálogo de datos en vivo — $n señales en $mods módulos. Los valores en vivo requieren el coche.',
        AppLang.pt =>
          'Catálogo de live data — $n sinais em $mods módulos. Valores ao vivo exigem o carro.',
        AppLang.en =>
          'Live data catalog — $n signals across $mods modules. Live values need the car.',
      };

  static String confirmedSummary(int confirmed, int total) => switch (lang) {
        AppLang.es =>
          '$confirmed confirmada${confirmed == 1 ? '' : 's'} · $total en memoria',
        AppLang.pt =>
          '$confirmed confirmada${confirmed == 1 ? '' : 's'} · $total na memória',
        AppLang.en => '$confirmed confirmed · $total in memory',
      };

  static String readingFaultsProgress(int done, int total) => switch (lang) {
        AppLang.es => 'Leyendo fallas… $done/$total módulos',
        AppLang.pt => 'Lendo falhas… $done/$total módulos',
        AppLang.en => 'Reading faults… $done/$total modules',
      };

  static String moduleLabel(String module) => switch (lang) {
        AppLang.es => 'módulo $module',
        AppLang.pt => 'módulo $module',
        AppLang.en => 'module $module',
      };

  static String noHelpFor(String base, String module) => switch (lang) {
        AppLang.es => 'Sin ayuda para $base en el módulo $module.',
        AppLang.pt => 'Sem help para $base no módulo $module.',
        AppLang.en => 'No help for $base in module $module.',
      };

  static String ccfMeta(
      String block, int byteStart, String maskHex, bool editable) {
    final tail = editable
        ? ''
        : switch (lang) {
            AppLang.es => ' · no editable en el SDD',
            AppLang.pt => ' · não editável no SDD',
            AppLang.en => ' · not editable in SDD',
          };
    return switch (lang) {
      AppLang.es => 'bloque $block · byte $byteStart · máscara 0x$maskHex$tail',
      AppLang.pt => 'bloco $block · byte $byteStart · máscara 0x$maskHex$tail',
      AppLang.en => 'block $block · byte $byteStart · mask 0x$maskHex$tail',
    };
  }

  // ── tabelas de tradução (chave = inglês) ─────────────────────────────────

  static const Map<String, String> _pt = {
    'Panel': 'Painel',
    'Live': 'Ao vivo',
    'Faults': 'Falhas',
    'Config': 'Config',
    'Connect': 'Conectar',
    'Disconnect': 'Desconectar',
    'Light theme': 'Tema claro',
    'Dark theme': 'Tema escuro',
    'Cancel': 'Cancelar',
    'Got it': 'Entendi',
    'Reread': 'Reler',
    'VEHICLE': 'VEÍCULO',
    'Model': 'Modelo',
    'Platform': 'Plataforma',
    'Year': 'Ano',
    'Engine': 'Motor',
    'online': 'online',
    'modules': 'módulos',
    'MODULES': 'MÓDULOS',
    'Connection failed': 'Falha ao conectar',
    'VIN, active faults and sensor values are read from the car — tap Connect.':
        'VIN, falhas ativas e valores de sensor são leituras do carro — toque em Conectar.',
    'Filter signals…': 'Filtrar sinais…',
    'Active faults read from the car. Search a code for the full reference.':
        'Falhas ativas lidas do carro. Busque um código para a referência completa.',
    'Fault reference: search a code for the description and the SDD help per module. Active ones come from the car.':
        'Referência de falhas: busque um código para a descrição e o help do SDD por módulo. As ativas vêm do carro.',
    'DTC code (e.g. C1A13, P0671)…': 'Código do DTC (ex.: C1A13, P0671)…',
    'No faults in memory': 'Nenhuma falha na memória',
    'No faults in memory.': 'Nenhuma falha na memória.',
    'Reading faults…': 'Lendo falhas…',
    'Type a code to search': 'Digite um código para buscar',
    'confirmed': 'confirmada',
    'history': 'histórico',
    'Additional description': 'Descrição complementar',
    'Symptoms': 'Sintomas',
    'Probable causes': 'Causas prováveis',
    'Recommended actions': 'Ações recomendadas',
    'No diagnostic help for this code in this pack.':
        'Sem help de diagnóstico para este código neste pacote.',
    'Vehicle configuration (CCF) read from the BCM. View only; writing to the BCM depends on a mechanism not yet unlocked.':
        'Configuração (CCF) lida do BCM. Visualização; a gravação no BCM depende de um mecanismo ainda não liberado.',
    'Vehicle configuration (CCF). Connect to read the current values. Writing is not available yet.':
        'Configuração (CCF) do veículo. Conecte para ler os valores atuais. A gravação ainda não está disponível.',
    'Filter options (e.g. light, alarm)…': 'Filtrar opções (ex.: luz, alarme)…',
    'Show advanced options (hidden in SDD)':
        'Mostrar opções avançadas (ocultas no SDD)',
    'Advanced': 'Avançadas',
    'Nothing found.': 'Nada encontrado.',
    'Error loading the pack:': 'Erro ao carregar o pacote:',
    'Car connection': 'Conexão com o carro',
    'This is the web version (no hardware access). Reading the car (VIN, active faults, sensor values) needs the native app (Windows/macOS/Android), which talks to the Mongoose over the FFI bridge. Here you can see the pack data: modules, live-data catalog and the fault reference with the SDD help per module.':
        'Esta é a versão web (sem acesso a hardware). A leitura do carro (VIN, falhas ativas, valores de sensor) exige o app nativo (Windows/macOS/Android), que fala com o Mongoose pela ponte FFI. Aqui você vê os dados do pacote: módulos, catálogo de live data e a referência de falhas com o help do SDD por módulo.',
    'Connect to the Mongoose': 'Conectar ao Mongoose',
    'Port (e.g. COM5, /dev/ttyACM0)': 'Porta (ex.: COM5, /dev/ttyACM0)',
    'Reads VIN, scans the modules and reads active faults. Ignition on, engine off. Nothing is written.':
        'Lê VIN, varre os módulos e lê as falhas ativas. Ignição ligada, motor desligado. Nada é gravado.',
    'Available only in the native app.': 'Disponível apenas no app nativo.',
    'connection failed': 'falha ao conectar',
    'Mongoose not found or USB permission denied':
        'Mongoose não encontrado ou permissão USB negada',
    'No local server. Run: car-scan --port COM5 serve --web app/jlr_app/build/web':
        'Sem servidor local. Rode: car-scan --port COM5 serve --web app/jlr_app/build/web',
  };

  static const Map<String, String> _es = {
    'Panel': 'Panel',
    'Live': 'En vivo',
    'Faults': 'Fallas',
    'Config': 'Config',
    'Connect': 'Conectar',
    'Disconnect': 'Desconectar',
    'Light theme': 'Tema claro',
    'Dark theme': 'Tema oscuro',
    'Cancel': 'Cancelar',
    'Got it': 'Entendido',
    'Reread': 'Releer',
    'VEHICLE': 'VEHÍCULO',
    'Model': 'Modelo',
    'Platform': 'Plataforma',
    'Year': 'Año',
    'Engine': 'Motor',
    'online': 'en línea',
    'modules': 'módulos',
    'MODULES': 'MÓDULOS',
    'Connection failed': 'Falló la conexión',
    'VIN, active faults and sensor values are read from the car — tap Connect.':
        'El VIN, las fallas activas y los valores de sensores se leen del coche — toca Conectar.',
    'Filter signals…': 'Filtrar señales…',
    'Active faults read from the car. Search a code for the full reference.':
        'Fallas activas leídas del coche. Busca un código para la referencia completa.',
    'Fault reference: search a code for the description and the SDD help per module. Active ones come from the car.':
        'Referencia de fallas: busca un código para la descripción y la ayuda del SDD por módulo. Las activas vienen del coche.',
    'DTC code (e.g. C1A13, P0671)…': 'Código DTC (ej.: C1A13, P0671)…',
    'No faults in memory': 'Ninguna falla en memoria',
    'No faults in memory.': 'Ninguna falla en memoria.',
    'Reading faults…': 'Leyendo fallas…',
    'Type a code to search': 'Escribe un código para buscar',
    'confirmed': 'confirmada',
    'history': 'histórico',
    'Additional description': 'Descripción complementaria',
    'Symptoms': 'Síntomas',
    'Probable causes': 'Causas probables',
    'Recommended actions': 'Acciones recomendadas',
    'No diagnostic help for this code in this pack.':
        'Sin ayuda de diagnóstico para este código en este paquete.',
    'Vehicle configuration (CCF) read from the BCM. View only; writing to the BCM depends on a mechanism not yet unlocked.':
        'Configuración (CCF) leída del BCM. Solo lectura; escribir en el BCM depende de un mecanismo aún no liberado.',
    'Vehicle configuration (CCF). Connect to read the current values. Writing is not available yet.':
        'Configuración (CCF) del vehículo. Conecta para leer los valores actuales. Escribir aún no está disponible.',
    'Filter options (e.g. light, alarm)…': 'Filtrar opciones (ej.: luz, alarma)…',
    'Show advanced options (hidden in SDD)':
        'Mostrar opciones avanzadas (ocultas en el SDD)',
    'Advanced': 'Avanzadas',
    'Nothing found.': 'Nada encontrado.',
    'Error loading the pack:': 'Error al cargar el paquete:',
    'Car connection': 'Conexión con el coche',
    'This is the web version (no hardware access). Reading the car (VIN, active faults, sensor values) needs the native app (Windows/macOS/Android), which talks to the Mongoose over the FFI bridge. Here you can see the pack data: modules, live-data catalog and the fault reference with the SDD help per module.':
        'Esta es la versión web (sin acceso a hardware). Leer el coche (VIN, fallas activas, valores de sensores) requiere la app nativa (Windows/macOS/Android), que habla con el Mongoose por el puente FFI. Aquí ves los datos del paquete: módulos, catálogo de datos en vivo y la referencia de fallas con la ayuda del SDD por módulo.',
    'Connect to the Mongoose': 'Conectar al Mongoose',
    'Port (e.g. COM5, /dev/ttyACM0)': 'Puerto (ej.: COM5, /dev/ttyACM0)',
    'Reads VIN, scans the modules and reads active faults. Ignition on, engine off. Nothing is written.':
        'Lee el VIN, escanea los módulos y lee las fallas activas. Encendido en contacto, motor apagado. No se escribe nada.',
    'Available only in the native app.': 'Disponible solo en la app nativa.',
    'connection failed': 'falló la conexión',
    'Mongoose not found or USB permission denied':
        'Mongoose no encontrado o permiso USB denegado',
    'No local server. Run: car-scan --port COM5 serve --web app/jlr_app/build/web':
        'Sin servidor local. Ejecuta: car-scan --port COM5 serve --web app/jlr_app/build/web',
  };
}
