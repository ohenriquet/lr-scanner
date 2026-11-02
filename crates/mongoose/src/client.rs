//! Cliente de alto nível sobre um [`Transport`](crate::Transport).
//!
//! Cuida da numeração de sequência, do envio enquadrado e da remontagem da
//! resposta a partir de um fluxo de bytes (o CDC-ACM pode fragmentar ou
//! concatenar quadros). A lógica é pura e genérica sobre o transporte, para
//! ser testável sem hardware com um transporte falso.

use std::time::{Duration, Instant};

use crate::can::{self, InboundFrame};
use crate::frame::{self, Decoded, Message, CHAN_DATA, CHAN_MGMT};
use crate::isotp::IsoTpReassembler;
use crate::opcode::{self, cmd};
use crate::Transport;

/// Status `cOutboundData`: quadro enfileirado com sucesso.
pub const STATUS_QUEUED: u32 = 0x100;
/// Status `cOutboundData`: buffer de transmissão cheio.
pub const STATUS_BUFFER_FULL: u32 = 0x101;
/// Status `cJumpToFirmware`: já estava rodando o firmware (não é erro).
pub const STATUS_ALREADY_FIRMWARE: u32 = 7;

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

/// Uma resposta ISO-TP remontada de um módulo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsoTpResponse {
    /// CAN ID de origem (ex.: 0x7E8).
    pub can_id: u32,
    /// Bytes de serviço UDS/OBD (ex.: `62 F1 90 ...`).
    pub service: Vec<u8>,
}

/// Protocolo atualmente conectado no canal CAN/ISO-TP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelProtocol {
    Can,
    IsoTp,
}

/// Cliente que conversa com o adaptador através de um transporte.
pub struct Device<T: Transport> {
    transport: T,
    seq: u16,
    rx: Vec<u8>,
    /// Protocolo do canal aberto (`None` se nenhum).
    protocol: Option<ChannelProtocol>,
    /// Remontador ISO-TP das respostas recebidas.
    reasm: IsoTpReassembler,
    /// Mensagens ISO-TP já remontadas, à espera de serem consumidas.
    reassembled: Vec<crate::isotp::Output>,
    /// Tempo máximo para montar uma resposta completa.
    pub timeout: Duration,
}

impl<T: Transport> Device<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            seq: 1,
            rx: Vec::with_capacity(4096),
            protocol: None,
            reasm: IsoTpReassembler::new(),
            reassembled: Vec::new(),
            timeout: Duration::from_millis(1500),
        }
    }

    /// Próxima sequência no intervalo 1..=255 (exigência do protocolo).
    fn next_seq(&mut self) -> u16 {
        let s = self.seq;
        self.seq = if self.seq >= 255 { 1 } else { self.seq + 1 };
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
        self.transact_chan(dest, opcode, CHAN_MGMT, data)
    }

    /// Como [`Device::transact`], com `chan` explícito. Casa a resposta pelo
    /// opcode (`req | 0x8000`) e pela sequência; quadros de dados (opcode 9)
    /// que cheguem no meio são roteados para o remontador ISO-TP.
    pub fn transact_chan(
        &mut self,
        dest: u16,
        opcode: u16,
        chan: u16,
        data: Vec<u8>,
    ) -> Result<Message, ClientError<T::Error>> {
        let seq = self.next_seq();
        let want = opcode::response_of(opcode);
        self.send(&Message::request_chan(dest, opcode, seq, chan, data))?;

        let deadline = Instant::now() + self.timeout;
        loop {
            let msg = self.recv()?;
            if msg.opcode == want && msg.seq == seq {
                return Ok(msg);
            }
            // Dados que chegam antes da resposta: alimenta o remontador.
            if msg.opcode == cmd::INBOUND_DATA {
                self.route_inbound(&msg);
            }
            if Instant::now() >= deadline {
                return Err(ClientError::Unexpected {
                    want,
                    got: msg.opcode,
                });
            }
        }
    }

    /// Roteia um corpo de `cInboundData` para o remontador ISO-TP.
    fn route_inbound(&mut self, msg: &Message) {
        // `parse_inbound` espera o corpo inteiro (com cabeçalho); reconstrói-o.
        let body = msg.encode_body();
        if let Ok(InboundFrame { data, .. }) = can::parse_inbound(&body) {
            // Alimenta e guarda as saídas no buffer de mensagens remontadas.
            let outs = self.reasm.feed(&data);
            self.reassembled.extend(outs);
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

    /// Status geral de uma resposta de comando (`corpo+12` = `data[0..4]`).
    fn status_of(resp: &Message) -> u32 {
        read_u32(&resp.data, 0).unwrap_or(0)
    }

    /// Sequência de abertura do dispositivo: cJumpToFirmware (tolera "já em
    /// firmware"), cOpenDevice, cGetBoardInfo.
    pub fn open(&mut self) -> Result<BoardInfo, ClientError<T::Error>> {
        // cJumpToFirmware pode responder 0 ou 7 (já em firmware).
        let jump = self.transact(frame::DEST_DEVICE, cmd::JUMP_TO_FIRMWARE, vec![])?;
        let st = Self::status_of(&jump);
        if st != 0 && st != STATUS_ALREADY_FIRMWARE {
            return Err(ClientError::DeviceStatus(st));
        }
        self.open_device()?;
        self.board_info()
    }

    /// Nó de canal `(protocolo<<8)|1`.
    fn channel_node(proto: u16) -> u16 {
        opcode::channel_dest(proto, 1)
    }

    /// Abre o canal ISO15765 (ISO-TP) e seleciona os pinos 6/14 (HS-CAN).
    pub fn connect_iso_tp(&mut self, baud: u32) -> Result<(), ClientError<T::Error>> {
        let node = Self::channel_node(can::PROTO_ISO15765);
        let open = self.transact(node, cmd::OPEN_CHANNEL, can::open_channel_payload(0, baud))?;
        Self::expect_ok(&open)?;
        let pin = self.transact(node, cmd::SET_PIN, can::set_pin_hs_can_payload())?;
        Self::expect_ok(&pin)?;
        self.protocol = Some(ChannelProtocol::IsoTp);
        self.reasm.reset();
        self.reassembled.clear();
        Ok(())
    }

    /// Adiciona um filtro de controle de fluxo ISO-TP e devolve o handle do
    /// firmware. Necessário para o adaptador responder a respostas multi-quadro.
    pub fn add_flow_control_filter(
        &mut self,
        response_id: u16,
        request_id: u16,
    ) -> Result<u32, ClientError<T::Error>> {
        let node = Self::channel_node(can::PROTO_ISO15765);
        let payload = can::flow_control_filter_payload(response_id, request_id);
        let resp = self.transact(node, cmd::TABLE_ADD_ENTRY, payload)?;
        Self::expect_ok(&resp)?;
        // Handle em corpo+20 = data[8..12].
        read_u32(&resp.data, 8).ok_or(ClientError::DeviceStatus(0xFFFF_FFFF))
    }

    /// Envia uma requisição ISO-TP (`request_id` + bytes de serviço) e coleta
    /// as respostas remontadas por até `collect` de tempo. Cada item devolvido
    /// são os bytes de serviço da resposta (sem o CAN ID), com o CAN ID de
    /// origem à parte.
    pub fn request_iso_tp(
        &mut self,
        request_id: u16,
        service: &[u8],
        collect: Duration,
    ) -> Result<Vec<IsoTpResponse>, ClientError<T::Error>> {
        let node = Self::channel_node(can::PROTO_ISO15765);
        let payload = can::isotp_transmit_payload(request_id, service, 250);
        let resp = self.transact_chan(node, cmd::OUTBOUND_DATA, CHAN_DATA, payload)?;
        let st = Self::status_of(&resp);
        if st == STATUS_BUFFER_FULL {
            return Err(ClientError::DeviceStatus(st));
        }
        if st != 0 && st != STATUS_QUEUED {
            return Err(ClientError::DeviceStatus(st));
        }

        // Coleta respostas por um tempo fixo.
        let deadline = Instant::now() + collect;
        let mut tmp = [0u8; 1024];
        loop {
            // Lê o que houver sem estourar o relógio geral.
            if Instant::now() >= deadline {
                break;
            }
            // Tenta montar e rotear quadros já no buffer.
            match frame::try_decode(&self.rx)? {
                Decoded::Frame { body, consumed } => {
                    self.rx.drain(..consumed);
                    if let Ok(msg) = Message::decode_body(&body) {
                        if msg.opcode == cmd::INBOUND_DATA {
                            self.route_inbound(&msg);
                        }
                    }
                    continue;
                }
                Decoded::NeedMore => {}
            }
            let n = self
                .transport
                .read(&mut tmp)
                .map_err(ClientError::Transport)?;
            if n > 0 {
                self.rx.extend_from_slice(&tmp[..n]);
            }
        }

        // Converte as mensagens completas (rx_status 0) em respostas.
        let mut out = Vec::new();
        for o in std::mem::take(&mut self.reassembled) {
            if o.rx_status != 0 {
                continue; // indicações de início não interessam aqui
            }
            if o.data.len() < 4 {
                continue;
            }
            let can_id = u32::from_be_bytes([o.data[0], o.data[1], o.data[2], o.data[3]]);
            out.push(IsoTpResponse {
                can_id,
                service: o.data[4..].to_vec(),
            });
        }
        Ok(out)
    }

    fn expect_ok(resp: &Message) -> Result<(), ClientError<T::Error>> {
        let st = Self::status_of(resp);
        if st == 0 {
            Ok(())
        } else {
            Err(ClientError::DeviceStatus(st))
        }
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
                chan: req.chan,
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
    fn sequencia_cicla_1_a_255() {
        let mut dev = Device::new(LoopbackEcho::new());
        dev.seq = 255;
        assert_eq!(dev.next_seq(), 255);
        assert_eq!(dev.next_seq(), 1, "após 255, volta a 1 (nunca 0)");
    }
}
