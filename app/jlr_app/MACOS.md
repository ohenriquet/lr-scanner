# macOS app (native, FFI)

The macOS target is ready, but **building and testing only run on a Mac** (you
cannot compile it from Windows). The Dart side is already in place:
`lib/bridge.dart` finds `libjlr_bridge.dylib` inside the bundle
(`Contents/Frameworks/`), and the UI/state are the same as the other platforms.

## Prerequisites on the Mac
- **Xcode** + command line tools (`xcode-select --install`).
- **Flutter** (stable channel) and **Rust** (`rustup`).
- The **Mongoose Pro JLR** shows up as a serial port `/dev/cu.usbmodem*`
  (CDC-ACM), no extra driver — the Rust core talks to it via `serialport`.

## How to build
In the repository (which you clone from GitHub), with a vehicle pack generated
on the SDD machine (`packs/L319_201200.sqlite` — see the README):

```bash
bash app/jlr_app/build-macos.sh                 # uses packs/L319_201200.sqlite
# or pointing at another pack:
bash app/jlr_app/build-macos.sh path/to/VEH.sqlite
```

The script: builds `libjlr_bridge.dylib` (your Mac's architecture), exports the
pack to the app, runs `flutter build macos --release`, **embeds the dylib** in
`Contents/Frameworks/` and re-signs the app ad-hoc (so Gatekeeper lets it run).

Result: `app/jlr_app/build/macos/Build/Products/Release/jlr_sdd.app`.

## Run
Open the `.app`. In **Connect**, enter the Mongoose port (e.g.
`/dev/cu.usbmodem1413`). It reads VIN, scans modules and reads faults per module
in the background, just like the other platforms.

## Notes
- **Architecture:** the script builds the dylib for the host (Apple Silicon →
  arm64, Intel → x86_64) and matches the app. For a *universal* `.app` (to ship
  to both), you would build the dylib for both targets and merge them with
  `lipo` — left for later.
- **Serial port permission:** if macOS asks, allow access to the USB/serial
  device on the first connection.
- **Signing:** the re-sign is ad-hoc (local). To distribute off your machine you
  would sign/notarize with an Apple developer account.
