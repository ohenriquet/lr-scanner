# Android (USB-OTG) — estado e plano

O app já roda a camada de dados no Android (pacote + UI). Falta a leitura do
carro por **USB-OTG**. Diferente do desktop (porta COM serial), no Android o
acesso ao USB passa pelo **USB Host API**: o app pede permissão, abre o
dispositivo e recebe um **file descriptor (fd)**; o Rust fala os endpoints bulk
por esse fd.

## Feito
- Alvo `android/` scaffoldado.
- `AndroidManifest.xml`: `uses-feature android.hardware.usb.host`, intent-filter
  `USB_DEVICE_ATTACHED` e `res/xml/device_filter.xml` com o Mongoose
  (VID 0x18E1 / PID 0x0104) — o app é oferecido ao plugar o adaptador.

## Falta

### 1. Compilar a ponte para Android (.so por ABI)
A `jlr_bridge` precisa virar `libjlr_bridge.so` para cada ABI, em
`android/app/src/main/jniLibs/<abi>/`. Com o Android NDK instalado:
```sh
cargo install cargo-ndk
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 \
  -o app/src/main/jniLibs build --release -p jlr-bridge
```
(ver `build-android.sh`).

### 2. Transporte USB (em vez do serial)
O `SerialTransport` (crate `diag`) é desktop. Para Android, um `UsbTransport`
que implemente o mesmo trait `mongoose::Transport` sobre o **fd**:
- o adaptador é CDC-ACM; falar os endpoints bulk (OUT 0x01 / IN 0x82) por
  `nusb::Device::from_fd(fd)` (a crate `nusb` aceita um fd já aberto), ou via
  `ioctl` do `usbfs`;
- enviar `SET_CONTROL_LINE_STATE` (DTR/RTS), como faz o driver cdc_acm, antes
  de transmitir.
O `jlr_bridge` ganha uma entrada `jlr_connect_fd(fd, baud)` que monta a sessão
com esse transporte (o `jlr_connect` atual, por porta, continua no desktop).

### 3. Permissão + fd no lado Dart/Android
Um plugin (ou `MethodChannel`) que:
- lista dispositivos (`UsbManager.getDeviceList`), acha o Mongoose;
- pede permissão (`requestPermission`), abre (`openDevice`) e obtém o fd
  (`UsbDeviceConnection.getFileDescriptor`);
- passa o fd para o Rust via `jlr_connect_fd`.
Em Flutter dá para usar um pacote USB (ex.: `usb_serial`/`quick_usb`) só para a
permissão/enumeração e então repassar o fd.

### 4. CarLink no Android
`car_link_io.dart` hoje chama `connect(port)`. No Android, o fluxo é
`listar → permissão → fd → connect_fd(fd)`. O `CarLink` ganha um caminho
Android (mesma API assíncrona), mantendo a UI igual.

## Testar
Precisa de um aparelho Android com USB Host (a maioria dos tablets/telefones
recentes), um cabo OTG e o Mongoose + carro. `flutter build apk --release` e
instalar; ao plugar o Mongoose, o app abre e o botão Conectar faz o fluxo USB.
