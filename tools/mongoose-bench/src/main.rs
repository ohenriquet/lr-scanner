//! Bancada do Mongoose Pro JLR por porta serial (CDC-ACM).
//!
//! Abre a porta (ex.: COM5 no Windows, /dev/ttyACM0 no Linux,
//! /dev/cu.usbmodem* no macOS) e executa um handshake de leitura: eco,
//! abrir dispositivo e informações de placa. Não toca na parte CAN (não
//! precisa de carro).

use std::io::{Read, Write};
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use mongoose::client::Device;
use mongoose::Transport;

/// Transporte serial sobre a crate `serialport`.
struct SerialTransport {
    port: Box<dyn serialport::SerialPort>,
}

impl SerialTransport {
    fn open(name: &str, baud: u32) -> Result<Self> {
        let port = serialport::new(name, baud)
            .timeout(Duration::from_millis(200))
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .open()
            .with_context(|| format!("abrir {name}"))?;
        // CDC-ACM costuma exigir DTR para o host "assumir" a linha.
        let mut port = port;
        let _ = port.write_data_terminal_ready(true);
        let _ = port.write_request_to_send(true);
        Ok(Self { port })
    }
}

impl Transport for SerialTransport {
    type Error = std::io::Error;

    fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.port.write_all(data)?;
        self.port.flush()
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        match self.port.read(buf) {
            Ok(n) => Ok(n),
            // Timeout de leitura = sem dados agora; o cliente tenta de novo.
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(0),
            Err(e) => Err(e),
        }
    }
}

#[derive(Parser)]
#[command(name = "mongoose-bench", about = "Handshake de bancada com o Mongoose")]
struct Cli {
    /// Nome da porta serial (ex.: COM5, /dev/ttyACM0). Dispensável com --list.
    #[arg(long, required_unless_present = "list")]
    port: Option<String>,
    /// Baud (CDC-ACM geralmente ignora, mas alguns stacks exigem um valor).
    #[arg(long, default_value_t = 115200)]
    baud: u32,
    /// Lista as portas disponíveis e sai.
    #[arg(long)]
    list: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.list {
        for p in serialport::available_ports().unwrap_or_default() {
            println!("{}  {:?}", p.port_name, p.port_type);
        }
        return Ok(());
    }

    let port = cli.port.expect("clap garante --port quando não é --list");
    println!("Abrindo {} @ {} baud...", port, cli.baud);
    let transport = SerialTransport::open(&port, cli.baud)?;
    let mut dev = Device::new(transport);
    dev.timeout = Duration::from_millis(2000);

    // 1) Eco: prova o enquadramento ida-e-volta sem efeitos colaterais.
    print!("cEchoPacket... ");
    std::io::stdout().flush().ok();
    match dev.echo(&[0x01, 0x02, 0x03, 0x04]) {
        Ok(d) => println!("OK, ecoou {} bytes: {}", d.len(), hexdump(&d)),
        Err(e) => println!("falhou: {e}"),
    }

    // 2) Abrir dispositivo (zera o relógio).
    print!("cOpenDevice... ");
    std::io::stdout().flush().ok();
    match dev.open_device() {
        Ok(()) => println!("OK"),
        Err(e) => println!("falhou: {e}"),
    }

    // 3) Informações de placa.
    print!("cGetBoardInfo... ");
    std::io::stdout().flush().ok();
    match dev.board_info() {
        Ok(info) => {
            println!("OK, status={:#010x}", info.status);
            println!("    dados: {}", hexdump(&info.data));
            if let Ok(txt) = std::str::from_utf8(&info.data) {
                let printable: String = txt.chars().filter(|c| !c.is_control()).collect();
                if printable.len() > 3 {
                    println!("    texto: {printable}");
                }
            }
        }
        Err(e) => println!("falhou: {e}"),
    }

    Ok(())
}

fn hexdump(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
