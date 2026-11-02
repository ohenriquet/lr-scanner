# jlr_app — app Flutter (porte do JLR SDD)

App multiplataforma (Windows, macOS, Android, web) que consome o núcleo Rust.
Lê o **pacote do veículo** (módulos, catálogo de live data, referência de
falhas com help do SDD por módulo) e, nos alvos nativos, fala com o carro pela
**ponte FFI** (`jlr_bridge.dll`): VIN, varredura de módulos, falhas ativas e
valores ao vivo dos sensores.

## Dados (não versionados)

```sh
# pacote do veículo -> JSON que o app carrega
cargo run -p sdd-import -- pack-export \
  --pack packs/L319_201200.sqlite --out app/jlr_app/assets/pack.json

# imagem do carro (render de estúdio, uso pessoal)
cp app/assets/car/lr4_santorini_black.jpg app/jlr_app/assets/car/d4.jpg
```

## Rodar / compilar

**Web** (sem acesso ao carro — modo referência):
```sh
cd app/jlr_app && flutter run -d chrome
```
Deep-link: `?tab=0|1|2`, `?q=<código>`, `?demo=1` (pré-visualiza o estado
conectado com dados de exemplo).

**Windows** (nativo, lê o carro). Requer Visual Studio com o workload
*Desktop development with C++* (MSVC, C++ CMake tools, Windows 10/11 SDK) e o
Rust GNU do repo (ver `env.sh`). Um comando faz tudo:
```sh
bash app/jlr_app/build-windows.sh
# -> app/jlr_app/build/windows/x64/runner/Release/jlr_sdd.exe (com jlr_bridge.dll ao lado)
```
Ou manual:
```sh
cargo build --release -p jlr-bridge       # gera target/release/jlr_bridge.dll
cd app/jlr_app && flutter build windows --release
```
O `windows/CMakeLists.txt` copia a `jlr_bridge.dll` para junto do `.exe`
(tanto no `flutter run` quanto no bundle do `flutter build`). A DLL depende só
de DLLs do sistema (ucrt do Windows 10/11), sem runtime do mingw.

No app: **Conectar** → informe a porta do Mongoose (ex.: `COM5`). Ignição
ligada, motor desligado. Nada é gravado.

**Android** (USB-OTG): a fazer — precisa compilar a `libjlr_bridge.so` para os
alvos do NDK (`aarch64-linux-android`, etc.) e pôr em `android/app/src/main/jniLibs/`,
além do transporte USB-OTG (o `SerialTransport` atual é desktop). Ver `diag`.

## Estrutura

- `lib/pack.dart` — modelo + carregamento do `pack.json`.
- `lib/theme.dart` — paleta (escuro + acento teal).
- `lib/bridge.dart` — bindings `dart:ffi` da `jlr_bridge` (nativo-only).
- `lib/car_link*.dart` — `CarLink` com import condicional (FFI nativo / stub web).
- `lib/car_state.dart` — estado da conexão, leituras e decodificação ao vivo.
- `lib/screens.dart` — Painel, Ao vivo, Falhas.
- `lib/main.dart` — app, navegação responsiva, fluxo de conexão, deep-link.
- `tool/ffi_smoke.dart` — valida a ponte FFI na Dart VM (sem Flutter/VS).
