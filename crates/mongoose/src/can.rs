//! Comandos de canal CAN/ISO15765 do Mongoose.
//!
//! Construtores dos payloads exatos (abrir canal, selecionar pinos, filtro de
//! controle de fluxo, envio ISO-TP) e o parser dos quadros recebidos. Os
//! formatos de byte vêm do `can.cpp` de
//! `dgcarr/Mongoose-USB-2-OBDII-Linux-Driver` (LGPL-2.1), validado em veículo.

/// Protocolo CAN cru (J2534 ProtocolID 5).
pub const PROTO_CAN: u16 = 5;
/// Protocolo ISO15765/ISO-TP (J2534 ProtocolID 6).
pub const PROTO_ISO15765: u16 = 6;

/// Flag J2534 `ISO15765_FRAME_PAD` (preenche quadros curtos a 8 bytes).
pub const ISO15765_FRAME_PAD: u32 = 0x40;

/// Seletor de tabela do filtro de controle de fluxo ISO-TP.
pub const TABLE_FLOW_CONTROL: u8 = 2;

/// Payload de `cOpenChannel` (opcode 6): `flags` (u32 LE) seguido de `baud`
/// (u32 LE). Enviado ao nó de canal `(protocolo<<8)|1` com `chan=0`.
pub fn open_channel_payload(flags: u32, baud: u32) -> Vec<u8> {
    let mut p = Vec::with_capacity(8);
    p.extend_from_slice(&flags.to_le_bytes());
    p.extend_from_slice(&baud.to_le_bytes());
    p
}

/// Payload de `cSetPin` (opcode 0x12) para HS-CAN nos pinos 6 e 14 do conector
/// OBD-II: três u32 LE `(1, 6, 14)`. O `1` inicial é do protocolo do fornecedor.
pub fn set_pin_hs_can_payload() -> Vec<u8> {
    let mut p = Vec::with_capacity(12);
    for word in [1u32, 6, 14] {
        p.extend_from_slice(&word.to_le_bytes());
    }
    p
}

/// Payload do filtro de controle de fluxo ISO-TP (`cTableAddEntry`, opcode
/// 0x0d). `response_id` é o CAN ID com que o módulo responde (ex.: 0x7E8);
/// `request_id` é para onde o adaptador manda o controle de fluxo (ex.: 0x7E0).
/// Ambos são IDs de 11 bits. 19 bytes, byte a byte como o fornecedor envia.
pub fn flow_control_filter_payload(response_id: u16, request_id: u16) -> Vec<u8> {
    let flags = ISO15765_FRAME_PAD;
    let mut p = vec![0u8; 19];
    p[0] = TABLE_FLOW_CONTROL;
    p[4] = flags as u8;
    p[5] = (flags >> 8) as u8;
    p[6] = (flags >> 16) as u8;
    p[7] = (flags >> 24) as u8;
    // pattern (response id) big-endian em p[9..13]
    let resp = (response_id as u32).to_be_bytes();
    p[9..13].copy_from_slice(&resp);
    // flow (request id) big-endian em p[14..18]
    let req = (request_id as u32).to_be_bytes();
    p[14..18].copy_from_slice(&req);
    p
}

/// Payload de `cOutboundData` (opcode 8) para ISO-TP. `request_id` é o CAN ID
/// de destino (ex.: 0x7E0 físico, ou 0x7DF funcional); `service` são os bytes
/// UDS/OBD (ex.: `[0x22, 0xF1, 0x90]`). Enviado com `chan=1`.
pub fn isotp_transmit_payload(request_id: u16, service: &[u8], timeout_ms: u32) -> Vec<u8> {
    let flags = ISO15765_FRAME_PAD;
    let data_size = (4 + service.len()) as u16;
    let mut p = vec![0u8; 12];
    p[0] = flags as u8;
    p[1] = (flags >> 8) as u8;
    p[2] = (flags >> 16) as u8;
    p[3] = (flags >> 24) as u8;
    p[4] = timeout_ms as u8;
    p[5] = (timeout_ms >> 8) as u8;
    p[6] = (timeout_ms >> 16) as u8;
    p[7] = (timeout_ms >> 24) as u8;
    p[8] = data_size as u8;
    p[9] = (data_size >> 8) as u8;
    // p[10..12] = 0
    // dados: CAN ID big-endian (4 bytes) + serviço
    p.extend_from_slice(&(request_id as u32).to_be_bytes());
    p.extend_from_slice(service);
    p
}

/// Um quadro CAN recebido (opcode 9), antes da remontagem ISO-TP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundFrame {
    pub rx_status: u32,
    pub timestamp: u32,
    /// Dados: CAN ID big-endian (4 bytes) + bytes CAN.
    pub data: Vec<u8>,
}

/// Erro ao interpretar um corpo de entrada.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum InboundError {
    #[error("corpo de entrada curto: {0} bytes")]
    Short(usize),
    #[error("tamanho de dados inválido: {0}")]
    BadSize(u16),
}

/// Interpreta um corpo de `cInboundData` (opcode 9). Layout a partir do corpo:
/// `+12` rx_status (u32), `+16` timestamp (u32), `+22` data_size (u16),
/// `+24` dados. Espelha o receptor do `can.cpp`.
pub fn parse_inbound(body: &[u8]) -> Result<InboundFrame, InboundError> {
    if body.len() < 24 {
        return Err(InboundError::Short(body.len()));
    }
    let rx_status = u32::from_le_bytes([body[12], body[13], body[14], body[15]]);
    let timestamp = u32::from_le_bytes([body[16], body[17], body[18], body[19]]);
    let size = u16::from_le_bytes([body[22], body[23]]);
    if !(4..=12).contains(&size) || body.len() != 24 + size as usize {
        return Err(InboundError::BadSize(size));
    }
    Ok(InboundFrame {
        rx_status,
        timestamp,
        data: body[24..24 + size as usize].to_vec(),
    })
}

/// Opcode do corpo (corpo+4), para distinguir dados (9) de indicações (10).
pub fn body_opcode(body: &[u8]) -> Option<u16> {
    body.get(4..6).map(|b| u16::from_le_bytes([b[0], b[1]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtro_fc_bate_com_o_fornecedor() {
        // "02000000 40000000 00 000007e8 00 000007e0 00"
        let p = flow_control_filter_payload(0x7E8, 0x7E0);
        let esperado = [
            0x02, 0, 0, 0, 0x40, 0, 0, 0, 0, 0x00, 0x00, 0x07, 0xE8, 0, 0x00, 0x00, 0x07, 0xE0, 0,
        ];
        assert_eq!(p, esperado);
    }

    #[test]
    fn open_channel_flags_e_baud() {
        let p = open_channel_payload(0, 500_000);
        assert_eq!(&p[0..4], &[0, 0, 0, 0]);
        assert_eq!(&p[4..8], &500_000u32.to_le_bytes());
    }

    #[test]
    fn set_pin_hs_can() {
        let p = set_pin_hs_can_payload();
        assert_eq!(p, vec![1, 0, 0, 0, 6, 0, 0, 0, 14, 0, 0, 0]);
    }

    #[test]
    fn transmit_vin_request() {
        // 0x22 F1 90 para 0x7E0
        let p = isotp_transmit_payload(0x7E0, &[0x22, 0xF1, 0x90], 250);
        assert_eq!(p[0], 0x40); // ISO15765_FRAME_PAD
        assert_eq!(u16::from_le_bytes([p[8], p[9]]), 7); // data_size = 4 + 3
        assert_eq!(&p[12..16], &[0x00, 0x00, 0x07, 0xE0]); // ID big-endian
        assert_eq!(&p[16..19], &[0x22, 0xF1, 0x90]);
    }

    #[test]
    fn parse_inbound_single_frame() {
        // Monta um corpo de opcode 9 com 8 bytes de dados (ID + 03 62 F1 90 ...)
        let mut body = vec![0u8; 24];
        body[4] = 9; // opcode
        body[22] = 8; // data_size
        let data = [0x00, 0x00, 0x07, 0xE8, 0x03, 0x62, 0xF1, 0x90];
        body.extend_from_slice(&data);
        let f = parse_inbound(&body).unwrap();
        assert_eq!(f.data, data);
    }
}
