// Seleciona a implementação por plataforma: FFI nativo quando disponível
// (`dart.library.ffi`), senão o stub da web.
export 'car_link_web.dart' if (dart.library.ffi) 'car_link_io.dart';
