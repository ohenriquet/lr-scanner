# Changelog

## v0.1.0-beta — first cross-platform beta

First public beta. All four platforms build; reading and diagnostics were
validated on a real car (Land Rover Discovery 4 / L319 MY12, 3.0 SDV6).

### Works
- **Rust core**: decrypts SDD `.exml` (3DES), parses modules/DIDs/DTCs/CCF and
  generates per-vehicle packs (`sdd-import`); native Mongoose Pro JLR protocol;
  UDS/ISO-TP (including a software ISO-TP to get past the BCM's 128-byte buffer).
- **On-car diagnostics**: VIN, module scan, DTCs with the SDD per-module help,
  live data with scaling/units, and CCF read + decode.
- **Flutter app** on all four platforms, with shared UI:
  - **Windows (native)**: direct FFI to the Mongoose (COM port). Fast, no
    server/browser.
  - **macOS**: target ready to build on a Mac (FFI).
  - **Android**: APK with the Rust core (`.so`); full reference mode.
  - **Web**: via `car-scan serve` (local HTTP bridge) + browser.
- **UI in English (default), Spanish and Portuguese**, with a language picker
  (follows the system locale by default).
- **Light/dark theme** with a toggle (follows the system by default).
- Fast connect (shows VIN/modules immediately; faults read per module in the
  background) and faults split into **confirmed** vs history.
- `car-scan` tool (CLI): scan, DTCs, live, CCF read, and `serve`.

### Experimental / limitations
- **Android — car connection (USB-OTG)**: implemented end to end
  (`UsbManager` fd → `nusb` in Rust), but **not yet validated on a device**.
  See `app/jlr_app/ANDROID.md`.
- **CCF writing**: blocked by a proprietary JLR **Secondary Boot Loader (SBL)**
  that does not ship in a local SDD install (SDD downloads it online). All CCF
  reading/decoding works. See `docs/security-ccf-write.md` (help wanted).
- **macOS**: builds/tests only on a Mac (not from Windows).

### Notes
- The app **does not embed JLR data**; the user generates the pack from their
  own SDD install.
- Our own code is MIT-licensed; the `mongoose` crate is LGPL-2.1 (derived from
  public reverse engineering).
