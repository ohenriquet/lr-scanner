import 'car_state.dart';
import 'pack.dart';

/// Núcleo do app fora da árvore de widgets: o pacote do veículo e o estado da
/// ligação com o carro.
///
/// Vive no `main()` (e não dentro de um widget) porque, no Android, a mesma
/// engine do Flutter atende também o Android Auto — e quando o app só está
/// aberto no carro não há Activity, a engine não desenha frames e a árvore de
/// widgets nunca é construída. Celular e carro partilham esta mesma instância.
class AppCore {
  AppCore._();
  static final instance = AppCore._();

  /// Pacote do veículo (`assets/pack.json`), carregado uma vez.
  late final Future<Pack> pack = Pack.load();

  CarState? _car;

  /// O estado do carro, criado na primeira vez a partir do pacote carregado.
  CarState carFor(Pack p) => _car ??= CarState(p);

  /// O estado do carro, se já foi criado.
  CarState? get car => _car;
}
