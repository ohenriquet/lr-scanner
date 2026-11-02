#!/usr/bin/env python3
"""Resume o capture.log do shim: a sequência UDS trocada com cada ECU.

Uso: python analyze.py [C:\\ProgramData\\jlr_shim\\capture.log]
Destaca o BCM (0x726/0x72E) e nomeia os serviços UDS.
"""
import sys
import re

LOG = sys.argv[1] if len(sys.argv) > 1 else r"C:\ProgramData\jlr_shim\capture.log"

SERVICES = {
    0x10: "DiagnosticSessionControl", 0x11: "ECUReset", 0x14: "ClearDTC",
    0x19: "ReadDTCInformation", 0x22: "ReadDataByIdentifier",
    0x23: "ReadMemoryByAddress", 0x27: "SecurityAccess", 0x28: "CommunicationControl",
    0x2E: "WriteDataByIdentifier", 0x2F: "InputOutputControl", 0x31: "RoutineControl",
    0x34: "RequestDownload", 0x35: "RequestUpload", 0x36: "TransferData",
    0x37: "RequestTransferExit", 0x3E: "TesterPresent", 0x3D: "WriteMemoryByAddress",
    0x85: "ControlDTCSetting",
}


def name(sid):
    if sid >= 0x40 and (sid - 0x40) in SERVICES:
        return f"resp {SERVICES[sid - 0x40]}"
    if sid == 0x7F:
        return "NEGATIVE"
    return SERVICES.get(sid, f"0x{sid:02X}")


def main():
    try:
        lines = open(LOG, encoding="utf-8", errors="replace").read().splitlines()
    except FileNotFoundError:
        print(f"Log não encontrado: {LOG}")
        return
    cur_id = None
    for ln in lines:
        m = re.search(r"\b(TX|RX)\b.*data=([0-9A-Fa-f]+)", ln)
        if not m:
            if "Connect" in ln or "Filter" in ln or "FLOW" in ln:
                print(ln.strip())
            continue
        direction, data = m.group(1), m.group(2)
        b = bytes.fromhex(data)
        if len(b) < 5:
            continue
        # ISO15765 J2534: CAN ID são os 4 primeiros bytes (big-endian).
        can_id = int.from_bytes(b[:4], "big")
        payload = b[4:]
        if not payload:
            continue
        sid = payload[0]
        tag = "BCM" if can_id in (0x726, 0x72E) else f"{can_id:#05X}"
        extra = ""
        if sid in (0x10, 0x27, 0x31, 0x34, 0x36, 0x37) and len(payload) > 1:
            extra = f" sub/arg={payload[1]:#04x}"
        if sid in (0x22, 0x2E) and len(payload) >= 3:
            extra = f" DID={payload[1] << 8 | payload[2]:#06x}"
        star = "  <<<" if sid in (0x2E, 0x34, 0x36, 0x37, 0x31) else ""
        print(f"[{direction}] {tag:>6}  {name(sid):<24} len={len(payload):<4} {payload[:16].hex()}{extra}{star}")


if __name__ == "__main__":
    main()
