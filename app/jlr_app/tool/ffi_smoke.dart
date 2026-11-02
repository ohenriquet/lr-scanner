// Smoke test da ponte FFI (roda na Dart VM, não precisa de Flutter/VS).
//
//   dart run tool/ffi_smoke.dart ../../target/debug/deps/jlr_bridge.dll
//
// Exercita toda a superfície que o app usa, sem carro: carrega a DLL, chama
// version, tenta connect numa porta inválida (erro tratado) e confirma que
// vin/scan/dtcs/readDid degradam sem crash quando não há sessão.
// ignore_for_file: avoid_print
import 'dart:io';
import 'package:jlr_sdd/bridge.dart';

void fail(String m) {
  stderr.writeln('FALHOU: $m');
  exit(1);
}

void main(List<String> args) {
  if (args.isEmpty) {
    stderr.writeln('uso: dart run tool/ffi_smoke.dart <caminho jlr_bridge.dll>');
    exit(2);
  }
  final b = JlrBridge.open(args[0]);

  final v = b.version();
  print('version(): $v');
  if (v['version'] == null) fail('jlr_version sem versão');

  final c = b.connect('COM_INEXISTENTE_99', baud: 500000);
  print('connect(porta inválida): $c');
  if (c['ok'] != false) fail('esperava ok:false para porta inválida');

  // Sem sessão: devem degradar (null / vazio), não crashar.
  final vin = b.vin();
  print('vin() sem sessão: $vin');
  if (vin != null) fail('esperava vin null sem sessão');

  final present = b.scan([
    ['PCM', 2016, 2024],
    ['RLM', 2003, 2011],
  ]);
  print('scan() sem sessão: $present');
  if (present.isNotEmpty) fail('esperava scan vazio sem sessão');

  final dtcs = b.dtcs([
    ['PCM', 2016, 2024],
  ]);
  print('dtcs() sem sessão: $dtcs');
  if (dtcs.isNotEmpty) fail('esperava dtcs vazio sem sessão');

  final hex = b.readDid(2016, 2024, 0x3B03);
  print('readDid() sem sessão: $hex');
  if (hex != null) fail('esperava readDid null sem sessão');

  print('OK: superfície FFI completa, sem crash.');
}
