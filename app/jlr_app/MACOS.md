# App macOS (nativo, FFI)

O alvo macOS está preparado, mas **o build e o teste só rodam num Mac** (não dá
pra compilar a partir do Windows). O lado Dart já está pronto: `lib/bridge.dart`
procura a `libjlr_bridge.dylib` dentro do bundle (`Contents/Frameworks/`), e a
UI/estado são os mesmos das outras plataformas.

## Pré-requisitos no Mac
- **Xcode** + command line tools (`xcode-select --install`).
- **Flutter** (canal stable) e **Rust** (`rustup`).
- O **Mongoose Pro JLR** aparece como porta serial `/dev/cu.usbmodem*` (CDC-ACM),
  sem driver extra — o núcleo Rust fala com ele pela `serialport`.

## Como compilar
No repositório (que você clona do GitHub), com um pacote do veículo gerado no PC
do SDD (`packs/L319_201200.sqlite` — veja o README):

```bash
bash app/jlr_app/build-macos.sh                 # usa packs/L319_201200.sqlite
# ou apontando outro pacote:
bash app/jlr_app/build-macos.sh caminho/para/VEH.sqlite
```

O script: compila a `libjlr_bridge.dylib` (arquitetura do seu Mac), exporta o
pacote pro app, roda `flutter build macos --release`, **embute a dylib** em
`Contents/Frameworks/` e re-assina o app ad-hoc (pro Gatekeeper não barrar).

Resultado: `app/jlr_app/build/macos/Build/Products/Release/jlr_sdd.app`.

## Rodar
Abra o `.app`. Em **Conectar**, informe a porta do Mongoose (ex.:
`/dev/cu.usbmodem1413`). Lê VIN, varre módulos e lê falhas por módulo em segundo
plano, igual às outras plataformas.

## Notas
- **Arquitetura:** o script compila a dylib para o host (Apple Silicon → arm64,
  Intel → x86_64) e casa com o app. Para um `.app` *universal* (distribuir pros
  dois), seria preciso compilar a dylib nos dois alvos e juntar com `lipo` — fica
  pra depois.
- **Permissão de porta serial:** se o macOS pedir, autorize o acesso ao
  dispositivo USB/serial na primeira conexão.
- **Assinatura:** o re-sign é ad-hoc (local). Para distribuir fora da sua
  máquina, seria preciso assinar/notarizar com conta de desenvolvedor Apple.
