# Android (USB-OTG)

O app Android está **completo e compila** (UI + núcleo Rust via `.so`). A
**conexão com o carro por USB-OTG está implementada, porém EXPERIMENTAL** —
ainda não validada num dispositivo real.

No desktop o Mongoose é uma porta COM serial; no Android o acesso ao USB passa
pelo **USB Host API**: o app pede permissão, abre o dispositivo e recebe um
**file descriptor (fd)**; o núcleo Rust fala os endpoints bulk CDC-ACM por esse
fd (via `nusb::Device::from_fd`).

## Como está montado (ponta a ponta)
1. **Rust** (`crates/diag/src/usb_fd.rs`): `UsbFdTransport` implementa
   `mongoose::Transport` sobre o fd — duplica o fd, abre com `nusb`, manda
   `SET_LINE_CODING` + `SET_CONTROL_LINE_STATE` (DTR/RTS) e usa um *reader* de
   fundo lendo o bulk IN para um buffer. `CarSession::open_usb_fd(fd, baud)`.
2. **Ponte** (`crates/bridge`): `jlr_connect_fd(fd, baud)` monta a sessão USB
   (a sessão vira um enum serial|usb). Só existe na `.so` do Android.
3. **Kotlin** (`MainActivity.kt`): `MethodChannel("jlr/usb")` → `openMongoose`
   acha o Mongoose (VID 0x18E1/PID 0x0104), pede permissão (`requestPermission`
   + BroadcastReceiver), abre (`openDevice`) e devolve `getFileDescriptor()`.
4. **Dart** (`car_link_io.dart`): no Android, `connect()` pega o fd pelo canal e
   chama `JlrBridge.connectFd(fd)` (FFI) em vez do caminho serial.
5. **Manifest**: `uses-feature android.hardware.usb.host`, intent-filter
   `USB_DEVICE_ATTACHED` + `res/xml/device_filter.xml` (o app é oferecido ao
   plugar o adaptador).

## Build
```sh
bash app/jlr_app/build-android.sh      # .so (3 ABIs) + pack + flutter build apk
# APK: app/jlr_app/build/app/outputs/flutter-apk/app-release.apk
```
Pré-requisitos: Android SDK + NDK, `cargo-ndk`, targets Rust de Android, Flutter
(ver `env.sh`; o setup fica em `C:\dev\android`).

## Testar no carro
Aparelho com USB Host + cabo OTG + Mongoose + carro (ignição ligada). Instale o
APK (`adb install -r ...`), plugue o Mongoose (o app abre), toque **Conectar**:
o Android pede permissão USB e o app conecta pelo fd.

## Incertezas a validar no dispositivo (ajustar se falhar)
- **Interface/endpoints** em `usb_fd.rs`: assumi o layout padrão CDC-ACM
  (comm=0, data=1, bulk OUT 0x01 / IN 0x82). Se a conexão falhar, confira os
  descritores reais do Mongoose e ajuste as constantes.
- **Ciclo de vida do fd**: o Rust duplica o fd (dono de uma cópia) e o Kotlin
  mantém a `UsbDeviceConnection` viva (dono do original). Conferir que não há
  double-close nem fd fechado cedo.
- **nusb/`from_fd` no Android**: compila; falta exercitar em runtime.
