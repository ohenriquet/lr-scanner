# Android (USB-OTG)

The Android app is **complete and builds** (UI + Rust core via `.so`). The
**car connection over USB-OTG is implemented but EXPERIMENTAL** — not yet
validated on a real device.

On desktop the Mongoose is a serial COM port; on Android the USB access goes
through the USB Host API: the app requests permission, opens the device and gets
a **file descriptor (fd)**; the Rust core talks the CDC-ACM bulk endpoints over
that fd (via `nusb::Device::from_fd`).

## How it is wired (end to end)
1. **Rust** (`crates/diag/src/usb_fd.rs`): `UsbFdTransport` implements
   `mongoose::Transport` over the fd — it dups the fd, opens it with `nusb`,
   sends `SET_LINE_CODING` + `SET_CONTROL_LINE_STATE` (DTR/RTS) and uses a
   background reader draining the bulk IN into a buffer. `CarSession::open_usb_fd(fd, baud)`.
2. **Bridge** (`crates/bridge`): `jlr_connect_fd(fd, baud)` builds the USB
   session (the session becomes a serial|usb enum). Only present in the Android `.so`.
3. **Kotlin** (`MainActivity.kt`): `MethodChannel("jlr/usb")` → `openMongoose`
   finds the Mongoose (VID 0x18E1/PID 0x0104), requests permission
   (`requestPermission` + BroadcastReceiver), opens it (`openDevice`) and returns
   `getFileDescriptor()`.
4. **Dart** (`car_link_io.dart`): on Android, `connect()` gets the fd over the
   channel and calls `JlrBridge.connectFd(fd)` (FFI) instead of the serial path.
5. **Manifest**: `uses-feature android.hardware.usb.host`, `USB_DEVICE_ATTACHED`
   intent-filter + `res/xml/device_filter.xml` (the app is offered when the
   adapter is plugged in).

## Build
```sh
bash app/jlr_app/build-android.sh      # .so (3 ABIs) + pack + flutter build apk
# APK: app/jlr_app/build/app/outputs/flutter-apk/app-release.apk
```
Prerequisites: Android SDK + NDK, `cargo-ndk`, Rust Android targets, Flutter
(see `env.sh`; the setup lives under `C:\dev\android`).

## Testing on the car
A device with USB Host + an OTG cable + the Mongoose + the car (ignition on).
Install the APK (`adb install -r ...`), plug the Mongoose (the app opens), tap
**Connect**: Android asks for USB permission and the app connects over the fd.

## Uncertainties to validate on the device (adjust if it fails)
- **Interface/endpoints** in `usb_fd.rs`: the standard CDC-ACM layout is assumed
  (comm=0, data=1, bulk OUT 0x01 / IN 0x82). If the connection fails, check the
  Mongoose's real descriptors and adjust the constants.
- **fd lifetime**: Rust dups the fd (owns a copy) and Kotlin keeps the
  `UsbDeviceConnection` alive (owns the original). Check there is no double-close
  nor an fd closed too early.
- **nusb/`from_fd` on Android**: it compiles; runtime still needs exercising.
