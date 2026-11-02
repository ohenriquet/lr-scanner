# jlr_app — app Flutter (porte do JLR SDD)

App multiplataforma (Windows, macOS, Android, web) que consome os dados do
núcleo Rust. Primeira fase: lê o **pacote do veículo** (módulos, catálogo de
live data, referência de falhas com help do SDD por módulo). As leituras ao
vivo (VIN, falhas ativas, valores de sensor, CCF atual) virão pela ponte nativa
(FFI: serial no desktop, USB-OTG no Android).

## Dados (não versionados)

Dois assets vêm de fora do git (ver `.gitignore` da raiz):

```sh
# pacote do veículo -> JSON que o app carrega
cargo run -p sdd-import -- pack-export \
  --pack packs/L319_201200.sqlite \
  --out app/jlr_app/assets/pack.json

# imagem do carro (render de estúdio, uso pessoal)
cp app/assets/car/lr4_santorini_black.jpg app/jlr_app/assets/car/d4.jpg
```

## Rodar

```sh
cd app/jlr_app
flutter run -d chrome      # web
# flutter run -d windows   # precisa do workload C++ do Visual Studio
```

## Estrutura

- `lib/pack.dart` — modelo + carregamento do `pack.json`.
- `lib/theme.dart` — paleta (escuro + acento teal).
- `lib/screens.dart` — Painel, Ao vivo, Falhas.
- `lib/main.dart` — app, navegação responsiva, deep-link (`?tab=`, `?q=`).
