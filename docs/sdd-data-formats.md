# JLR SDD data formats (reverse-engineering notes)

Reference for the `sdd-data` crate parsers. Based on the SDD v165 install at
`C:\Program Files (x86)\JLR`. Paths are relative to that root.

## `.exml` encryption

3DES (EDE, three keys) in ECB mode, no IV, PKCS#5 padding. Public 24-byte key
(see the `sdd-crypto` crate). Confirmed against the plaintext/ciphertext pair
`IDS\Xml\JLR_VSV_CONFIG.xml` ↔ `.exml` and against the whole `IDS\Xml` folder.
`SDD\Xml` is a copy of `IDS\Xml` (same SHA-256 per file).

## `PLATFORM_<VEH>_<YEAR>.exml` (schema `platform.xsd`)

Topology and addressing of a platform. For the L319 MY10:

- `network_architecture > network`: `CAN_HS` (500 kbps), `CAN_MS` (125 kbps) and
  `SUB_MOST` (gateway via the ACM) networks. Each with diagnostic protocol
  `ISO14229` (UDS) and network protocol `ISO15765`, `normal` addressing (11 bits).
- `car_configuration`: defines the CCF master source and the backups.
  - `master > ccf_source upload="did"`: `BCM_SYSTEM_A` (the master CCF is
    read/written by DID on the BCM).
  - `backup`: `PCM_SYSTEM_A`, with `upload="did"` or `upload="mem"` depending on
    the engine type (`V8NA`, `V6NA`, `V6DIESEL/3L`, `V6DIESEL/2.7L`).
  - `as_built > ccf_source upload="vbf"`: `AS_BUILT`.
- `module_fitment > module` (53 modules): each with
  `module_code_name acronym=... ISOName=...`, addresses
  `address type="can_tx|can_rx" session="diag|prog"` (e.g. ABS tx `0x760`,
  rx `0x768`), `network`, DID sets (`data_identifier_set`), `fitment`
  (mandatory/optional) and a title `tm id="@..."`.

This file is the primary source of the `module` table (addresses, bus, sessions)
and of the vehicle→modules link.

## `MDX_<MOD>.exml` and `<MOD>.exml` (DTD `MDX-002.0.dtd`)

Per-module definition. `IDS\Xml\<VEH>_<YEAR>\`. Structure:

- `ADMINISTRATION`: ECU name, shortname.
- `ECU_DATA > DATA_IDENTIFIERS > DID`: each DID has `NUMBER` (e.g. `0xDD01`),
  `BYTE_SIZE`, `DID_TYPE` (`single_value`, `packeted`), `ACCESS_PARAMETERS`
  (`READABLE`/`WRITEABLE` with `SERVICE_REFS` = `service_22`/`service_2E`,
  `SESSION_REFS`, `SECURITY_REFS`) and `SUB_FIELD` with `LEAST_SIG_BIT`/
  `MOST_SIG_BIT` and `DATA_DEFINITION` (type, `NUMERIC_PARAMETERS` with
  `RESOLUTION`/`OFFSET`/`UNITS`/`RANGE_*`, or `ENUMERATED_PARAMETERS`).
- Also contains `DTC`, `ROUTINE` (start/stop/results parameters) and per-module
  protocol/security data (addresses, sessions, levels).

Source of the `did` table (number, subfield, bits, scaling, unit, enum) and
`routine`.

## `CCF_DATA_<VEH>_<YEAR>.exml` (schema `ccf_data.xsd`)

**The CCF bit map** — the core of configuration editing. For the L319 MY10 the
header cites "SA5AC/01/96 Rev 4.01, EUCD 31830271 Rev 6", `vbf_version 2.0`.

- `block id="1" name="CCF" serviceIdRd="0x22" serviceIdWr="0x2E"`: the CCF is
  read with ReadDataByIdentifier and written with WriteDataByIdentifier.
  - `address module=... t_ccf="L319_201000_BCM_CCF.vbf" parameterId="0xDE00"
    start_address="0x00FFAC00" stop_address="0x00FFACC3" merge="true"`:
    per module/engine, which VBF template, which DID (`0xDE00` on the BCM,
    `0xF106` on the PCM) and the address range. The block is 196 bytes
    (`length="196"`).
  - `group start=N stop=M name="GROUP_CCF_..."`: groups parameters by byte(s).
  - `parameter id="BBB_BBB_bbb_bbb" name="PARAM_CCF_..." mask="0xFF"
    type="ENUM|BOOL|..."`: the `id` encodes start byte, end byte, start bit and
    end bit (e.g. `047_047_002_002` = byte 47, bits 2..2, mask `0x04`).
    `category display/edit/scope` controls visibility/editing.
    `select > option value="0x.." name=".." code=".."`: possible values, with
    `code` = factory option codes (WERS) and `tm id="@..."` = text.

Comfort-lighting examples on the L319 MY10 (confirmed):
`PARAM_CCF_INT_LIGHT_APPROACH_LAMP` (byte 47 bit 2, BOOL),
`PARAM_CCF_EXT_LIGHT_FOLLOW_ME_HOME_REVERSE` (byte 46 bit 7, BOOL),
`PARAM_CCF_EUCD_PUDDLE_LIGHT_OUTER_MIRRORS` (byte 86, ENUM: none/both/driver/
passenger), `PARAM_CCF_EUCD_HOMESAFE_LIGHTING` (byte 148, ENUM: off/30..180 s).

Source of the `ccf_block`, `ccf_param`, `ccf_option` tables.

## `Security.exml` (schema `Security.xsd`)

Per-module-group SecurityAccess data: constants (`DataConstant`) and groups
(`SecDataGroup Index=.. Title="L319_<MOD>" ProgDataRef=.. DiagDataRef=..`) that
reference the programming and diagnostic constants. Combined with the seed-key
levels/services from the MDX files and the disassembly of `SecAlg.dll` to
reconstruct the 0x27 algorithm (phase 5). **Sensitive content: do not version or
display the constants.**

## `CONFIG_MENU_<VEH>_<YEAR>.exml` / `MDX_PROFILE_*` / `CCF_RULES_*`

`CONFIG_MENU`: menus of the configuration functions (items, `job_list`,
`funcName`, SPA test). `MDX_PROFILE`: routine profiles (`run_routine`,
`routine_start_parameters`, `rw_identifier`, `sec_level`). `CCF_RULES`:
synchronization/consistency rules between CCF parameters. Sources of the routines
(phase 6) and of CCF validation (phase 5).

## `VINDecode.exml` / `VEHICLE_MANIFEST*`

Decodes the VIN into model/year/engine; the manifest links variant→files. Used to
detect the vehicle from the read VIN (`0x22 F190`).

## Supporting flat files (outside `.exml`)

- `Common\SDD\Data\ids\MODEL_YEAR_BREAKPOINTS.xml`: VIN-year → `MYxx`/`<year>`
  folder. L319: 2005-09→MY05/200500, 2010-11→MY10/201000, 2012-13→MY12/201200,
  2014-16→MY14/201400.
- `ProgramData\JLR\IDS\OnlinePIDHelp\Data\PROCS\mdl.xml`: dictionary of 3,956
  DIDs (name, bits, modules, vehicle lines, engine codes).
- `Common\SDD\Data\<lang>\gradex\Converters\CVT_*.xml`: scaling/enumeration
  converters for live data.
- `Common\SDD\Data\<lang>\gradex\Snapshot\*DID Formatting.xml`: DID bit-field
  layout with a link to the converter.
- `IDS\Xml\text\@<L>\@<MNEMONIC>.xml`: texts by mnemonic, 15 languages (incl.
  PTG). Resolves the `tm id="@..."` of the files above.
- `IDS\Xml\gdx.xml`: DTC text by number.
- `Common\Flash\Vbf\*_CCF.vbf`: VBF templates `sw_part_type=CARCFG` (flat header)
  to assemble the CCF write package.

## CCF VBF templates (L319 MY10)

| Module | File | ecu_address | start..stop | erase |
|---|---|---|---|---|
| BCM (master) | `L319_201000_BCM_CCF.vbf` | 0x726 | 0x00FFAC00..C3 | 0x00FFAC00 len 0x400 |
| PCM diesel 3L | `L319_201000_PCM_DIESEL_3L_CCF.vbf` | 0x7E0 | 0x80800000..C3 | 0x80800000 len 0x2000 |
| GWM (MY14) | `L319_201400_GWM_CCF.vbf` | 0x716 | — | — |
