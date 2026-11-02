//! Leitura do veículo via Mongoose (somente leitura): VIN, presença de
//! módulos e DTCs. Comunicação ISO-TP a 500 kbps nos pinos 6/14 (HS-CAN); o
//! gateway encaminha os módulos de MS-CAN.
//!
//! Este utilitário NÃO escreve nada no veículo. Limpar DTCs é um comando à
//! parte, explícito.

use std::io::{Read, Write};
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mongoose::client::Device;
use mongoose::Transport;

/// Tabela de módulos do L319 MY10 (sigla, id de requisição, id de resposta).
/// Extraída do PLATFORM do SDD. HS-CAN direto; MS-CAN via gateway.
const MODULES: &[(&str, u16, u16)] = &[
    ("IPC", 0x720, 0x728),
    ("VIM", 0x722, 0x72A),
    ("BCM", 0x726, 0x72E),
    ("ACM", 0x727, 0x72F),
    ("KVM", 0x731, 0x739),
    ("HVAC", 0x733, 0x73B),
    ("HCM", 0x734, 0x73C),
    ("PAM", 0x736, 0x73E),
    ("RCM", 0x737, 0x73F),
    ("DDM", 0x740, 0x748),
    ("PDM", 0x741, 0x749),
    ("DSM", 0x744, 0x74C),
    ("PBM", 0x756, 0x75E),
    ("ABS", 0x760, 0x768),
    ("TCCM", 0x761, 0x769),
    ("CCM", 0x764, 0x76C),
    ("OCS", 0x765, 0x76D),
    ("SPRM", 0x772, 0x77A),
    ("ATCM", 0x792, 0x79A),
    ("RDCM", 0x795, 0x79D),
    ("SASM", 0x797, 0x79F),
    ("FCIMB", 0x7A0, 0x7A8),
    ("FCDIM", 0x7A5, 0x7AD),
    ("FCIM", 0x7A7, 0x7AF),
    ("IPMB", 0x7B1, 0x7B9),
    ("HCMB", 0x7C3, 0x7CB),
    ("RLM", 0x7D3, 0x7DB),
    ("PCM", 0x7E0, 0x7E8),
    ("TCM", 0x7E1, 0x7E9),
    ("AHCM", 0x7E3, 0x7EB),
];

struct SerialTransport {
    port: Box<dyn serialport::SerialPort>,
}

impl SerialTransport {
    fn open(name: &str, baud: u32) -> Result<Self> {
        let mut port = serialport::new(name, baud)
            .timeout(Duration::from_millis(60))
            .open()
            .with_context(|| format!("abrir {name}"))?;
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
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(0),
            Err(e) => Err(e),
        }
    }
}

#[derive(Parser)]
#[command(
    name = "car-scan",
    about = "Leitura do veículo via Mongoose (somente leitura)"
)]
struct Cli {
    /// Porta serial do Mongoose (ex.: COM5).
    #[arg(long)]
    port: String,
    /// Taxa do HS-CAN (padrão 500000).
    #[arg(long, default_value_t = 500_000)]
    baud: u32,
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Lê o VIN do módulo de powertrain (0x7E0).
    Vin,
    /// Varre quais módulos respondem (TesterPresent).
    Scan,
    /// Lê os DTCs de todos os módulos que respondem.
    Dtcs,
    /// VIN + varredura + DTCs, em sequência.
    All,
    /// Lê dados ao vivo do motor (OBD modo 01: temperaturas, tensão, RPM).
    Live,
    /// Limpa os DTCs. ESCREVE no veículo: exige --yes. Opcional --module SIGLA.
    Clear {
        /// Confirmação obrigatória para gravar.
        #[arg(long)]
        yes: bool,
        /// Limita a um módulo (sigla, ex.: PCM); sem isso, limpa todos.
        #[arg(long)]
        module: Option<String>,
    },
    /// Lê o CCF (configuração do veículo) do BCM e salva um backup. Leitura pura.
    Ccf {
        /// Arquivo de saída do backup bruto (.bin).
        #[arg(long, default_value = "ccf_backup.bin")]
        out: String,
    },
    /// Diagnóstico: envia bytes crus a um módulo e mostra todas as respostas.
    Raw {
        /// Sigla do módulo (ex.: BCM).
        #[arg(long)]
        module: String,
        /// Serviço em hex (ex.: 22DD01 ou 22DE00).
        #[arg(long)]
        hex: String,
        /// Entrar em sessão estendida (0x10 03) antes.
        #[arg(long)]
        extended: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    println!("Abrindo {} @ {} baud...", cli.port, cli.baud);
    let transport = SerialTransport::open(&cli.port, cli.baud)?;
    let mut dev = Device::new(transport);
    dev.timeout = Duration::from_millis(2500);

    let info = dev
        .open()
        .map_err(|e| anyhow::anyhow!("abrir dispositivo: {e:?}"))?;
    println!("Adaptador OK (status {:#010x}).", info.status);
    dev.connect_iso_tp(cli.baud)
        .map_err(|e| anyhow::anyhow!("conectar ISO-TP: {e:?}"))?;
    println!("Canal ISO15765 aberto, pinos 6/14.\n");

    match cli.cmd {
        Command::Vin => {
            read_vin(&mut dev)?;
        }
        Command::Scan => {
            scan(&mut dev)?;
        }
        Command::Dtcs => {
            let present = scan(&mut dev)?;
            read_all_dtcs(&mut dev, &present)?;
        }
        Command::All => {
            read_vin(&mut dev)?;
            println!();
            let present = scan(&mut dev)?;
            println!();
            read_all_dtcs(&mut dev, &present)?;
        }
        Command::Live => {
            read_live(&mut dev)?;
        }
        Command::Clear { yes, module } => {
            clear_dtcs(&mut dev, yes, module.as_deref())?;
        }
        Command::Ccf { out } => {
            read_ccf(&mut dev, &out)?;
        }
        Command::Raw {
            module,
            hex,
            extended,
        } => {
            raw(&mut dev, &module, &hex, extended)?;
        }
    }
    Ok(())
}

fn raw<T: Transport>(dev: &mut Device<T>, module: &str, hex: &str, extended: bool) -> Result<()> {
    let (_, req, resp) = *MODULES
        .iter()
        .find(|(name, _, _)| name.eq_ignore_ascii_case(module))
        .with_context(|| format!("módulo {module} desconhecido"))?;
    let bytes = parse_hex(hex)?;
    dev.add_flow_control_filter(resp, req)
        .map_err(|e| anyhow::anyhow!("filtro: {e:?}"))?;
    if extended {
        let s = dev
            .request_iso_tp(req, &[0x10, 0x03], Duration::from_millis(500))
            .unwrap_or_default();
        println!("sessão estendida: {} resposta(s)", s.len());
        for r in &s {
            println!("  de {:#05X}: {}", r.can_id, hexdump(&r.service));
        }
    }
    println!("Enviando a {module} (req {req:#05X}): {}", hexdump(&bytes));
    let answers = dev
        .request_iso_tp(req, &bytes, Duration::from_millis(1500))
        .map_err(|e| anyhow::anyhow!("enviar: {e:?}"))?;
    if answers.is_empty() {
        println!("  (sem resposta)");
    }
    for r in &answers {
        let kind = match r.service.first() {
            Some(0x7F) => " [resposta negativa]",
            _ => "",
        };
        println!(
            "  de {:#05X} ({} bytes){kind}: {}",
            r.can_id,
            r.service.len(),
            hexdump(&r.service)
        );
    }
    Ok(())
}

fn parse_hex(s: &str) -> Result<Vec<u8>> {
    let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    anyhow::ensure!(clean.len() % 2 == 0, "hex com número ímpar de dígitos");
    (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i + 2], 16).map_err(Into::into))
        .collect()
}

/// Lê o CCF do BCM (DID 0xDE00, serviço 0x22) e salva um backup bruto.
/// Tenta na sessão padrão; se recusado, entra em sessão estendida (0x10 03)
/// e repete. Operação de leitura (a troca de sessão é reversível).
fn read_ccf<T: Transport>(dev: &mut Device<T>, out: &str) -> Result<()> {
    const BCM_REQ: u16 = 0x726;
    const BCM_RESP: u16 = 0x72E;
    dev.add_flow_control_filter(BCM_RESP, BCM_REQ)
        .map_err(|e| anyhow::anyhow!("filtro BCM: {e:?}"))?;

    // O CCF tem ~1 KB (~147 quadros ISO-TP); a janela precisa ser generosa.
    let read = |dev: &mut Device<T>| {
        dev.request_iso_tp(BCM_REQ, &[0x22, 0xDE, 0x00], Duration::from_millis(2500))
            .unwrap_or_default()
    };

    let mut answers = read(dev);
    let mut positive = find_positive(&answers, BCM_RESP);

    if positive.is_none() {
        // Pode exigir sessão estendida. 0x10 03 é reversível.
        println!("CCF recusado na sessão padrão; tentando sessão estendida (0x10 03)...");
        let _ = dev.request_iso_tp(BCM_REQ, &[0x10, 0x03], Duration::from_millis(500));
        answers = read(dev);
        positive = find_positive(&answers, BCM_RESP);
        // Volta à sessão padrão.
        let _ = dev.request_iso_tp(BCM_REQ, &[0x10, 0x01], Duration::from_millis(400));
    }

    match positive {
        Some(data) => {
            // data = 62 DE 00 + bytes do CCF
            let ccf = &data[3..];
            println!("CCF lido do BCM: {} bytes.", ccf.len());
            std::fs::write(out, ccf).with_context(|| format!("salvar {out}"))?;
            println!("Backup salvo em {out}");
            // Primeiros bytes para conferência.
            let head = &ccf[..ccf.len().min(32)];
            println!("Início: {}", hexdump(head));
        }
        None => {
            println!("Não consegui ler o CCF. Respostas:");
            for r in &answers {
                println!("  de {:#05X}: {}", r.can_id, hexdump(&r.service));
            }
        }
    }
    Ok(())
}

fn find_positive(answers: &[mongoose::client::IsoTpResponse], resp_id: u16) -> Option<Vec<u8>> {
    answers
        .iter()
        .find(|r| {
            r.can_id == resp_id as u32
                && r.service.len() > 3
                && r.service[0] == 0x62
                && r.service[1] == 0xDE
                && r.service[2] == 0x00
        })
        .map(|r| r.service.clone())
}

/// Leitura de dados ao vivo via OBD-II modo 01 (padrão, somente leitura).
fn read_live<T: Transport>(dev: &mut Device<T>) -> Result<()> {
    // Responde quem tiver OBD: PCM em 0x7E8, pede-se a 0x7E0.
    dev.add_flow_control_filter(0x7E8, 0x7E0)
        .map_err(|e| anyhow::anyhow!("filtro OBD: {e:?}"))?;
    println!("Dados ao vivo (OBD modo 01):");
    // (PID, rótulo, decodificador)
    type Dec = fn(&[u8]) -> String;
    let pids: &[(u8, &str, Dec)] = &[
        (0x05, "Temp. líq. arrefecimento", |d| {
            format!("{} °C", d[0] as i32 - 40)
        }),
        (0x0F, "Temp. ar admissão", |d| {
            format!("{} °C", d[0] as i32 - 40)
        }),
        (0x0C, "Rotação", |d| {
            format!("{} rpm", ((d[0] as u32) << 8 | d[1] as u32) / 4)
        }),
        (0x0D, "Velocidade", |d| format!("{} km/h", d[0])),
        (0x42, "Tensão do módulo", |d| {
            format!(
                "{:.2} V",
                ((d[0] as u32) << 8 | d[1] as u32) as f32 / 1000.0
            )
        }),
        (0x2F, "Nível de combustível", |d| {
            format!("{:.0} %", d[0] as f32 * 100.0 / 255.0)
        }),
        (0x33, "Pressão barométrica", |d| format!("{} kPa", d[0])),
        (0x5C, "Temp. óleo motor", |d| {
            format!("{} °C", d[0] as i32 - 40)
        }),
    ];
    for &(pid, label, dec) in pids {
        let answers = dev
            .request_iso_tp(0x7E0, &[0x01, pid], Duration::from_millis(350))
            .unwrap_or_default();
        // Resposta: 41 <pid> <dados...>
        let val = answers.iter().find_map(|r| {
            if r.service.len() >= 3 && r.service[0] == 0x41 && r.service[1] == pid {
                Some(dec(&r.service[2..]))
            } else {
                None
            }
        });
        match val {
            Some(v) => println!("  {label:<26} {v}"),
            None => println!("  {label:<26} (sem suporte/sem resposta)"),
        }
    }
    Ok(())
}

/// Limpa DTCs (serviço UDS 0x14, grupo 0xFFFFFF = todos). ESCREVE no veículo.
fn clear_dtcs<T: Transport>(dev: &mut Device<T>, yes: bool, module: Option<&str>) -> Result<()> {
    if !yes {
        anyhow::bail!("limpar DTCs escreve no veículo. Repita com --yes para confirmar.");
    }
    let alvos: Vec<(&str, u16, u16)> = match module {
        Some(m) => MODULES
            .iter()
            .copied()
            .filter(|(name, _, _)| name.eq_ignore_ascii_case(m))
            .collect(),
        None => {
            // Só os que respondem, para não esperar por módulos ausentes.
            scan(dev)?
        }
    };
    if alvos.is_empty() {
        anyhow::bail!("nenhum módulo alvo (verifique a sigla).");
    }
    println!("\nLimpando DTCs (serviço 0x14, todos os grupos):");
    for (name, req, resp) in alvos {
        let _ = dev.add_flow_control_filter(resp, req);
        let answers = dev
            .request_iso_tp(req, &[0x14, 0xFF, 0xFF, 0xFF], Duration::from_millis(900))
            .unwrap_or_default();
        let ok = answers
            .iter()
            .any(|r| r.can_id == resp as u32 && r.service.first() == Some(&0x54));
        let neg = answers
            .iter()
            .find(|r| r.can_id == resp as u32 && r.service.first() == Some(&0x7F));
        if ok {
            println!("  {name:<6}: limpo");
        } else if let Some(r) = neg {
            let nrc = r.service.get(2).copied().unwrap_or(0);
            println!("  {name:<6}: recusado (NRC {nrc:#04X})");
        } else {
            println!("  {name:<6}: sem resposta");
        }
    }
    println!("\nReleia os DTCs para confirmar.");
    Ok(())
}

fn read_vin<T: Transport>(dev: &mut Device<T>) -> Result<()> {
    dev.add_flow_control_filter(0x7E8, 0x7E0)
        .map_err(|e| anyhow::anyhow!("filtro VIN: {e:?}"))?;
    let resp = dev
        .request_iso_tp(0x7E0, &[0x22, 0xF1, 0x90], Duration::from_millis(900))
        .map_err(|e| anyhow::anyhow!("pedir VIN: {e:?}"))?;
    for r in &resp {
        // Resposta positiva: 62 F1 90 + 17 bytes ASCII.
        if r.service.len() >= 3
            && r.service[0] == 0x62
            && r.service[1] == 0xF1
            && r.service[2] == 0x90
        {
            let vin: String = r.service[3..]
                .iter()
                .map(|&b| b as char)
                .filter(|c| c.is_ascii_graphic())
                .collect();
            println!("VIN: {vin}");
            return Ok(());
        }
    }
    println!("VIN: sem resposta válida (respostas: {})", resp.len());
    for r in &resp {
        println!("  de {:#05X}: {}", r.can_id, hexdump(&r.service));
    }
    Ok(())
}

fn scan<T: Transport>(dev: &mut Device<T>) -> Result<Vec<(&'static str, u16, u16)>> {
    println!("Varredura de módulos (TesterPresent):");
    // Adiciona um filtro de controle de fluxo por módulo (uma vez).
    for &(_, req, resp) in MODULES {
        let _ = dev.add_flow_control_filter(resp, req);
    }
    let mut present = Vec::new();
    for &(name, req, resp) in MODULES {
        let answers = dev
            .request_iso_tp(req, &[0x3E, 0x00], Duration::from_millis(220))
            .unwrap_or_default();
        let ok = answers.iter().any(|r| {
            r.can_id == resp as u32 && r.service.first().is_some_and(|&b| b == 0x7E || b == 0x7F)
        });
        if ok {
            println!("  [x] {name:<6} req {req:#05X} resp {resp:#05X}");
            present.push((name, req, resp));
        }
    }
    println!("{} módulos responderam.", present.len());
    Ok(present)
}

fn read_all_dtcs<T: Transport>(
    dev: &mut Device<T>,
    present: &[(&'static str, u16, u16)],
) -> Result<()> {
    println!("\nDTCs por módulo:");
    let mut total = 0;
    for &(name, req, resp) in present {
        // 0x19 0x02 0xFF: ReadDTCInformation / reportDTCByStatusMask / todos.
        let answers = dev
            .request_iso_tp(req, &[0x19, 0x02, 0xFF], Duration::from_millis(700))
            .unwrap_or_default();
        let Some(r) = answers
            .iter()
            .find(|r| r.can_id == resp as u32 && r.service.first() == Some(&0x59))
        else {
            continue;
        };
        let dtcs = decode_dtcs(&r.service);
        if dtcs.is_empty() {
            println!("  {name:<6}: sem DTCs");
        } else {
            total += dtcs.len();
            println!("  {name:<6}: {} DTC(s)", dtcs.len());
            for (code, status) in dtcs {
                println!("      {code}  (status {status:#04X})");
            }
        }
    }
    println!("\nTotal: {total} DTC(s).");
    Ok(())
}

/// Decodifica a resposta 0x59 0x02 <mask> [3 bytes DTC + 1 status]*.
fn decode_dtcs(service: &[u8]) -> Vec<(String, u8)> {
    let mut out = Vec::new();
    if service.len() < 3 || service[0] != 0x59 || service[1] != 0x02 {
        return out;
    }
    let mut i = 3; // pula 59 02 <availabilityMask>
    while i + 4 <= service.len() {
        let b0 = service[i];
        let b1 = service[i + 1];
        let b2 = service[i + 2];
        let status = service[i + 3];
        out.push((dtc_code(b0, b1, b2), status));
        i += 4;
    }
    out
}

/// Converte 3 bytes em código DTC padrão (ex.: P0238, C1A77, U0401).
fn dtc_code(b0: u8, b1: u8, b2: u8) -> String {
    let letter = match b0 >> 6 {
        0 => 'P',
        1 => 'C',
        2 => 'B',
        _ => 'U',
    };
    let d1 = (b0 >> 4) & 0x3;
    let d2 = b0 & 0x0F;
    format!("{letter}{d1}{d2:X}{b1:02X}{b2:02X}")
}

fn hexdump(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
