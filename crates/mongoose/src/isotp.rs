//! Remontagem ISO-TP (ISO 15765-2) do lado do host.
//!
//! O adaptador gera os quadros de controle de fluxo sozinho; cabe ao host
//! remontar as respostas multi-quadro. Portado fielmente do `isotp.cpp` de
//! `dgcarr/Mongoose-USB-2-OBDII-Linux-Driver` (LGPL-2.1), que foi validado
//! contra uma captura real de leitura de VIN. Função pura do fluxo de quadros,
//! sem temporização nem tráfego, logo testável sem veículo.
//!
//! Cada quadro de entrega é: 4 bytes do CAN ID (big-endian) seguidos dos bytes
//! de dados CAN. Uma conversa é mantida por CAN ID de origem.

/// Bit de RxStatus: início de uma mensagem multi-quadro (first frame).
pub const START_OF_MESSAGE: u32 = 0x02;

const IDENTIFIER: usize = 4;
const SINGLE_LIMIT: usize = 7;
const SEGMENTED_MINIMUM: usize = 8;
/// Limite do fornecedor para uma mensagem remontada (10021070 rejeita >= 0x1000).
pub const MAX_MESSAGE: usize = 0xfff;
const MAX_CONVERSATIONS: usize = 32;

/// Saída da remontagem: uma mensagem completa ou uma indicação de início.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// 0 para mensagem completa, [`START_OF_MESSAGE`] para first-frame.
    pub rx_status: u32,
    /// 4 bytes do CAN ID (big-endian) + payload remontado (ou só o ID numa
    /// indicação).
    pub data: Vec<u8>,
}

struct Assembly {
    id: u32,
    message: Vec<u8>,
    expected: usize,
    sequence: u8,
}

/// Remontador ISO-TP. Mantém uma conversa por CAN ID de origem.
#[derive(Default)]
pub struct IsoTpReassembler {
    assemblies: Vec<Assembly>,
}

fn source_id(frame: &[u8]) -> u32 {
    (frame[0] as u32) << 24 | (frame[1] as u32) << 16 | (frame[2] as u32) << 8 | frame[3] as u32
}

fn with_identifier(frame: &[u8]) -> Vec<u8> {
    frame[..IDENTIFIER].to_vec()
}

impl IsoTpReassembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.assemblies.clear();
    }

    fn find(&self, id: u32) -> Option<usize> {
        self.assemblies.iter().position(|a| a.id == id)
    }

    fn drop_id(&mut self, id: u32) {
        if let Some(i) = self.find(id) {
            self.assemblies.remove(i);
        }
    }

    /// Alimenta um quadro de entrega (ID big-endian + bytes CAN) e devolve as
    /// saídas produzidas.
    pub fn feed(&mut self, frame: &[u8]) -> Vec<Output> {
        if frame.len() <= IDENTIFIER {
            return Vec::new();
        }
        let ptype = frame[IDENTIFIER] >> 4;
        let id = source_id(frame);
        if self.find(id).is_none() {
            return match ptype {
                0 => self.single(frame),
                1 => self.first(frame),
                _ => Vec::new(),
            };
        }
        match ptype {
            0 => self.single(frame),
            1 => {
                self.drop_id(id);
                self.first(frame)
            }
            2 => self.consecutive(frame),
            _ => Vec::new(),
        }
    }

    fn single(&mut self, frame: &[u8]) -> Vec<Output> {
        let id = source_id(frame);
        if frame.len() < IDENTIFIER + 1 {
            self.drop_id(id);
            return Vec::new();
        }
        let length = (frame[IDENTIFIER] & 0x0f) as usize;
        if length > SINGLE_LIMIT || length > frame.len() - IDENTIFIER - 1 {
            self.drop_id(id);
            return Vec::new();
        }
        let mut data = with_identifier(frame);
        data.extend_from_slice(&frame[IDENTIFIER + 1..IDENTIFIER + 1 + length]);
        vec![Output { rx_status: 0, data }]
    }

    fn first(&mut self, frame: &[u8]) -> Vec<Output> {
        if frame.len() < IDENTIFIER + 2 {
            return Vec::new();
        }
        let announced = ((frame[IDENTIFIER] & 0x0f) as usize) << 8 | frame[IDENTIFIER + 1] as usize;
        if announced < SEGMENTED_MINIMUM || announced > MAX_MESSAGE {
            return Vec::new();
        }
        let mut message = with_identifier(frame);
        let payload = &frame[IDENTIFIER + 2..];
        let taken = payload.len().min(announced);
        message.extend_from_slice(&payload[..taken]);
        if self.assemblies.len() == MAX_CONVERSATIONS {
            self.assemblies.remove(0);
        }
        self.assemblies.push(Assembly {
            id: source_id(frame),
            message,
            expected: announced,
            sequence: 1,
        });
        vec![Output {
            rx_status: START_OF_MESSAGE,
            data: with_identifier(frame),
        }]
    }

    fn consecutive(&mut self, frame: &[u8]) -> Vec<Output> {
        let id = source_id(frame);
        let Some(idx) = self.find(id) else {
            return Vec::new();
        };
        if frame.len() < IDENTIFIER + 2 {
            self.assemblies.remove(idx);
            return Vec::new();
        }
        if (frame[IDENTIFIER] & 0x0f) != self.assemblies[idx].sequence {
            // Lacuna de sequência: o resto da mensagem não é confiável.
            self.assemblies.remove(idx);
            return Vec::new();
        }
        self.assemblies[idx].sequence = (self.assemblies[idx].sequence + 1) & 0x0f;
        let payload = &frame[IDENTIFIER + 1..];
        let acc = self.assemblies[idx].message.len() - IDENTIFIER;
        let outstanding = self.assemblies[idx].expected - acc;
        let taken = payload.len().min(outstanding);
        self.assemblies[idx]
            .message
            .extend_from_slice(&payload[..taken]);
        if self.assemblies[idx].message.len() - IDENTIFIER < self.assemblies[idx].expected {
            return Vec::new();
        }
        let done = self.assemblies.remove(idx);
        vec![Output {
            rx_status: 0,
            data: done.message,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(id: u32, bytes: &[u8]) -> Vec<u8> {
        let mut f = id.to_be_bytes().to_vec();
        f.extend_from_slice(bytes);
        f
    }

    #[test]
    fn single_frame() {
        let mut r = IsoTpReassembler::new();
        // 0x03 = single frame, 3 bytes: 62 F1 90 (resposta UDS)
        let out = r.feed(&frame(0x7E8, &[0x03, 0x62, 0xF1, 0x90]));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rx_status, 0);
        assert_eq!(out[0].data, vec![0x00, 0x00, 0x07, 0xE8, 0x62, 0xF1, 0x90]);
    }

    #[test]
    fn multi_frame_vin() {
        let mut r = IsoTpReassembler::new();
        // VIN: 0x22 F190 -> resposta 62 F1 90 + 17 bytes, total 20 bytes de payload.
        // First frame: 10 14 (announced=0x014=20), depois 6 bytes.
        let ff = r.feed(&frame(
            0x7E8,
            &[0x10, 0x14, 0x62, 0xF1, 0x90, b'S', b'A', b'L'],
        ));
        assert_eq!(ff.len(), 1);
        assert_eq!(ff[0].rx_status, START_OF_MESSAGE);
        // Consecutive 1 (0x21): 7 bytes
        let c1 = r.feed(&frame(
            0x7E8,
            &[0x21, b'L', b'A', b'A', b'A', b'A', b'A', b'A'],
        ));
        assert!(c1.is_empty());
        // Consecutive 2 (0x22): 7 bytes -> completa (20 payload)
        let c2 = r.feed(&frame(
            0x7E8,
            &[0x22, b'B', b'B', b'B', b'B', b'B', b'B', b'B'],
        ));
        assert_eq!(c2.len(), 1);
        assert_eq!(c2[0].rx_status, 0);
        // 4 (ID) + 20 (payload)
        assert_eq!(c2[0].data.len(), 24);
        assert_eq!(&c2[0].data[4..7], &[0x62, 0xF1, 0x90]);
    }

    #[test]
    fn sequencia_fora_de_ordem_descarta() {
        let mut r = IsoTpReassembler::new();
        r.feed(&frame(0x7E8, &[0x10, 0x14, 1, 2, 3, 4, 5, 6]));
        // Esperava 0x21, veio 0x22: descarta a conversa.
        let out = r.feed(&frame(0x7E8, &[0x22, 9, 9, 9, 9, 9, 9, 9]));
        assert!(out.is_empty());
        assert!(r.find(0x7E8).is_none(), "conversa deve ter sido descartada");
    }
}
