//! Protocolo do adaptador **Mongoose Pro JLR** (J2534, USB `18E1:0104`).
//!
//! Este crate implementa o enquadramento e os comandos do adaptador sobre um
//! transporte abstrato ([`Transport`]), para que o mesmo protocolo rode sobre
//! USB bulk (Windows/Android) ou serial CDC-ACM (macOS/Linux). Deriva de
//! engenharia reversa publicada sob LGPL-2.1
//! (`dgcarr/Mongoose-USB-2-OBDII-Linux-Driver`); ver `LICENSE-LGPL.txt`.
//!
//! Estado: o codec de [`frame`] está implementado e testado a partir da
//! especificação. A camada de transporte e os comandos de alto nível serão
//! validados em bancada com o adaptador físico (Fase 3).

pub mod can;
pub mod client;
pub mod frame;
pub mod isotp;
pub mod isotp_sw;
pub mod opcode;

/// Transporte físico de bytes para o adaptador.
///
/// As implementações concretas (USB bulk, serial) vivem fora deste crate ou
/// atrás de features, para manter o núcleo do protocolo testável sem hardware.
pub trait Transport {
    type Error: std::fmt::Debug;

    /// Envia todos os bytes para o endpoint de saída (bulk OUT `0x01`).
    fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error>;

    /// Lê até `buf.len()` bytes do endpoint de entrada (bulk IN `0x82`),
    /// devolvendo quantos leu.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
}
