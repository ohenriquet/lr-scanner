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
3. **Kotlin** (`MongooseUsb.kt`): `MethodChannel("jlr/usb")` → `openMongoose`
   finds the Mongoose (VID 0x18E1/PID 0x0104), requests permission
   (`requestPermission` + BroadcastReceiver), opens it (`openDevice`) and returns
   `getFileDescriptor()`. It uses the Application context, so the car screen
   (Android Auto) can connect too.
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
Prerequisites: Android SDK + NDK, `cargo-ndk`, Rust Android targets, Flutter, a
JDK. On Windows the setup lives under `C:\dev\android` (see `env.sh`). On macOS:

```sh
brew install openjdk@21 && brew install --cask android-commandlinetools
sdkmanager --sdk_root="$HOME/Library/Android/sdk" "platform-tools" \
  "platforms;android-36" "build-tools;36.0.0" "ndk;27.2.12479018" "extras;google;auto"
rustup +stable target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo +stable install cargo-ndk
flutter config --android-sdk "$HOME/Library/Android/sdk"
```
The script picks the stable Rust toolchain, the SDK/NDK and the JDK by itself. It
needs `assets/pack.json` (exported from your SDD pack on the SDD PC; if the
`.sqlite` is not here, an existing `assets/pack.json` is used).

## Android Auto
The app also runs on the car screen: **read faults, clear faults (with
confirmation) and live charts** (air suspension heights + compressor temperature, engine,
battery, transmission, plus whatever you are watching on the phone).

**Requirement: wireless Android Auto.** The Mongoose uses the phone's USB port
(OTG); with wired Android Auto that port is taken by the head unit. A wireless
adapter on a wired head unit also works.

How it is wired: Android Auto only accepts **template** screens (Car App Library),
written in Kotlin (`kotlin/.../auto/`). The logic stays in Dart: `LrScannerApp`
creates **one shared Flutter engine** used by both the phone (`MainActivity`) and
the car (`LrCarAppService`), so both see the same state and the same Mongoose
session, even when the app is open only in the car. The `lr/auto` channel carries
commands (Kotlin → Dart) and state snapshots (Dart → Kotlin); see
`lib/auto_bridge.dart` and `auto/AutoBridge.kt`.

Setup on the phone (once):
1. **Install from the Mac/PC with the installer set to Play** — Android Auto may
   hide car apps that did not come from the Play Store:
   `adb install -r -i com.android.vending app-release.apk`
2. Android Auto settings → tap **Version** ~10 times → ⋮ → **Developer settings**
   → enable **Unknown sources**. Then in **Customize launcher**, make sure
   **LR Scanner** is ticked.
3. The first time you plug the Mongoose, choose **always open LR Scanner** for
   it: that keeps the USB permission granted, so the car can connect without a
   prompt on a locked phone. With the Mongoose plugged and authorised, opening
   LR Scanner in the car **connects by itself**.

Screens: Home (Faults · Live · Clear faults · Connection) → Faults (confirmed
first) → fault detail (SDD help + "Clear this module") · Clear (confirm →
clearing → result) · Live → group → chart (last 60 s). **Demo** (on the home
action strip when disconnected) shows sample data, handy for testing.

Testing without the car (Desktop Head Unit, in `extras;google;auto`): enable
developer mode as above, then in Android Auto ⋮ → **Start head unit server**;
on the computer `adb forward tcp:5277 tcp:5277` and run
`$ANDROID_HOME/extras/google/auto/desktop-head-unit`.

Clearing faults sends UDS `0x14 FF FF FF` (all groups) to each module, the same
service the SDD uses; a fault that is still present comes back on the next
read. Do it with the car stopped.

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
