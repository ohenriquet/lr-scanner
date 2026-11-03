# SecurityAccess and CCF writing (L319 BCM) — notes

Goal: enable comfort options (welcome/approach/puddle lights, etc.) by writing
the CCF to the BCM. This requires an unlock via SecurityAccess (UDS 0x27).
Source: decrypted SDD data + disassembly of `SecAlg.dll` + cross-checking with
public reverse engineering of the Ford/JLR EUCD.

## What is already known (from decrypted SDD)

- The CCF is read/written on the BCM via DID 0xDE00 (service 0x22 read, 0x2E
  write), extended session (0x10 0x03). Confirmed: reading 0xDE00 returns 1024
  bytes (validated on the car).
- Writing BCM configuration DIDs requires `security_level_1` (MDX_BCM).
- This BCM's `BCM.xml` (swdl): `SECURITY_ALGORITHM_ID=0`,
  `NUMBER_OF_SECURITY_CONSTANTS=5`, `KEY_CONST_REF=168`.
- `Security.xml` group Index 168 (Module=BCM, 5 SecData) is this BCM's:
  - SecLevel 0x01 (P, programming), constant DC0163
  - **SecLevel 0x03 (D, diagnostic), constant DC0164** ← writes CCF
  - SecLevel 0x11 (D), DC0165
  - SecLevel 0x21 (D), DC0165
  - SecLevel 0x99 (X, special), DC0009
- For the CCF (diag session + level 1): request the seed with `0x27 0x03`, send
  the key with `0x27 0x04`, using constant **DC0164** (5 bytes = 40 bits) and
  algorithm ID 0.

## Cross-check with the community

Legacy Ford/JLR EUCD modules use a 24-bit LFSR: 3-byte seed, 3-byte key, with 40
bits of secret. Those 40 bits = exactly the 5 per-module constants from the SDD.
Important difference: the SDD **already ships the secret** (DC0164), so nothing
needs cracking — only applying the LFSR. The exact form (taps, byte order,
iterations) comes from `SecAlg.dll`.

Public references (methodology, not JLR's key):
- Ford F-150 IPC SecurityAccess (24-bit LFSR, 40-bit secret):
  https://github.com/Ol-Swamp-Donkey/ford-f150-ipc-securityaccess
- Mercedes IC204 (seed/key RE method with Ghidra):
  https://github.com/Mattwmaster58/ic204

## Planned write sequence (NOT executed yet)

1. `0x10 0x03` on the BCM (0x726) — extended session. Reversible.
2. `0x27 0x03` → BCM replies `0x67 0x03 <seed>` (requesting the seed is safe, it
   does not count as a failed attempt).
3. key = algorithm0(seed, DC0164).
4. `0x27 0x04 <key>` → `0x67 0x04` (ok) or `0x7F 27 <NRC>`. CAUTION: a repeated
   wrong key triggers the module's anti-brute-force delay.
5. Assemble the modified CCF: read the current 1024 bytes, change only the
   parameter's byte(s), recompute the CCF checksums (CHECKSUM_CCF_EUCD) and the
   date (DATECALC_JAG_X150).
6. Write `0x2E 0xDE00 <1024 bytes>` (or the VBF 0x34/0x36/0x37 sequence if the
   SDD uses that path for CARCFG).
7. Re-read 0xDE00 and compare byte by byte. Tester present during the process.

## To do before any write

- [x] Algorithm extracted: `GetSecurityCodeISO14229` (LFSR init 0xC541A9, poly
  0x109028, 64 iterations). Implemented in the `jlr-security` crate.
- Reconstruct the CCF checksum (CHECKSUM_CCF_EUCD) — likely in the CCF DLLs.
- [x] Generator VALIDATED against the SDD's own `SecAlg.dll` (export
  `SECALG_GetSecurityKey`, algorithm 32, 3-byte seed): 12/12 seeds produce an
  identical key. 32-bit oracle in `tools/secalg-oracle`.
- Always back up the CCF first (we have one) and have a restore plan.

## Procedure safety

Requesting the seed (0x27 03) is a read and may be repeated. Only sending a wrong
key (0x27 04) is penalized. The strategy is to not send a key until we have high
confidence (algorithm verified and, ideally, validated against a capture).

## Checksum (reconstruction in progress)

Algorithm confirmed from `VbfConversion.dll` (`PerformEUCD_CCCChecksum`): 8-bit
sum of the bytes in a range, `checksum = sum & 0xff`. Clues in the car's real
CCF: `(-sum8(block1)) & 0xff == 0x53`, which is the stored
`VEH_BUILD_CHECKSUM_BYTE_1` — i.e. a two's-complement checksum (data sum +
checksum byte ≡ 0 mod 256). Positions in the DID 0xDE00 image: EUCD checksum at
562/563, VEH_BUILD at 696/697 (block 5, bytes 2/3 and 136/137). The exact range
of each is still to be pinned down (the SDD sums over the VBF buffer, whose layout
differs from the raw image). Until closed, `ccf-set` is blocked; `ccf-writeback`
(rewrite identical) does not need the checksum.

## Write sequence mapped on the car (MY12 BCM)

Validated on the Discovery 4's real BCM, via software ISO-TP:
1. `0x10 03` (extended session) — ok.
2. `0x27 03`/`0x04` DIAGNOSTIC unlock (constant DC0164) — ACCEPTED.
3. `0x10 02` (programming session) — ok (module stays alive, 0x3E → 0x7E).
4. `0x27 01`/`0x02` PROGRAMMING unlock (constant DC0163) — ACCEPTED. The same
   `jlr-security` algorithm works for the programming level.
5. `0x34` RequestDownload — supported ONLY in the programming session; with
   security ok it replies `0x7F 34 31` (requestOutOfRange) for the CCF addresses
   (0xFFAC00, blocks and region). So the standard 0x34 points to the firmware
   region, not the CCF.

Conclusion: writing the CCF is NOT a plain RequestDownload to 0xFFAC00. It is an
SDD-specific mechanism (likely a proprietary DID/routine or a RequestDownload with
a different logical address/format). To close it safely, the reliable path is to
CAPTURE the SDD writing a CCF (J2534 shim) and copy the exact sequence (format,
address, block size, checksum).

Already validated and ready: the full session and security chain (diagnostic AND
programming) on the real module, and the software ISO-TP to send/receive any size
within the BCM's 128-byte buffer.

## CCF flash sequence — CONFIRMED by RE (offline)

I decompiled (Ghidra 12.2 headless) the SDD's own DLLs that write the CCF:
`SessionCcf.dll` (orchestrator), `VISO14229.dll` (UDS ISO-14229 channel),
`SWDL.dll`/`SWDLInterface.dll` (software download engine) and `LegacyFlash.dll`
(discarded — it is the old powertrain reflash for Denso/Jatco/ZF/Puma, J2190, it
does not touch the CCF). Artifacts in the scratchpad: `decomp_sessionccf.txt`,
`decomp_viso14229.txt`.

**Central finding — the missing precondition.** The erase `31 01 FF00` is only
accepted after ARMING the module with a "Programming Enable" routine. In
`CSessionCcf::InitiateProgrammingEnableMode` (log: "Enabling SWDL Mode"):

```
V14229_SetDiagnosticSession(0x03)   → 10 03   (EXTENDED session, not 10 02)
ObtainDiagSecurityConstant(0x23) + V14229_SecureAccess → 27  (DIAG security)
V14229_PerformRoutine(routineId=0x800E, controlType=1) → 31 01 80 0E   ★
V14229_Disconnect
```

This is called inside `CSessionCcf::InitiateBlockWriteSWDLProcess`, **only when
`SWDL_GetProgrammingEnableFlag()==1`** (per-module flag; on for the BCM), and
**before** `SWDL_StartJob()` — which is what enters the programming session and
does erase/0x34/0x36/0x37. So our `7F 31 22` (conditionsNotCorrect) on the erase
was exactly the absence of `31 01 800E`.

The SDD's UDS channel (`CISO14229Channel`) implements 10, 11, 19, 27, 31, 34, 36,
37, 85 — and does **NOT** implement CommunicationControl (0x28). So the preamble
does not use 0x28 (earlier hypothesis discarded).

**Full write sequence (implemented in `ccf-flash`):**
1. `10 03` + `27` diag (DC0164) — extended session + diagnostic security.
2. `31 01 80 0E` (startRoutine, Programming Enable / Enabling SWDL Mode). ★
3. `10 02` + `27` prog (DC0163) — programming session.
4. `31 01 FF00 <addr 0xFFAC00> <len 0x400>` — erase.
5. `34 00 44 <addr> <len 698>` — RequestDownload.
6. `36 <seq> <data>` in chunks ≤ maxBlockLength − 2.
7. `37` — RequestTransferExit.
8. `11 01` — ECUReset; re-enables DTC/communication; re-read `0xDE00` and verify.

**Only residual uncertainty:** the RE shows step 2's security coming from
`ObtainDiagSecurityConstantFromAccessLevel(0x23)`; the BCM's group 168 levels are
0x03/0x11/0x21 (0x23 does not match). It may be an SDD-internal "access level"
mapping to the default diag constant (DC0164, the one we already use and that
opens diag), or it may require DC0165. The `ccf-flash` command in PROBE mode
(no `--write`) runs 1→3 and shows the `31 01 800E` reply: `71` = armed;
`7F 31 33` = different security level (try DC0165 before 800E); `7F 31 xx` = other.
That is non-destructive (erases nothing), so it is the first test on the car.

## UPDATE (on-car test + decrypted data): 0x800E DISCARDED

The probe on the car gave **`31 01 800E` → `7F 31 31` (requestOutOfRange)**: this
BCM does not have that routineId (the `0x800E` from `InitiateProgrammingEnableMode`
is from another platform). Diagnostic unlock 100% (seed→key ok). Nothing written.

Decrypting the SDD data (via the new `sdd-import decrypt`):
- `IDS/Xml/L319_201200/BCM.exml` (SWDL): `SOFTWARE_DOWNLOAD_PHASE=3` (spec
  "SWDL-004"), `SECURITY_ALGORITHM_ID=0`, `KEY_CONST_REF=168`,
  `READ_ECU_SERIAL_NO=1`. **No `PROGRAMMING_ENABLE_MODE`**.
- `IDS/Xml/L319_201200/MDX_BCM.exml`: the BCM's `ROUTINE_IDENTIFIERS` table does
  NOT contain `FF00`. So **the erase FF00 is a BOOTLOADER routine**, not an
  application one. Hence `conditionsNotCorrect` (recognized, but the module is not
  actually in programming mode) instead of `requestOutOfRange`. The application
  routines (0202 Self Test, 716E "Start Control Unit Program" in the PATS/key
  cluster, etc.) are all extended-session (session_03).

**Corrected conclusion:** `10 02` replies `0x50`, but the BCM does not actually
enter the bootloader. The strongest candidate (strings in SessionCcf.dll:
"Initialise bus with Programming Sessions" / "2 sec burst of Prog Sessions") is to
**hammer `10 02` for ~2 s** to force entry — today we send it once. Implemented in
`ccf-flash` (burst by default; `--no-preamble` disables it; `--start-routine <hex>`
runs a routine in the EXTENDED session, e.g. to test 716E in the right session).
Since the erase self-aborts if denied, testing `--write` is safe.

**If the burst does not solve it:** the definitive path is capture via a J2534
shim (phase 2) — observe the SDD writing a CCF and copy the exact sequence.

New tool: `sdd-import decrypt --file X.exml --out Y.xml` decrypts any `.exml`
(3DES) with the `sdd-crypto` crate key.

## CONCLUSION: CCF writing needs an SBL (not in a local SDD)

Research + on-car test + RE converge: writing the CCF on the L319 BCM downloads a
**Secondary Boot Loader (SBL)** to the module's RAM (via 34/36/37) and starts it;
only then are the erase 0xFF00 and the CCF download accepted. Without the SBL, the
primary bootloader rejects the erase with `7F 31 22` (conditionsNotCorrect) —
exactly what we see, even with a programming session (burst 18/18 `0x50`),
security ok and the correct erase format.

Evidence:
- `CSessionCcf` (decompiled): the CCF path has `DownloadGBL`, `CreateSBLSession`,
  `GetSBLNode`, "Download GBL to get control of subnetwork".
- Community: r/CarHacking "Updating CCF on car - SBL needed"; SBLs live on JLR's
  online server, not in the SDD.
- Local install: only the BCM's `_CCF` (CARCFG) VBF exists; no L319 SBL/GBL (the
  only SBL/GBL present are L322 IGWM, X150/X250 ICM, and some generic ones in
  IDS/Flash).

Real paths to unlock writing:
1. Obtain the L319 BCM SBL (JLR TOPIx online, or from the community). With the SBL
   VBF, the flow becomes: download SBL -> start -> erase -> write CCF, all offline.
2. Capture via J2534 shim with the SDD running ONLINE (it fetches the SBL and
   writes; we copy the exact sequence).
3. Commercial tools (Abrites AVDI, Diatronik Pluto) already ship the SBLs.

All CCF READING/decoding and diagnostics (VIN, modules, DTCs, live) already work;
only WRITING depends on the SBL.
