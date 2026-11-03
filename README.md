# jlr-open-diag

**English** · [Português](README.pt-BR.md)

Open, cross-platform (Windows 11, macOS, Android) diagnostics and configuration
tool for Jaguar Land Rover vehicles, reimplementing the functions of the
**JLR SDD** (Symptom Driven Diagnostics) over the **Mongoose Pro JLR** adapter
(J2534, USB).

First validated target: **Land Rover Discovery 4 (L319) 3.0 SDV6 diesel**.
The architecture is generic across the SDD's whole coverage; other vehicles are
added as they are validated.

> **Beta 0.1** — all four platforms (Windows, macOS, Android, web) build; reading
> and diagnostics were validated on a real car. Binaries under
> **[Releases](../../releases)**. The **Android** connection is **experimental**
> (USB-OTG, not yet validated on a device) and **CCF writing** depends on a
> proprietary JLR binary — details below. The app UI is available in **English,
> Spanish and Portuguese** (language picker in the app bar; follows the system
> locale by default).

## Current status

Reading and diagnostics **work on the real car** (validated on an L319 MY12).
**CCF writing** is blocked by a proprietary JLR binary (the SBL) — see "Known
blocker" below.

| Area | Deliverable | Status |
|---|---|---|
| `sdd-crypto` | `.exml` codec (3DES) | ✅ |
| `sdd-data` / `sdd-import` | parsers → SQLite, `.exml` `decrypt` | ✅ |
| `mongoose` | native USB/serial protocol of the adapter | ✅ |
| `diag` | UDS/ISO-TP: VIN, module scan, DTCs with help, live data | ✅ on car |
| `ccf` (read) | read + decode the BCM CCF | ✅ on car |
| `ccf` (write) | edit and write the CCF | ⛔ blocked (SBL) |
| Flutter app | UI (Windows/macOS/Android/web) + FFI/HTTP bridge | ✅ builds on all 4; Windows/macOS/web connect; Android USB-OTG **experimental** |
| `j2534-shim` | capture SDD writing (unlocks the CCF) | 🚧 planned |
| `procedures` | service routines (DPF, injectors, suspension…) | ⬜ |
| expansion | all vehicles covered by the SDD | ⬜ |

## Builds and install (beta 0.1)

Beta binaries under **[Releases](../../releases)**. To build from source (load
the environment with `source env.sh` first):

- **Windows (native):** `bash app/jlr_app/build-windows.sh` → `jlr_sdd.exe`
  (direct FFI to the Mongoose; requires VS Build Tools with the C++ workload).
- **Android (APK):** `bash app/jlr_app/build-android.sh` → `app-release.apk`
  (requires Android SDK/NDK + `cargo-ndk`; USB-OTG connection is experimental —
  see [`app/jlr_app/ANDROID.md`](app/jlr_app/ANDROID.md)).
- **macOS:** `bash app/jlr_app/build-macos.sh` on a Mac
  (see [`app/jlr_app/MACOS.md`](app/jlr_app/MACOS.md)).
- **Web + local server:** `run-app.bat` (starts `car-scan serve` and opens the browser).

The app **does not embed JLR data**: generate your vehicle's pack from your own
SDD install with `sdd-import` (it becomes `app/jlr_app/assets/pack.json`).

## Known blocker: CCF writing (help wanted)

Reading and decoding the BCM's CCF (vehicle configuration) works. **Writing**
does not: the BCM requires a **Secondary Boot Loader (SBL)** to be downloaded to
its RAM before it accepts the erase/flash of the config region — without it, the
erase replies `7F 31 22` (conditionsNotCorrect), even with the programming
session and security correct. That SBL **does not ship in a local SDD install**:
SDD fetches it from JLR's online server (TOPIx) at write time. The full
investigation (DLL RE, on-car tests, decrypted data) is in
[`docs/security-ccf-write.md`](docs/security-ccf-write.md).

**Help wanted:** writing the CCF on an L319 (EUCD) BCM needs JLR's Secondary Boot
Loader (SBL), delivered from JLR online — it is not in a local SDD install.
Everything else (reading/decoding the CCF, DTCs, live data, security access)
works. If you have the **L319 BCM SBL VBF**, or a **J2534 capture of SDD writing
a CCF**, please open an issue — that single piece is what's needed to finish
offline CCF writing.

## Data and rights principle

This repository **does not contain** Jaguar Land Rover data. The diagnostic
databases (module definitions, DTCs, DIDs, CCF map, texts) belong to JLR and
live in the user's own SDD install. The `sdd-import` tool reads that install
**locally**, on the vehicle owner's machine, and generates data packs that are
likewise not redistributed (see `.gitignore`).

Our own code is MIT-licensed. The `mongoose` crate, deriving from reverse
engineering published under LGPL-2.1
(`github.com/dgcarr/Mongoose-USB-2-OBDII-Linux-Driver`), is LGPL-2.1. GPL code
(e.g. `5555l/ccf_parser`) is used only as a **behavior reference**, never copied.

## Warning

Touching a vehicle's modules carries risk. Every write backs up first, requires
ignition on with the engine off, and explicit confirmation. Use at your own risk.

## Development environment

Toolchains live under `C:\dev` (portable, no admin needed): Rust (GNU toolchain +
mingw-w64), Flutter, JDK, Ghidra; Android SDK/NDK under `C:\dev\android`. Load the
environment with `source env.sh` before `cargo`/`flutter`.

```bash
source env.sh
cargo test            # tests all crates
```
