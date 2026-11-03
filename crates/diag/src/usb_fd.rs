//! Transporte USB-OTG (Android): fala com o Mongoose Pro JLR (que no Linux/Android
//! é CDC-ACM) a partir de um *file descriptor* entregue pelo `UsbManager`, via
//! `nusb` (`Device::from_fd`). Um reader de fundo lê o bulk IN continuamente para
//! um buffer; `read()` apenas drena (não bloqueia), como o cliente mongoose exige.
//!
//! EXPERIMENTAL — os números de interface/endpoint abaixo são o layout padrão de
//! CDC-ACM (comm=0, data=1, bulk OUT 0x01 / IN 0x82) e precisam ser confirmados no
//! dispositivo real; ajuste aqui se a conexão falhar.
#![cfg(target_os = "android")]

use std::collections::VecDeque;
use std::os::fd::{BorrowedFd, RawFd};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use futures_lite::future::block_on;
use mongoose::Transport;
use nusb::transfer::{ControlOut, ControlType, Recipient, RequestBuffer};

const COMM_IFACE: u8 = 0; // interface de controle (SET_CONTROL_LINE_STATE)
const DATA_IFACE: u8 = 1; // interface de dados (bulk)
const EP_OUT: u8 = 0x01; // bulk OUT (host -> device)
const EP_IN: u8 = 0x82; // bulk IN  (device -> host)

const CDC_SET_LINE_CODING: u8 = 0x20;
const CDC_SET_CONTROL_LINE_STATE: u8 = 0x22;

#[derive(Debug)]
pub struct UsbError(pub String);
impl std::fmt::Display for UsbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for UsbError {}

pub struct UsbFdTransport {
    data: nusb::Interface,
    rx: Arc<Mutex<VecDeque<u8>>>,
    stop: Arc<Mutex<bool>>,
    _reader: JoinHandle<()>,
}

impl UsbFdTransport {
    /// Abre a partir de um fd do `UsbManager`. `baud` ajusta o line coding CDC;
    /// o baud do CAN é definido depois pelo protocolo Mongoose.
    pub fn open_fd(fd: RawFd, baud: u32) -> Result<Self, String> {
        // Duplicamos o fd: o Rust fica dono de uma cópia e o lado Android (Kotlin)
        // continua dono do original — cada um fecha o seu, sem double-close.
        // SAFETY: `fd` é um fd válido e aberto vindo do UsbManager.
        let owned = unsafe { BorrowedFd::borrow_raw(fd) }
            .try_clone_to_owned()
            .map_err(|e| format!("dup fd: {e}"))?;
        let device = nusb::Device::from_fd(owned).map_err(|e| format!("from_fd: {e}"))?;

        // Interface de controle: SET_LINE_CODING + DTR/RTS (como no serial).
        let comm = device
            .claim_interface(COMM_IFACE)
            .map_err(|e| format!("claim comm: {e}"))?;
        let mut lc = Vec::with_capacity(7);
        lc.extend_from_slice(&baud.to_le_bytes()); // dwDTERate
        lc.push(0); // bCharFormat: 1 stop bit
        lc.push(0); // bParityType: none
        lc.push(8); // bDataBits: 8
        let _ = block_on(comm.control_out(ControlOut {
            control_type: ControlType::Class,
            recipient: Recipient::Interface,
            request: CDC_SET_LINE_CODING,
            value: 0,
            index: COMM_IFACE as u16,
            data: &lc,
        }));
        let _ = block_on(comm.control_out(ControlOut {
            control_type: ControlType::Class,
            recipient: Recipient::Interface,
            request: CDC_SET_CONTROL_LINE_STATE,
            value: 0x0003, // DTR | RTS
            index: COMM_IFACE as u16,
            data: &[],
        }));

        // Interface de dados: bulk IN/OUT.
        let data = device
            .claim_interface(DATA_IFACE)
            .map_err(|e| format!("claim data: {e}"))?;

        let rx = Arc::new(Mutex::new(VecDeque::<u8>::new()));
        let stop = Arc::new(Mutex::new(false));
        let rx2 = Arc::clone(&rx);
        let stop2 = Arc::clone(&stop);
        let rd = data.clone();
        let reader = std::thread::spawn(move || loop {
            if *stop2.lock().unwrap() {
                break;
            }
            let comp = block_on(rd.bulk_in(EP_IN, RequestBuffer::new(512)));
            if comp.status.is_err() {
                break; // erro/desconexão
            }
            if !comp.data.is_empty() {
                rx2.lock().unwrap().extend(comp.data.iter().copied());
            }
        });

        Ok(Self {
            data,
            rx,
            stop,
            _reader: reader,
        })
    }
}

impl Drop for UsbFdTransport {
    fn drop(&mut self) {
        *self.stop.lock().unwrap() = true;
    }
}

impl Transport for UsbFdTransport {
    type Error = UsbError;

    fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let comp = block_on(self.data.bulk_out(EP_OUT, data.to_vec()));
        comp.status.map_err(|e| UsbError(format!("bulk_out: {e}")))
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut q = self.rx.lock().unwrap();
        let n = buf.len().min(q.len());
        for slot in buf.iter_mut().take(n) {
            *slot = q.pop_front().unwrap();
        }
        Ok(n)
    }
}
