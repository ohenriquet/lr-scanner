# Mongoose Pro JLR protocol — notes and validation

Implemented in the `mongoose` crate. Based on the `PROTOCOL.md` from
`dgcarr/Mongoose-USB-2-OBDII-Linux-Driver` (LGPL-2.1) and validated against the
user's physical adapter.

## Key finding: on Windows it is a COM port (CDC-ACM)

Plugged into Windows 11, the adapter (USB `18E1:0104`, "Drew Technologies Inc.")
enumerates as a **USB Serial Device** bound to the built-in `usbser` driver,
creating a COM port (e.g. COM5). **No vendor KMDF driver nor Visual Studio is
required.** We speak the native protocol directly over the serial port, the same
path as Linux (`/dev/ttyACM*`) and macOS (`/dev/cu.usbmodem*`). This simplifies
the port on every platform and makes Android possible over USB-host CDC-ACM.

## Framing (implemented in `frame.rs`)

- Transport prefix (4 bytes): body length `N` as `u16` LE, then `N XOR 0x51E6`
  as `u16` LE. `1 <= N <= 0x1800`.
- Body header (12 bytes): dest `u16` LE, source `u16` LE, opcode `u16` LE, seq
  `u16` LE, 4 reserved bytes. Data after byte 12.
- Replies: `req | 0x8000`; from `body+12` there is a `u32` status, `body+16` a
  microsecond counter, `body+20` the data (or error text).

## Validated read handshake (bench, no car)

`mongoose-bench --port COM5`, user's adapter (serial AOLHE0000003666A):

| Command | Opcode | Result |
|---|---|---|
| cEchoPacket | 0x0100 | echoed `01 02 03 04` exactly |
| cOpenDevice | 0x0003 | status 0x00000000 |
| cGetBoardInfo | 0x0109 | status 0, board data (below) |

Board info (data after status+timestamp), observed:
`05 02 01 01 B4 B0 00 00 D8 55 00 00 00 08 01 01 00 10 01 01 ...`
The blocks `00 10 01 01` and `00 08 01 01` correspond to firmware 1.1.16.0 and
bootloader 1.1.8.0 (SDD's `GatewayManager.ini` lists "FW:1.1.16.0 BL:1.1.8.0").
The full layout will be formalized when the CAN part is implemented.

## Pending (needs the car or extra captures)

- Open a CAN channel (0x0006, channel `0x0501`) and ISO15765 (`0x0601`), baud and
  flags.
- cSetPin (0x0012): body `(1,6,14)` for HS-CAN (pins 6/14); the body for MS-CAN
  (pins 3/11), where the CCF broadcast lives, is not known yet — obtain it from a
  USB capture of the official driver (USBPcap).
- cOutboundData / cInboundData (0x0008 / 0x0009), ISO-TP flow-control filters
  (cTableAddEntry 0x000D, selector 2).
- Battery voltage reading (cIoctl / cSetPin).

## ISO-TP transmit limit: 128 bytes (found on the car)

Validated on the Discovery 4 BCM: the adapter's ISO-TP transmit (a protocol-6
`cOutboundData`) completes up to **128 payload bytes**; from ~130 bytes on, the
multi-frame transmit stalls and the module receives nothing (no reply). Multi-
frame RECEIVE works (reading 1 KB from DID 0xDE00 works), because the adapter
generates flow control on its own. It is a 128-byte TX buffer in the firmware.

Consequence: writing the CCF (1024 bytes) with a single `0x2E DE00` is impossible
with the firmware stack. Options: (a) implement the ISO-TP TRANSMIT in software
over a raw CAN channel (protocol 5), slicing into frames and handling the module's
flow control (BS/STmin, multiple blocks) — what the official DLL does; (b) use the
block download sequence (0x34/0x36/0x37) with TransferData ≤128 bytes. The BCM's
SecurityAccess is already validated on the car (seed/key accepted).
