//! Opcodes do protocolo Mongoose e constantes de canal.
//!
//! Valores conforme o `PROTOCOL.md` de
//! `dgcarr/Mongoose-USB-2-OBDII-Linux-Driver`. Alguns status/opcodes de
//! resposta observados no hardware (0x8101, 0x8107, 0x0200) não constam das
//! tabelas do fornecedor; o hardware é a autoridade.

/// Opcodes de comando (PC -> dispositivo).
#[allow(non_upper_case_globals)]
pub mod cmd {
    pub const OPEN_DEVICE: u16 = 0x0003;
    pub const CLOSE_DEVICE: u16 = 0x0005;
    pub const OPEN_CHANNEL: u16 = 0x0006;
    pub const CLOSE_CHANNEL: u16 = 0x0007;
    pub const OUTBOUND_DATA: u16 = 0x0008;
    pub const INBOUND_DATA: u16 = 0x0009;
    pub const INDICATION: u16 = 0x000A;
    pub const SET_VALUE: u16 = 0x000B;
    pub const GET_VALUE: u16 = 0x000C;
    pub const TABLE_ADD_ENTRY: u16 = 0x000D;
    pub const TABLE_REMOVE_ENTRY: u16 = 0x000E;
    pub const TABLE_MODIFY_ENTRY: u16 = 0x000F;
    pub const TABLE_CLEAR: u16 = 0x0010;
    pub const IOCTL: u16 = 0x0011;
    pub const SET_PIN: u16 = 0x0012;
    pub const GET_STRING: u16 = 0x0013;
    pub const ECHO_PACKET: u16 = 0x0100;
    pub const GET_BOARD_STATUS: u16 = 0x0101;
    pub const RESET_BOARD: u16 = 0x0102;
    pub const JUMP_TO_FIRMWARE: u16 = 0x0103;
    pub const GET_BOARD_INFO: u16 = 0x0109;
    pub const CHECK_CRN: u16 = 0x0111;
}

/// Bit OR-ado no opcode para formar o opcode de resposta (`req | 0x8000`).
pub const RESPONSE_FLAG: u16 = 0x8000;

/// Opcode de resposta correspondente a uma requisição.
pub fn response_of(req: u16) -> u16 {
    req | RESPONSE_FLAG
}

/// Protocolos de canal (byte alto do endereço de canal).
#[allow(non_upper_case_globals)]
pub mod protocol {
    /// CAN cru.
    pub const CAN: u16 = 5;
    /// ISO15765 (ISO-TP).
    pub const ISO15765: u16 = 6;
}

/// Monta o endereço de destino de um canal: `(protocolo << 8) | placa`.
///
/// A placa principal é `1`; assim CAN -> `0x0501`, ISO15765 -> `0x0601`.
pub fn channel_dest(proto: u16, board: u8) -> u16 {
    (proto << 8) | board as u16
}

/// Flag J2534 `CAN_29BIT_ID`.
pub const CAN_29BIT_ID: u32 = 0x100;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enderecos_de_canal_conhecidos() {
        assert_eq!(channel_dest(protocol::CAN, 1), 0x0501);
        assert_eq!(channel_dest(protocol::ISO15765, 1), 0x0601);
    }

    #[test]
    fn opcode_de_resposta() {
        assert_eq!(response_of(cmd::GET_BOARD_INFO), 0x8109);
        assert_eq!(response_of(cmd::OPEN_DEVICE), 0x8003);
    }
}
