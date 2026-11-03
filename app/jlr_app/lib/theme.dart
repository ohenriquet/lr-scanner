import 'package:flutter/material.dart';

/// Paleta do app — clara ou escura conforme [C.brightness], que o app define a
/// cada build segundo o tema ativo (sistema ou override do usuário pelo toggle).
/// As cores são getters (não const) para poderem alternar; por isso os widgets
/// que as usam não podem estar em contexto `const`.
class C {
  /// Definido pelo app no topo da árvore antes de construir (ver main.dart).
  static Brightness brightness = Brightness.dark;
  static bool get _d => brightness == Brightness.dark;

  // fundo e superfícies
  static Color get bg => _d ? const Color(0xFF0F141A) : const Color(0xFFF4F6F8);
  static Color get panel => _d ? const Color(0xFF151A21) : const Color(0xFFFFFFFF);
  static Color get panel2 => _d ? const Color(0xFF1B222B) : const Color(0xFFEDF1F4);
  static Color get line => _d ? const Color(0xFF26303B) : const Color(0xFFD7DEE4);

  // texto
  static Color get ink => _d ? const Color(0xFFEEF3F7) : const Color(0xFF15202A);
  static Color get ink2 => _d ? const Color(0xFFA7B4C0) : const Color(0xFF53626E);
  static Color get ink3 => _d ? const Color(0xFF748896) : const Color(0xFF86949F);

  // acento + estados
  static Color get accent => _d ? const Color(0xFF2DD4BF) : const Color(0xFF0E9E8E);
  static Color get accentDim => _d ? const Color(0xFF16413F) : const Color(0xFFCFEFEA);
  static Color get good => _d ? const Color(0xFF34D399) : const Color(0xFF0E9E6E);
  static Color get warn => _d ? const Color(0xFFF2B347) : const Color(0xFFB07D16);
  static Color get crit => _d ? const Color(0xFFF6685E) : const Color(0xFFD6453F);

  static const mono = TextStyle(fontFamily: 'monospace', letterSpacing: 0.2);
}
