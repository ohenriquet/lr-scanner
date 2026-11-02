//! Enquadramento do protocolo USB do Mongoose Pro JLR.
//!
//! Baseado no `PROTOCOL.md` de `dgcarr/Mongoose-USB-2-OBDII-Linux-Driver`
//! (LGPL-2.1). Cada mensagem tem:
//!
//! 1. Um prefixo de transporte de 4 bytes: comprimento `N` do corpo como
//!    `u16` little-endian, seguido de `N XOR 0x51E6` como `u16` LE (checagem,
//!    não soma de verificação). `1 <= N <= 0x1800`.
//! 2. Um corpo que começa com um cabeçalho de 12 bytes:
//!    - `0..2`  destino (roteamento A). Requisições ao dispositivo usam `1`;
//!      requisições a um canal usam `(protocolo<<8)|placa`.
//!    - `2..4`  origem (roteamento B). O PC é o nó `0`.
//!    - `4..6`  opcode (`u16` LE).
//!    - `6..8`  sequência (`u16` LE); as respostas a ecoam.
//!    - `8..12` normalmente zero (campo `chan`/reservado).
//!    - `12..`  dados do comando.
//!
//! As respostas têm, a partir de `corpo+12`, um `u32` de status; `corpo+16`
//! um contador de microssegundos do dispositivo; `corpo+20` os dados de
//! retorno (ou texto ASCII de erro em caso de falha).

/// Constante de verificação do prefixo de transporte.
pub const LEN_XOR: u16 = 0x51E6;

/// Tamanho do cabeçalho do corpo, em bytes.
pub const HEADER_LEN: usize = 12;

/// Comprimento máximo do corpo aceito pelo dispositivo.
pub const MAX_BODY: usize = 0x1800;

/// Nó do PC (origem das requisições).
pub const NODE_PC: u16 = 0;

/// Destino das requisições ao próprio dispositivo (não a um canal).
pub const DEST_DEVICE: u16 = 1;

/// Erros de enquadramento.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("corpo com {0} bytes fora de 1..={MAX_BODY}")]
    BadLength(usize),
    #[error("prefixo curto: {0} bytes (mínimo 4)")]
    ShortPrefix(usize),
    #[error("verificação do comprimento falhou: len={len:#06x} check={check:#06x}")]
    BadLenCheck { len: u16, check: u16 },
    #[error("buffer tem {have} bytes, esperados {need} para o corpo")]
    Truncated { have: usize, need: usize },
    #[error("corpo menor que o cabeçalho de {HEADER_LEN} bytes: {0}")]
    BodyTooSmall(usize),
}

/// Um corpo de mensagem já decomposto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub dest: u16,
    pub source: u16,
    pub opcode: u16,
    pub seq: u16,
    /// Dados após o cabeçalho de 12 bytes.
    pub data: Vec<u8>,
}

impl Message {
    /// Cria uma requisição do PC (origem 0, bytes 8..12 zerados).
    pub fn request(dest: u16, opcode: u16, seq: u16, data: Vec<u8>) -> Self {
        Self {
            dest,
            source: NODE_PC,
            opcode,
            seq,
            data,
        }
    }

    /// Serializa o corpo (cabeçalho de 12 bytes + dados).
    pub fn encode_body(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(HEADER_LEN + self.data.len());
        b.extend_from_slice(&self.dest.to_le_bytes());
        b.extend_from_slice(&self.source.to_le_bytes());
        b.extend_from_slice(&self.opcode.to_le_bytes());
        b.extend_from_slice(&self.seq.to_le_bytes());
        b.extend_from_slice(&[0u8; 4]); // reservado
        b.extend_from_slice(&self.data);
        b
    }

    /// Serializa a mensagem completa (prefixo de transporte + corpo).
    pub fn encode(&self) -> Result<Vec<u8>, FrameError> {
        let body = self.encode_body();
        frame_body(&body)
    }

    /// Decompõe um corpo (sem o prefixo de transporte) em [`Message`].
    pub fn decode_body(body: &[u8]) -> Result<Self, FrameError> {
        if body.len() < HEADER_LEN {
            return Err(FrameError::BodyTooSmall(body.len()));
        }
        Ok(Self {
            dest: u16::from_le_bytes([body[0], body[1]]),
            source: u16::from_le_bytes([body[2], body[3]]),
            opcode: u16::from_le_bytes([body[4], body[5]]),
            seq: u16::from_le_bytes([body[6], body[7]]),
            data: body[HEADER_LEN..].to_vec(),
        })
    }
}

/// Envolve um corpo já serializado no prefixo de transporte de 4 bytes.
pub fn frame_body(body: &[u8]) -> Result<Vec<u8>, FrameError> {
    let n = body.len();
    if n == 0 || n > MAX_BODY {
        return Err(FrameError::BadLength(n));
    }
    let len = n as u16;
    let mut out = Vec::with_capacity(4 + n);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(len ^ LEN_XOR).to_le_bytes());
    out.extend_from_slice(body);
    Ok(out)
}

/// Lê um prefixo de transporte e devolve o comprimento `N` do corpo,
/// validando a constante de verificação.
pub fn parse_prefix(buf: &[u8]) -> Result<usize, FrameError> {
    if buf.len() < 4 {
        return Err(FrameError::ShortPrefix(buf.len()));
    }
    let len = u16::from_le_bytes([buf[0], buf[1]]);
    let check = u16::from_le_bytes([buf[2], buf[3]]);
    if (len ^ LEN_XOR) != check {
        return Err(FrameError::BadLenCheck { len, check });
    }
    let n = len as usize;
    if n == 0 || n > MAX_BODY {
        return Err(FrameError::BadLength(n));
    }
    Ok(n)
}

/// Resultado de tentar extrair um quadro de um fluxo de bytes.
#[derive(Debug, PartialEq, Eq)]
pub enum Decoded {
    /// Um corpo completo foi extraído; o `usize` é quantos bytes consumir.
    Frame { body: Vec<u8>, consumed: usize },
    /// Faltam bytes; é preciso ler mais do transporte.
    NeedMore,
}

/// Tenta extrair um quadro do começo de `buf`. Não consome o buffer; o
/// chamador avança `consumed` bytes quando recebe [`Decoded::Frame`].
pub fn try_decode(buf: &[u8]) -> Result<Decoded, FrameError> {
    if buf.len() < 4 {
        return Ok(Decoded::NeedMore);
    }
    let n = parse_prefix(buf)?;
    let total = 4 + n;
    if buf.len() < total {
        return Ok(Decoded::NeedMore);
    }
    Ok(Decoded::Frame {
        body: buf[4..total].to_vec(),
        consumed: total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixo_tem_xor_de_verificacao() {
        // Corpo de 12 bytes -> len=12, check = 12 ^ 0x51E6.
        let framed = frame_body(&[0u8; 12]).unwrap();
        assert_eq!(&framed[0..2], &12u16.to_le_bytes());
        assert_eq!(&framed[2..4], &(12u16 ^ LEN_XOR).to_le_bytes());
        assert_eq!(framed.len(), 16);
    }

    #[test]
    fn parse_prefix_valida_e_rejeita() {
        let framed = frame_body(&[0u8; 20]).unwrap();
        assert_eq!(parse_prefix(&framed).unwrap(), 20);
        // Corrompe o byte de verificação.
        let mut bad = framed.clone();
        bad[2] ^= 0xFF;
        assert!(matches!(
            parse_prefix(&bad),
            Err(FrameError::BadLenCheck { .. })
        ));
    }

    #[test]
    fn rejeita_comprimentos_invalidos() {
        assert_eq!(frame_body(&[]), Err(FrameError::BadLength(0)));
        let big = vec![0u8; MAX_BODY + 1];
        assert_eq!(frame_body(&big), Err(FrameError::BadLength(MAX_BODY + 1)));
    }

    #[test]
    fn roundtrip_mensagem() {
        let msg = Message::request(DEST_DEVICE, 0x0109, 7, vec![1, 2, 3, 4]);
        let wire = msg.encode().unwrap();
        // Decodifica via try_decode e decode_body.
        let dec = try_decode(&wire).unwrap();
        let Decoded::Frame { body, consumed } = dec else {
            panic!("esperava um quadro completo");
        };
        assert_eq!(consumed, wire.len());
        let back = Message::decode_body(&body).unwrap();
        assert_eq!(back, msg);
        assert_eq!(back.source, NODE_PC);
    }

    #[test]
    fn try_decode_pede_mais_quando_incompleto() {
        let msg = Message::request(DEST_DEVICE, 0x0003, 1, vec![9, 9, 9, 9, 9, 9]);
        let wire = msg.encode().unwrap();
        // Só o prefixo: faltam os dados.
        assert_eq!(try_decode(&wire[..4]).unwrap(), Decoded::NeedMore);
        // Nem o prefixo inteiro.
        assert_eq!(try_decode(&wire[..3]).unwrap(), Decoded::NeedMore);
        // Completo menos um byte.
        assert_eq!(
            try_decode(&wire[..wire.len() - 1]).unwrap(),
            Decoded::NeedMore
        );
    }

    #[test]
    fn decode_body_rejeita_corpo_curto() {
        assert_eq!(
            Message::decode_body(&[0u8; 8]),
            Err(FrameError::BodyTooSmall(8))
        );
    }
}
