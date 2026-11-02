// Smoke test da ponte FFI (roda na Dart VM, não precisa de Flutter/VS).
//
//   dart run tool/ffi_smoke.dart ../../target/debug/deps/jlr_bridge.dll
//
// Verifica: carrega a DLL, chama jlr_version (sem carro) e tenta um connect
// numa porta inexistente (deve voltar ok:false, sem crash).
// ignore_for_file: avoid_print
import 'dart:io';
import 'package:jlr_sdd/bridge.dart';

void main(List<String> args) {
  if (args.isEmpty) {
    stderr.writeln('uso: dart run tool/ffi_smoke.dart <caminho jlr_bridge.dll>');
    exit(2);
  }
  final b = JlrBridge.open(args[0]);

  final v = b.version();
  print('version(): $v');
  if (v['version'] == null) {
    stderr.writeln('FALHOU: jlr_version não retornou versão');
    exit(1);
  }

  // Porta inexistente -> erro tratado (não deve crashar o processo).
  final c = b.connect('COM_INEXISTENTE_99', baud: 500000);
  print('connect(porta inválida): $c');
  if (c['ok'] != false) {
    stderr.writeln('FALHOU: esperava ok:false para porta inválida');
    exit(1);
  }

  print('OK: ponte FFI carrega e responde.');
}
