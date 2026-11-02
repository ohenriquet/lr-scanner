//! Cliente de alto nível sobre um [`Transport`](crate::Transport).
//!
//! Cuida da numeração de sequência, do envio enquadrado e da remontagem da
//! resposta a partir de um fluxo de bytes (o CDC-ACM pode fragmentar ou
//! concatenar quadros). A lógica é pura e genérica sobre o transporte, para
//! ser testável sem hardware com um transporte falso.

use std::time::{Duration, Instant};

use crate::frame::{self, Decoded, Message};
use crate::opcode::{self, cmd};
use crate::Transport;

/// Erros do cliente.
#[derive(Debug, thiserror::Error)]
pub enum ClientError<E: std::fmt::Debug> {
    #[error("transporte: {0:?}")]
    Transport(E),
    #[error("enquadramento: {0}")]
    Frame(#[from] frame::FrameError),
    #[error("tempo esgotado esperando resposta")]
    Timeout,
    #[error("resposta com opcode {got:#06x}, esperado {want:#06x}")]
    Unexpected { want: u16, got: u16 },
    #[error("o dispositivo relatou status {0:#010x}")]
    DeviceStatus(u32),
}

/// Informações de placa devolvidas por `cGetBoardInfo`.
#[derive(Debug, Clone)]
pub struct BoardInfo {
    /// Status bruto (`corpo+12`).
    pub status: u32,
    /// Bytes de dados da resposta (layout específico do firmware).
    pub data: Vec<u8>,
}

/// Cliente que conversa com o adaptador através de um transporte.
pub struct Device<T: Transport> {
    transport: T,
    seq: u16,
    rx: Vec<u8>,
    /// Tempo máximo para montar uma resposta completa.
    pub timeout: Duration,
}

impl<T: Transport> Device<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            seq: 1,
            rx: Vec::with_capacity(4096),
            timeout: Duration::from_millis(1500),
        }
    }

    fn next_seq(&mut self) -> u16 {
        let s = self.seq;
        self.seq = self.seq.wrapping_add(1).max(1);
        s
    }

    /// Envia uma mensagem já montada.
    fn send(&mut self, msg: &Message) -> Result<(), ClientError<T::Error>> {
        let wire = msg.encode()?;
        self.transport
            .write_all(&wire)
            .map_err(ClientError::Transport)
    }

    /// Lê até montar um quadro completo (ou estourar o tempo).
    fn recv(&mut self) -> Result<Message, ClientError<T::Error>> {
        let deadline = Instant::now() + self.timeout;
        let mut tmp = [0u8; 1024];
        loop {
            // Já há um quadro completo no buffer?
            match frame::try_decode(&self.rx)? {
                Decoded::Frame { body, consumed } => {
                    self.rx.drain(..consumed);
                    return Ok(Message::decode_body(&body)?);
                }
                Decoded::NeedMore => {}
            }
            if Instant::now() >= deadline {
                return Err(ClientError::Timeout);
            }
            let n = self
                .transport
                .read(&mut tmp)
                .map_err(ClientError::Transport)?;
            if n > 0 {
                self.rx.extend_from_slice(&tmp[..n]);
            }
        }
    }

    /// Envia uma requisição e espera a resposta com o opcode esperado
    /// (`req | 0x8000`). Mensagens de indicação assíncrona (opcodes sem o bit
    /// de resposta) são ignoradas enquanto se espera.
    pub fn transact(
        &mut self,
        dest: u16,
        opcode: u16,
        data: Vec<u8>,
    ) -> Result<Message, ClientError<T::Error>> {
        let seq = self.next_seq();
        let want = opcode::response_of(opcode);
        self.send(&Message::request(dest, opcode, seq, data))?;

        let deadline = Instant::now() + self.timeout;
        loop {
            let msg = self.recv()?;
            if msg.opcode == want {
                return Ok(msg);
            }
            // Resposta de outro opcode ou indicação: ignora e segue.
            if Instant::now() >= deadline {
                return Err(ClientError::Unexpected {
                    want,
                    got: msg.opcode,
                });
            }
        }
    }

    /// `cEchoPacket`: devolve o payload ecoado pelo dispositivo.
    pub fn echo(&mut self, payload: &[u8]) -> Result<Vec<u8>, ClientError<T::Error>> {
        let resp = self.transact(frame::DEST_DEVICE, cmd::ECHO_PACKET, payload.to_vec())?;
        Ok(resp.data)
    }

    /// `cOpenDevice`: zera o relógio do dispositivo e prepara-o.
    pub fn open_device(&mut self) -> Result<(), ClientError<T::Error>> {
        self.transact(frame::DEST_DEVICE, cmd::OPEN_DEVICE, vec![])?;
        Ok(())
    }

    /// `cGetBoardInfo`: identifica a placa.
    pub fn board_info(&mut self) -> Result<BoardInfo, ClientError<T::Error>> {
        let resp = self.transact(frame::DEST_DEVICE, cmd::GET_BOARD_INFO, vec![])?;
        let status = read_u32(&resp.data, 0).unwrap_or(0);
        let data = resp.data.get(8..).unwrap_or(&[]).to_vec();
        Ok(BoardInfo { status, data })
    }

    /// Acesso ao transporte interno (para fechar, etc.).
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }
}

fn read_u32(buf: &[u8], off: usize) -> Option<u32> {
    buf.get(off..off + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Message;
    use std::collections::VecDeque;

    /// Transporte falso: responde a cada requisição ecoando o opcode com o
    /// bit de resposta e devolvendo os dados enviados.
    struct LoopbackEcho {
        pending: VecDeque<u8>,
    }

    impl LoopbackEcho {
        fn new() -> Self {
            Self {
                pending: VecDeque::new(),
            }
        }
    }

    impl Transport for LoopbackEcho {
        type Error = std::convert::Infallible;

        fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
            // Interpreta o que o cliente mandou e enfileira uma resposta.
            let n = frame::parse_prefix(data).unwrap();
            let body = &data[4..4 + n];
            let req = Message::decode_body(body).unwrap();
            let resp = Message {
                dest: frame::NODE_PC,
                source: req.dest,
                opcode: req.opcode | crate::opcode::RESPONSE_FLAG,
                seq: req.seq,
                data: req.data.clone(),
            };
            for b in resp.encode().unwrap() {
                self.pending.push_back(b);
            }
            Ok(())
        }

        fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            let mut i = 0;
            while i < buf.len() {
                match self.pending.pop_front() {
                    Some(b) => {
                        buf[i] = b;
                        i += 1;
                    }
                    None => break,
                }
            }
            Ok(i)
        }
    }

    #[test]
    fn echo_ida_e_volta_com_transporte_falso() {
        let mut dev = Device::new(LoopbackEcho::new());
        let got = dev.echo(&[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
        assert_eq!(got, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn transact_casa_opcode_de_resposta() {
        let mut dev = Device::new(LoopbackEcho::new());
        let resp = dev
            .transact(frame::DEST_DEVICE, cmd::GET_BOARD_INFO, vec![1, 2, 3])
            .unwrap();
        assert_eq!(resp.opcode, 0x8109);
        assert_eq!(resp.data, vec![1, 2, 3]);
    }

    #[test]
    fn sequencia_nunca_zero() {
        let mut dev = Device::new(LoopbackEcho::new());
        dev.seq = u16::MAX;
        let s1 = dev.next_seq();
        let s2 = dev.next_seq();
        assert_eq!(s1, u16::MAX);
        assert_eq!(s2, 1, "após estourar, pula o zero");
    }
}
