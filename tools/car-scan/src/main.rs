//! Leitura do veículo via Mongoose (somente leitura): VIN, presença de
//! módulos e DTCs. Comunicação ISO-TP a 500 kbps nos pinos 6/14 (HS-CAN); o
//! gateway encaminha os módulos de MS-CAN.
//!
//! Este utilitário NÃO escreve nada no veículo. Limpar DTCs é um comando à
//! parte, explícito.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

mod ccf_checksum;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mongoose::client::Device;
use mongoose::Transport;

/// Descrições de DTC carregadas de um pacote (número de 16 bits -> texto).
type DtcText = HashMap<u16, String>;

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
    /// Pacote SQLite do veículo (para nomear DTCs). Se omitido, procura em
    /// `packs/`. Sem pacote, os DTCs aparecem só como código.
    #[arg(long)]
    pack: Option<String>,
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
    /// Testa o SecurityAccess do BCM: pede semente, calcula a chave e a envia.
    /// NÃO grava nada; só desbloqueia (e trava de volta ao sair da sessão).
    CcfUnlock {
        /// Constante de segurança do BCM (DC0164) em hex, 5 bytes.
        #[arg(long)]
        constant: String,
    },
    /// Sonda o tamanho de recepção do BCM: envia 0x2E DE00 + N bytes via
    /// ISO-TP de software SEM desbloquear (o módulo rejeita, então é seguro).
    /// Mostra se foi aceito (0x7F 2E ..) ou overflow.
    ProbeWrite {
        #[arg(long)]
        size: usize,
    },
    /// Sonda a sequência de flash: desbloqueia e envia RequestDownload (0x34)
    /// para um endereço/tamanho, mostra a resposta e aborta com 0x37.
    /// NÃO transfere dados (não grava). Precisa da constante.
    ProbeDownload {
        #[arg(long)]
        constant: String,
        /// Endereço (hex, ex.: 00FFAC00). Padrão: bloco 1 do CCF.
        #[arg(long, default_value = "00FFAC00")]
        addr: String,
        /// Tamanho em bytes (decimal). Padrão: 196 (bloco 1).
        #[arg(long, default_value_t = 196)]
        size: u32,
    },
    /// Sonda o caminho de PROGRAMAÇÃO: desbloqueia na sessão estendida
    /// (constante diagnóstica DC0164), troca para a sessão de programação
    /// (0x10 02), desbloqueia a segurança de programação (DC0163) e tenta
    /// RequestDownload. NÃO transfere dados.
    ProbeProg {
        /// Constante diagnóstica do BCM (DC0164) em hex, 5 bytes.
        #[arg(long)]
        constant: String,
        /// Constante de programação do BCM (DC0163) em hex, 5 bytes.
        #[arg(long)]
        prog_constant: String,
    },
    /// Teste seguro da tubulação: lê o CCF, desbloqueia e REGRAVA os MESMOS
    /// bytes (não muda nada), depois confere a releitura. ESCREVE (idêntico):
    /// exige --yes. Precisa da constante do BCM.
    CcfWriteback {
        #[arg(long)]
        constant: String,
        #[arg(long)]
        yes: bool,
    },
    /// Altera UM byte do CCF (offset absoluto na imagem do DID 0xDE00),
    /// recalcula o checksum, grava e confere. ESCREVE: exige --yes.
    CcfSet {
        #[arg(long)]
        constant: String,
        /// Offset absoluto (decimal) na imagem do CCF.
        #[arg(long)]
        offset: usize,
        /// Novo valor do byte (hex, ex.: 02).
        #[arg(long)]
        value: String,
        #[arg(long)]
        yes: bool,
    },
    /// Grava o CCF pela via de FLASH (VBF CARCFG): erase 0xFFAC00 ->
    /// RequestDownload -> TransferData (em pedaços) -> TransferExit -> verifica.
    /// Supera o limite de 128 bytes do 0x2E. Por padrão faz só um PROBE
    /// não-destrutivo (não apaga/grava). Com --write + --yes, GRAVA de verdade.
    CcfFlash {
        /// Constante diagnóstica do BCM (DC0164), hex 5 bytes.
        #[arg(long)]
        constant: String,
        /// Constante de programação do BCM (DC0163), hex 5 bytes.
        #[arg(long)]
        prog_constant: String,
        /// Imagem alvo (.bin, 1024 bytes). Sem isto, usa o CCF atual
        /// (regravação IDÊNTICA — não muda nenhuma opção).
        #[arg(long)]
        image: Option<String>,
        /// RoutineId do erase (hex). Padrão FF00 (eraseMemory UDS).
        #[arg(long, default_value = "FF00")]
        erase_routine: String,
        /// RoutineId a rodar ANTES do erase/download (ex.: 716E "Start Control
        /// Unit Program"). Opcional; sem isto, não roda nada antes.
        #[arg(long)]
        start_routine: Option<String>,
        /// GRAVA de verdade (erase+download+transfer). Sem isto é só probe.
        #[arg(long)]
        write: bool,
        /// Confirmação obrigatória quando --write.
        #[arg(long)]
        yes: bool,
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
            let texts = load_pack_texts(cli.pack.as_deref());
            let present = scan(&mut dev)?;
            read_all_dtcs(&mut dev, &present, &texts)?;
        }
        Command::All => {
            let texts = load_pack_texts(cli.pack.as_deref());
            read_vin(&mut dev)?;
            println!();
            let present = scan(&mut dev)?;
            println!();
            read_all_dtcs(&mut dev, &present, &texts)?;
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
        Command::CcfUnlock { constant } => {
            unlock_bcm(&mut dev, parse_constant(&constant)?)?;
        }
        Command::ProbeWrite { size } => {
            sw_setup(&mut dev)?;
            let mut service = vec![0x2E, 0xDE, 0x00];
            service.extend(std::iter::repeat_n(0u8, size));
            println!("Enviando 0x2E DE00 + {size} bytes (sem desbloquear)...");
            match sw_uds(&mut dev, &service, 4000) {
                Ok(r) => println!("Resposta: {}", hexdump(&r)),
                Err(e) => println!("Falha: {e}"),
            }
        }
        Command::ProbeDownload {
            constant,
            addr,
            size,
        } => {
            let c = parse_constant(&constant)?;
            let addr = u32::from_str_radix(addr.trim_start_matches("0x"), 16)
                .context("endereço hex inválido")?;
            sw_setup(&mut dev)?;
            if !unlock_bcm_sw(&mut dev, c)? {
                anyhow::bail!("não desbloqueou; abortado.");
            }
            probe_download(&mut dev, addr, size)?;
        }
        Command::ProbeProg {
            constant,
            prog_constant,
        } => {
            let c = parse_constant(&constant)?;
            let pc = parse_constant(&prog_constant)?;
            sw_setup(&mut dev)?;
            probe_prog(&mut dev, c, pc)?;
        }
        Command::CcfWriteback { constant, yes } => {
            ccf_writeback(&mut dev, parse_constant(&constant)?, yes)?;
        }
        Command::CcfSet {
            constant,
            offset,
            value,
            yes,
        } => {
            let v = parse_hex(&value)?;
            anyhow::ensure!(v.len() == 1, "o valor deve ser 1 byte");
            ccf_set(&mut dev, parse_constant(&constant)?, offset, v[0], yes)?;
        }
        Command::CcfFlash {
            constant,
            prog_constant,
            image,
            erase_routine,
            start_routine,
            write,
            yes,
        } => {
            let parse_u16 = |s: &str| -> Result<u16> {
                u16::from_str_radix(s.trim_start_matches("0x"), 16)
                    .map_err(|e| anyhow::anyhow!("routineId inválido '{s}': {e}"))
            };
            let img = match &image {
                Some(p) => {
                    let b = std::fs::read(p).with_context(|| format!("ler {p}"))?;
                    Some(b)
                }
                None => None,
            };
            let start = match &start_routine {
                Some(s) => Some(parse_u16(s)?),
                None => None,
            };
            sw_setup(&mut dev)?;
            ccf_flash(
                &mut dev,
                parse_constant(&constant)?,
                parse_constant(&prog_constant)?,
                img,
                parse_u16(&erase_routine)?,
                start,
                write,
                yes,
            )?;
        }
    }
    Ok(())
}

fn parse_constant(s: &str) -> Result<[u8; 5]> {
    let c = parse_hex(s)?;
    anyhow::ensure!(c.len() == 5, "a constante deve ter 5 bytes");
    let mut c5 = [0u8; 5];
    c5.copy_from_slice(&c);
    Ok(c5)
}

/// Prepara a via de ESCRITA: fecha o canal ISO-TP de firmware e abre um canal
/// CAN cru com filtro PASS, para usar o ISO-TP de software (sem o limite de
/// 128 bytes do firmware).
fn sw_setup<T: Transport<Error: 'static>>(dev: &mut Device<T>) -> Result<()> {
    dev.close_channel()
        .map_err(|e| anyhow::anyhow!("fechar canal: {e:?}"))?;
    dev.connect_can(500_000)
        .map_err(|e| anyhow::anyhow!("abrir canal CAN: {e:?}"))?;
    dev.add_can_pass_filter(BCM_RESP)
        .map_err(|e| anyhow::anyhow!("filtro PASS BCM: {e:?}"))?;
    Ok(())
}

/// Faz uma requisição UDS ao BCM via ISO-TP de software, tratando resposta
/// pendente (0x7F <sid> 0x78) até a resposta final.
fn sw_uds<T: Transport<Error: 'static>>(
    dev: &mut Device<T>,
    service: &[u8],
    timeout_ms: u64,
) -> Result<Vec<u8>> {
    use mongoose::isotp_sw;
    isotp_sw::send(dev, BCM_REQ, BCM_RESP, service)
        .map_err(|e| anyhow::anyhow!("envio sw: {e}"))?;
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let resp = isotp_sw::recv(dev, BCM_REQ, BCM_RESP, Duration::from_millis(2500))
            .map_err(|e| anyhow::anyhow!("recepção sw: {e}"))?;
        let pending = resp.first() == Some(&0x7F) && resp.get(2) == Some(&0x78);
        if !pending {
            return Ok(resp);
        }
        if std::time::Instant::now() >= deadline {
            return Ok(resp);
        }
    }
}

/// Desbloqueio do BCM pela via de software.
fn unlock_bcm_sw<T: Transport<Error: 'static>>(
    dev: &mut Device<T>,
    constant: [u8; 5],
) -> Result<bool> {
    let s = sw_uds(dev, &[0x10, 0x03], 800)?;
    anyhow::ensure!(s.first() == Some(&0x50), "sem sessão estendida (sw)");
    let seed = sw_uds(dev, &[0x27, 0x03], 800)?;
    anyhow::ensure!(
        seed.first() == Some(&0x67) && seed.len() >= 5,
        "sem semente (sw): {}",
        hexdump(&seed)
    );
    let s3 = [seed[2], seed[3], seed[4]];
    println!("Semente (sw): {}", hexdump(&s3));
    if s3 == [0, 0, 0] {
        return Ok(true);
    }
    let key = jlr_security::iso14229_key(s3, constant);
    println!("Chave (sw): {}", hexdump(&key));
    let mut send = vec![0x27, 0x04];
    send.extend_from_slice(&key);
    let r = sw_uds(dev, &send, 1000)?;
    if r.first() == Some(&0x67) && r.get(1) == Some(&0x04) {
        println!("BCM desbloqueado (sw).");
        Ok(true)
    } else {
        println!("Chave recusada (sw): {}", hexdump(&r));
        Ok(false)
    }
}

/// Sonda a sequência de flash: envia RequestDownload (0x34) com vários
/// formatos de identificador de endereço/tamanho e mostra a resposta do BCM.
/// Aborta com RequestTransferExit (0x37). Não transfere dados (não grava).
fn probe_download<T: Transport<Error: 'static>>(
    dev: &mut Device<T>,
    addr: u32,
    size: u32,
) -> Result<()> {
    // dataFormatIdentifier 0x00 (sem compressão/cripto); tenta formatos de
    // endereço comuns: 0x44 (4+4), 0x33 (3+3), 0x14/0x24.
    for fmt in [0x44u8, 0x33, 0x24, 0x14] {
        let asize = (fmt & 0x0f) as usize; // nº de bytes do endereço
        let ssize = ((fmt >> 4) & 0x0f) as usize; // nº de bytes do tamanho
        let mut req = vec![0x34, 0x00, fmt];
        req.extend_from_slice(&addr.to_be_bytes()[4 - asize..]);
        req.extend_from_slice(&size.to_be_bytes()[4 - ssize..]);
        println!("RequestDownload fmt {fmt:#04X}: enviando {}", hexdump(&req));
        let r = sw_uds(dev, &req, 2500)?;
        println!("  resp: {}", hexdump(&r));
        if r.first() == Some(&0x74) {
            println!(
                "  >>> ACEITO. maxNumberOfBlockLength e formato: {}",
                hexdump(&r)
            );
            let exit = sw_uds(dev, &[0x37], 2500).unwrap_or_default();
            println!("  TransferExit (abortar): {}", hexdump(&exit));
            return Ok(());
        }
        // 0x7F 34 NRC: mostra o motivo e tenta o próximo formato.
    }
    println!("Nenhum formato de RequestDownload foi aceito nesta sondagem.");
    Ok(())
}

/// Sonda o caminho de programação do BCM: sessão 0x10 02, segurança de
/// programação (seed 0x27 01, key 0x27 02, constante DC0163) e tenta
/// RequestDownload e um 0x2E grande. Não transfere dados.
fn probe_prog<T: Transport<Error: 'static>>(
    dev: &mut Device<T>,
    diag_const: [u8; 5],
    prog_const: [u8; 5],
) -> Result<()> {
    // 1) Desbloqueia na sessão estendida (diagnóstica), que já é validada.
    if !unlock_bcm_sw(dev, diag_const)? {
        println!("Não desbloqueou na sessão estendida; abortando.");
        return Ok(());
    }
    // 2) Troca para a sessão de programação.
    let s = sw_uds(dev, &[0x10, 0x02], 1500)?;
    println!("Sessão de programação (0x10 02): {}", hexdump(&s));
    std::thread::sleep(Duration::from_millis(150));
    let tp = sw_uds(dev, &[0x3E, 0x00], 1000).unwrap_or_default();
    println!("TesterPresent após programação: {}", hexdump(&tp));
    // 3) Desbloqueio de PROGRAMAÇÃO: 0x27 01 (seed) / 0x27 02 (key), DC0163.
    let seed = sw_uds(dev, &[0x27, 0x01], 1000).unwrap_or_default();
    println!("Seed programação (0x27 01): {}", hexdump(&seed));
    if seed.first() == Some(&0x67) && seed.len() >= 5 {
        let s3 = [seed[2], seed[3], seed[4]];
        if s3 != [0, 0, 0] {
            let key = jlr_security::iso14229_key(s3, prog_const);
            let mut k = vec![0x27, 0x02];
            k.extend_from_slice(&key);
            let kr = sw_uds(dev, &k, 1200).unwrap_or_default();
            println!(
                "Key programação (0x27 02): {} (chave {})",
                hexdump(&kr),
                hexdump(&key)
            );
            if kr.first() == Some(&0x67) {
                println!(">>> SEGURANÇA DE PROGRAMAÇÃO DESBLOQUEADA!");
            }
        } else {
            println!("Seed zero: já desbloqueado.");
        }
    }
    // 4) Varre endereço/tamanho para o RequestDownload (formato 0x44).
    let combos: &[(u32, u32, &str)] = &[
        (0x00FF_AC00, 196, "bloco 1 (196)"),
        (0x00FF_AC00, 0x2BA, "config inteiro (698)"),
        (0x00FF_AC00, 0x400, "região (1024)"),
        (0x00FF_ACC4, 252, "bloco EUCD (252)"),
        (0x00FF_AC00, 698, "config (698 dec)"),
    ];
    for &(addr, size, label) in combos {
        let mut rd = vec![0x34, 0x00, 0x44];
        rd.extend_from_slice(&addr.to_be_bytes());
        rd.extend_from_slice(&size.to_be_bytes());
        let r = sw_uds(dev, &rd, 2500)?;
        println!(
            "RequestDownload {label} {addr:#010X}/{size}: {}",
            hexdump(&r)
        );
        if r.first() == Some(&0x74) {
            println!(">>> ACEITO! A resposta traz o maxNumberOfBlockLength.");
            let exit = sw_uds(dev, &[0x37], 2000).unwrap_or_default();
            println!("TransferExit (abortar): {}", hexdump(&exit));
            return Ok(());
        }
    }
    println!("Nenhuma combinação de endereço/tamanho foi aceita nesta sondagem.");
    Ok(())
}

/// Entra na sessão de programação (0x10 02) e desbloqueia a segurança de
/// programação (0x27 01/02, constante DC0163). Retorna true se destravou.
fn prog_unlock_bcm_sw<T: Transport<Error: 'static>>(
    dev: &mut Device<T>,
    prog_const: [u8; 5],
) -> Result<bool> {
    let s = sw_uds(dev, &[0x10, 0x02], 1500)?;
    if s.first() != Some(&0x50) {
        println!("Sessão de programação negada: {}", hexdump(&s));
        return Ok(false);
    }
    std::thread::sleep(Duration::from_millis(150));
    let _ = sw_uds(dev, &[0x3E, 0x00], 1000);
    let seed = sw_uds(dev, &[0x27, 0x01], 1000)?;
    if seed.first() != Some(&0x67) || seed.len() < 5 {
        println!("Seed de programação inesperada: {}", hexdump(&seed));
        return Ok(false);
    }
    let s3 = [seed[2], seed[3], seed[4]];
    if s3 == [0, 0, 0] {
        return Ok(true); // já desbloqueado
    }
    let key = jlr_security::iso14229_key(s3, prog_const);
    let mut k = vec![0x27, 0x02];
    k.extend_from_slice(&key);
    let kr = sw_uds(dev, &k, 1200)?;
    Ok(kr.first() == Some(&0x67))
}

/// Monta um UDS com endereço+tamanho no formato 0x44 (4 bytes cada).
fn alfid44(service: u8, sub: &[u8], addr: u32, len: u32) -> Vec<u8> {
    let mut v = vec![service];
    v.extend_from_slice(sub);
    v.extend_from_slice(&addr.to_be_bytes());
    v.extend_from_slice(&len.to_be_bytes());
    v
}

/// Timestamp simples (segundos desde a época) para nomear backups.
fn now_stamp() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{t}")
}

/// Grava o CCF pela via de FLASH (VBF CARCFG). Região 0x00FFAC00, 698 bytes de
/// config (erase de 1024). Com `write=false` faz só um probe não-destrutivo.
#[allow(clippy::too_many_arguments)]
fn ccf_flash<T: Transport<Error: 'static>>(
    dev: &mut Device<T>,
    diag_const: [u8; 5],
    prog_const: [u8; 5],
    image: Option<Vec<u8>>,
    erase_routine: u16,
    start_routine: Option<u16>,
    write: bool,
    yes: bool,
) -> Result<()> {
    const ADDR: u32 = 0x00FF_AC00;
    const ERASE_LEN: u32 = 0x0400; // 1024
    const CFG_LEN: usize = 698;

    // 1) Lê o CCF atual (baseline + backup).
    let current = read_ccf_sw(dev)?;
    anyhow::ensure!(current.len() >= CFG_LEN, "CCF curto: {} bytes", current.len());
    let backup = format!("captures/ccf_preflash_{}.bin", now_stamp());
    std::fs::create_dir_all("captures").ok();
    std::fs::write(&backup, &current)?;
    println!("CCF atual lido ({} bytes). Backup: {backup}", current.len());

    // 2) Imagem alvo (default = idêntica).
    let target = match image {
        Some(img) => {
            anyhow::ensure!(
                img.len() == current.len(),
                "imagem alvo tem {} bytes; esperado {}",
                img.len(),
                current.len()
            );
            img
        }
        None => current.clone(),
    };
    let changed: Vec<usize> = (0..CFG_LEN).filter(|&i| current[i] != target[i]).collect();
    if changed.is_empty() {
        println!("Alvo = regravação IDÊNTICA (nenhum byte de config muda).");
    } else {
        println!("Alvo muda {} byte(s) da config:", changed.len());
        for &i in changed.iter().take(16) {
            println!("  byte {i}: 0x{:02X} -> 0x{:02X}", current[i], target[i]);
        }
    }

    // 3) Desbloqueio (diagnóstico + programação).
    anyhow::ensure!(
        unlock_bcm_sw(dev, diag_const)?,
        "não desbloqueou a sessão diagnóstica"
    );
    anyhow::ensure!(
        prog_unlock_bcm_sw(dev, prog_const)?,
        "não desbloqueou a segurança de programação"
    );
    println!("Programação desbloqueada.");

    // 4) Rotina opcional antes do flash (ex.: 0x716E Start Control Unit Program).
    if let Some(rid) = start_routine {
        let req = vec![0x31, 0x01, (rid >> 8) as u8, (rid & 0xff) as u8];
        let r = sw_uds(dev, &req, 5000)?;
        println!("RoutineControl start {rid:#06X}: {}", hexdump(&r));
    }

    if !write {
        // PROBE não-destrutivo: tenta RequestDownload (sem erase/transfer).
        let rd = alfid44(0x34, &[0x00, 0x44], ADDR, CFG_LEN as u32);
        println!("PROBE RequestDownload (sem erase): {}", hexdump(&rd));
        let r = sw_uds(dev, &rd, 3000)?;
        println!("  resposta: {}", hexdump(&r));
        if r.first() == Some(&0x74) {
            println!(">>> ACEITO sem erase! Abortando com 0x37 (nada gravado).");
            let _ = sw_uds(dev, &[0x37], 2000);
        } else {
            println!("Rejeitado sem erase. Nada foi apagado/gravado (probe seguro).");
        }
        let _ = sw_uds(dev, &[0x10, 0x01], 1000); // volta à sessão padrão
        return Ok(());
    }

    // === GRAVAÇÃO (destrutivo a partir do erase) ===
    anyhow::ensure!(yes, "--write exige --yes (vai APAGAR e regravar o CCF do BCM)");

    // 5) ERASE.
    let erase = alfid44(
        0x31,
        &[0x01, (erase_routine >> 8) as u8, (erase_routine & 0xff) as u8],
        ADDR,
        ERASE_LEN,
    );
    println!("ERASE: {}", hexdump(&erase));
    let r = sw_uds(dev, &erase, 15000)?;
    println!("  resposta erase: {}", hexdump(&r));
    anyhow::ensure!(
        r.first() == Some(&0x71),
        "erase NEGADO ({}); nada foi apagado — abortando",
        hexdump(&r)
    );
    println!(">>> Região apagada. É preciso completar a gravação agora.");

    // 6) RequestDownload.
    let rd = alfid44(0x34, &[0x00, 0x44], ADDR, CFG_LEN as u32);
    let r = sw_uds(dev, &rd, 5000)?;
    anyhow::ensure!(
        r.first() == Some(&0x74) && r.len() >= 2,
        "RequestDownload NEGADO: {}",
        hexdump(&r)
    );
    let nbytes = ((r[1] >> 4) as usize).min(r.len() - 2);
    let mut maxblk = 0usize;
    for b in &r[2..2 + nbytes] {
        maxblk = (maxblk << 8) | *b as usize;
    }
    let chunk = maxblk.saturating_sub(2).clamp(1, 126);
    println!("RequestDownload OK. maxBlockLength={maxblk}, pedaço={chunk} bytes.");

    // 7) TransferData em pedaços.
    let data = &target[..CFG_LEN];
    let mut seq: u8 = 1;
    let mut off = 0;
    while off < data.len() {
        let end = (off + chunk).min(data.len());
        let mut td = vec![0x36, seq];
        td.extend_from_slice(&data[off..end]);
        let r = sw_uds(dev, &td, 5000)?;
        anyhow::ensure!(
            r.first() == Some(&0x76) && r.get(1) == Some(&seq),
            "TransferData seq {seq} NEGADO em {off}: {}",
            hexdump(&r)
        );
        off = end;
        seq = seq.wrapping_add(1);
    }
    println!("TransferData completo ({} bytes).", data.len());

    // 8) TransferExit.
    let r = sw_uds(dev, &[0x37], 5000)?;
    println!("TransferExit: {}", hexdump(&r));

    // 9) Reset + verificação por releitura.
    let _ = sw_uds(dev, &[0x11, 0x01], 2000); // ECUReset
    std::thread::sleep(Duration::from_millis(1500));
    let after = read_ccf_sw(dev).unwrap_or_default();
    if after.len() >= CFG_LEN && after[..CFG_LEN] == target[..CFG_LEN] {
        println!(">>> SUCESSO: releitura confere com o alvo ({CFG_LEN} bytes).");
    } else {
        println!("ATENÇÃO: releitura NÃO confere. Regrave o backup {backup} com --image <arquivo> --write --yes.");
    }
    Ok(())
}

/// Lê o CCF pela via de software (0x22 DE00).
fn read_ccf_sw<T: Transport<Error: 'static>>(dev: &mut Device<T>) -> Result<Vec<u8>> {
    let r = sw_uds(dev, &[0x22, 0xDE, 0x00], 4000)?;
    anyhow::ensure!(
        r.len() > 3 && r[0] == 0x62 && r[1] == 0xDE && r[2] == 0x00,
        "resposta inesperada à leitura do CCF (sw): {}",
        hexdump(&r[..r.len().min(8)])
    );
    Ok(r[3..].to_vec())
}

/// Grava o CCF pela via de software (0x2E DE00 + imagem).
fn write_ccf_sw<T: Transport<Error: 'static>>(dev: &mut Device<T>, image: &[u8]) -> Result<bool> {
    let mut service = vec![0x2E, 0xDE, 0x00];
    service.extend_from_slice(image);
    let r = sw_uds(dev, &service, 10_000)?;
    if r.first() == Some(&0x6E) && r.get(1) == Some(&0xDE) {
        Ok(true)
    } else {
        println!("Gravação (sw) sem positivo: {}", hexdump(&r));
        Ok(false)
    }
}

/// Lê a imagem do CCF (1024 bytes) do BCM via DID 0xDE00.
fn read_ccf_image<T: Transport<Error: 'static>>(dev: &mut Device<T>) -> Result<Vec<u8>> {
    ensure_bcm_filter(dev);
    let answers = dev
        .request_iso_tp(BCM_REQ, &[0x22, 0xDE, 0x00], Duration::from_millis(2500))
        .map_err(|e| anyhow::anyhow!("ler CCF: {e:?}"))?;
    let data = find_positive(&answers, BCM_RESP).context("CCF não lido (sem resposta positiva)")?;
    Ok(data[3..].to_vec())
}

/// Grava a imagem do CCF no BCM via WriteDataByIdentifier (0x2E DE00).
/// Requer o módulo já desbloqueado e em sessão estendida. Retorna Ok(true)
/// se o módulo respondeu positivamente (0x6E DE00).
/// Teste seguro: lê o CCF, faz backup, desbloqueia, regrava os MESMOS bytes
/// e confere a releitura. Não muda nenhuma configuração.
fn ccf_writeback<T: Transport<Error: 'static>>(
    dev: &mut Device<T>,
    constant: [u8; 5],
    yes: bool,
) -> Result<()> {
    if !yes {
        anyhow::bail!("regravar o CCF escreve no módulo (mesmo idêntico). Repita com --yes.");
    }
    let original = read_ccf_image(dev)?;
    println!("CCF lido: {} bytes.", original.len());
    let backup = format!("backups/ccf_writeback_{}.bin", timestamp());
    std::fs::create_dir_all("backups").ok();
    std::fs::write(&backup, &original).ok();
    println!("Backup salvo em {backup}");

    // Via de ESCRITA: CAN cru + ISO-TP de software (sem o limite de 128 bytes).
    sw_setup(dev)?;
    if !unlock_bcm_sw(dev, constant)? {
        anyhow::bail!("não foi possível desbloquear o BCM; nada gravado.");
    }

    println!(
        "Regravando os MESMOS {} bytes (ISO-TP sw)...",
        original.len()
    );
    if !write_ccf_sw(dev, &original)? {
        anyhow::bail!("gravação não confirmada.");
    }
    println!("Gravação confirmada. Conferindo releitura...");

    let reread = read_ccf_sw(dev)?;
    if reread == original {
        println!("\n*** OK: releitura idêntica. Tubulação de escrita validada. ***");
    } else {
        println!("\nATENÇÃO: releitura difere do gravado. Backup em {backup}.");
        let diffs = original.iter().zip(&reread).filter(|(a, b)| a != b).count();
        println!("{diffs} bytes diferentes.");
    }
    Ok(())
}

/// Altera um byte do CCF, recalcula o checksum, grava e confere.
fn ccf_set<T: Transport<Error: 'static>>(
    dev: &mut Device<T>,
    constant: [u8; 5],
    offset: usize,
    value: u8,
    yes: bool,
) -> Result<()> {
    if !yes {
        anyhow::bail!("alterar o CCF escreve no módulo. Repita com --yes.");
    }
    let original = read_ccf_image(dev)?;
    anyhow::ensure!(offset < original.len(), "offset fora da imagem");
    println!(
        "CCF lido: {} bytes. Byte {offset}: 0x{:02X} -> 0x{value:02X}",
        original.len(),
        original[offset]
    );
    let backup = format!("backups/ccf_before_set_{}.bin", timestamp());
    std::fs::create_dir_all("backups").ok();
    std::fs::write(&backup, &original).ok();
    println!("Backup salvo em {backup}");

    let mut modified = original.clone();
    modified[offset] = value;
    // Recalcula o checksum (preenchido após reconstruir o algoritmo).
    ccf_checksum::fix_checksums(&mut modified).context("recalcular checksum do CCF")?;

    sw_setup(dev)?;
    if !unlock_bcm_sw(dev, constant)? {
        anyhow::bail!("não foi possível desbloquear o BCM; nada gravado.");
    }
    if !write_ccf_sw(dev, &modified)? {
        anyhow::bail!("gravação não confirmada.");
    }
    println!("Gravação confirmada. Conferindo...");
    let reread = read_ccf_sw(dev)?;
    if reread.get(offset) == Some(&value) {
        println!("\n*** OK: byte {offset} agora é 0x{value:02X} no módulo. ***");
        println!("Backup do estado anterior: {backup}");
    } else {
        println!("\nATENÇÃO: o byte não mudou como esperado. Backup em {backup}.");
    }
    Ok(())
}

fn timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

const BCM_REQ: u16 = 0x726;
const BCM_RESP: u16 = 0x72E;

/// Faz o SecurityAccess diagnóstico do BCM (nível 0x03): sessão estendida,
/// pedir semente (0x27 0x03), calcular a chave e enviá-la (0x27 0x04).
/// Retorna Ok(true) se o módulo aceitou a chave. NÃO grava nada.
/// Garante o filtro de controle de fluxo do BCM, ignorando "já existe"
/// (ERR_NOT_UNIQUE), para poder ser chamado em sequência por vários passos.
fn ensure_bcm_filter<T: Transport<Error: 'static>>(dev: &mut Device<T>) {
    let _ = dev.add_flow_control_filter(BCM_RESP, BCM_REQ);
}

fn unlock_bcm<T: Transport<Error: 'static>>(
    dev: &mut Device<T>,
    constant: [u8; 5],
) -> Result<bool> {
    ensure_bcm_filter(dev);

    // Sessão estendida.
    let s = dev
        .request_iso_tp(BCM_REQ, &[0x10, 0x03], Duration::from_millis(500))
        .unwrap_or_default();
    anyhow::ensure!(
        s.iter().any(|r| r.service.first() == Some(&0x50)),
        "o BCM não entrou em sessão estendida"
    );
    println!("Sessão estendida OK.");

    // Pedir semente (0x27 0x03).
    let seed_resp = dev
        .request_iso_tp(BCM_REQ, &[0x27, 0x03], Duration::from_millis(500))
        .map_err(|e| anyhow::anyhow!("pedir semente: {e:?}"))?;
    let seed_msg = seed_resp
        .iter()
        .find(|r| r.service.first() == Some(&0x67) && r.service.get(1) == Some(&0x03))
        .with_context(|| {
            format!(
                "sem resposta de semente (respostas: {})",
                seed_resp
                    .iter()
                    .map(|r| hexdump(&r.service))
                    .collect::<Vec<_>>()
                    .join(" | ")
            )
        })?;
    anyhow::ensure!(seed_msg.service.len() >= 5, "semente curta");
    let seed = [
        seed_msg.service[2],
        seed_msg.service[3],
        seed_msg.service[4],
    ];
    println!("Semente recebida: {}", hexdump(&seed));

    if seed == [0, 0, 0] {
        println!("Semente zero: o módulo já está desbloqueado.");
        return Ok(true);
    }

    // Calcular a chave com o gerador validado.
    let key = jlr_security::iso14229_key(seed, constant);
    println!("Chave calculada: {}", hexdump(&key));

    // Enviar a chave (0x27 0x04).
    let mut send = vec![0x27, 0x04];
    send.extend_from_slice(&key);
    let key_resp = dev
        .request_iso_tp(BCM_REQ, &send, Duration::from_millis(700))
        .map_err(|e| anyhow::anyhow!("enviar chave: {e:?}"))?;

    if key_resp
        .iter()
        .any(|r| r.service.first() == Some(&0x67) && r.service.get(1) == Some(&0x04))
    {
        println!("\n*** BCM DESBLOQUEADO: a chave foi ACEITA pelo módulo real. ***");
        println!("(Nada foi gravado; o módulo trava de novo ao sair da sessão.)");
        Ok(true)
    } else if let Some(neg) = key_resp.iter().find(|r| r.service.first() == Some(&0x7F)) {
        let nrc = neg.service.get(2).copied().unwrap_or(0);
        let motivo = match nrc {
            0x35 => "chave inválida",
            0x36 => "número de tentativas excedido",
            0x37 => "tempo de atraso ativo (aguarde)",
            _ => "ver código",
        };
        println!("\nChave recusada (NRC {nrc:#04X}: {motivo}).");
        Ok(false)
    } else {
        println!("\nSem resposta clara ao enviar a chave.");
        Ok(false)
    }
}

fn raw<T: Transport<Error: 'static>>(
    dev: &mut Device<T>,
    module: &str,
    hex: &str,
    extended: bool,
) -> Result<()> {
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
    anyhow::ensure!(
        clean.len().is_multiple_of(2),
        "hex com número ímpar de dígitos"
    );
    (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i + 2], 16).map_err(Into::into))
        .collect()
}

/// Lê o CCF do BCM (DID 0xDE00, serviço 0x22) e salva um backup bruto.
/// Tenta na sessão padrão; se recusado, entra em sessão estendida (0x10 03)
/// e repete. Operação de leitura (a troca de sessão é reversível).
fn read_ccf<T: Transport<Error: 'static>>(dev: &mut Device<T>, out: &str) -> Result<()> {
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
fn read_live<T: Transport<Error: 'static>>(dev: &mut Device<T>) -> Result<()> {
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
fn clear_dtcs<T: Transport<Error: 'static>>(
    dev: &mut Device<T>,
    yes: bool,
    module: Option<&str>,
) -> Result<()> {
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

fn read_vin<T: Transport<Error: 'static>>(dev: &mut Device<T>) -> Result<()> {
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

fn scan<T: Transport<Error: 'static>>(
    dev: &mut Device<T>,
) -> Result<Vec<(&'static str, u16, u16)>> {
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

fn read_all_dtcs<T: Transport<Error: 'static>>(
    dev: &mut Device<T>,
    present: &[(&'static str, u16, u16)],
    texts: &DtcText,
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
            for ([b0, b1, b2], status) in dtcs {
                let code = dtc_code(b0, b1, b2);
                let desc = texts
                    .get(&dtc_number(b0, b1))
                    .map(|s| format!("  {s}"))
                    .unwrap_or_default();
                println!("      {code:<10} (status {status:#04X}){desc}");
            }
        }
    }
    println!("\nTotal: {total} DTC(s).");
    if texts.is_empty() {
        println!("(sem pacote de textos: use --pack para ver as descrições)");
    }
    Ok(())
}

/// Decodifica a resposta 0x59 0x02 <mask> [3 bytes DTC + 1 status]*.
/// Devolve os 3 bytes crus do DTC + o byte de status.
fn decode_dtcs(service: &[u8]) -> Vec<([u8; 3], u8)> {
    let mut out = Vec::new();
    if service.len() < 3 || service[0] != 0x59 || service[1] != 0x02 {
        return out;
    }
    let mut i = 3; // pula 59 02 <availabilityMask>
    while i + 4 <= service.len() {
        out.push((
            [service[i], service[i + 1], service[i + 2]],
            service[i + 3],
        ));
        i += 4;
    }
    out
}

/// Converte 3 bytes em código DTC padrão (ex.: P0238, C1A77, U0401).
///
/// Os dois primeiros bytes são o código base; o terceiro é o tipo de falha
/// (sufixo após o hífen, ex.: `C1A13-08`). `00` significa sem sufixo.
fn dtc_code(b0: u8, b1: u8, b2: u8) -> String {
    let base = dtc_base_code(b0, b1);
    if b2 == 0 {
        base
    } else {
        format!("{base}-{b2:02X}")
    }
}

/// Só o código base (2 bytes), ex.: `C1A13`.
fn dtc_base_code(b0: u8, b1: u8) -> String {
    let letter = match b0 >> 6 {
        0 => 'P',
        1 => 'C',
        2 => 'B',
        _ => 'U',
    };
    let d1 = (b0 >> 4) & 0x3;
    let d2 = b0 & 0x0F;
    format!("{letter}{d1}{d2:X}{b1:02X}")
}

/// Número de 16 bits usado como chave no `gdx` (byte0<<8 | byte1).
fn dtc_number(b0: u8, b1: u8) -> u16 {
    (b0 as u16) << 8 | b1 as u16
}

/// Resolve o caminho do pacote: o informado em `--pack`, ou o primeiro
/// `*.sqlite` encontrado em `packs/` (ao lado do cwd ou do executável).
fn resolve_pack(explicit: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        let pb = PathBuf::from(p);
        return pb.exists().then_some(pb);
    }
    let mut roots: Vec<PathBuf> = vec![PathBuf::from("packs")];
    if let Ok(exe) = std::env::current_exe() {
        // target/debug/car-scan.exe -> subir até a raiz do repo.
        for anc in exe.ancestors().skip(1).take(4) {
            roots.push(anc.join("packs"));
        }
    }
    for dir in roots {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                let path = e.path();
                if path.extension().and_then(|s| s.to_str()) == Some("sqlite") {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Resolve o pacote e carrega os textos de DTC, avisando qual pacote usou.
fn load_pack_texts(explicit: Option<&str>) -> DtcText {
    match resolve_pack(explicit) {
        Some(pack) => {
            let texts = load_dtc_text(&pack);
            println!(
                "Textos de DTC: {} descrições de {}",
                texts.len(),
                pack.display()
            );
            texts
        }
        None => {
            println!("Pacote não encontrado; DTCs serão exibidos só como código.");
            HashMap::new()
        }
    }
}

/// Carrega a tabela `dtc_text` de um pacote. Qualquer erro vira um mapa vazio
/// (os DTCs então aparecem só com o código).
fn load_dtc_text(pack: &Path) -> DtcText {
    let mut map = HashMap::new();
    let Ok(conn) = rusqlite::Connection::open_with_flags(
        pack,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) else {
        return map;
    };
    let Ok(mut stmt) = conn.prepare("SELECT number, description FROM dtc_text") else {
        return map;
    };
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, i64>(0)? as u16, r.get::<_, Option<String>>(1)?))
    });
    if let Ok(rows) = rows {
        for row in rows.flatten() {
            if let (num, Some(desc)) = row {
                if !desc.is_empty() {
                    map.insert(num, desc);
                }
            }
        }
    }
    map
}

fn hexdump(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodifica_codigo_dtc() {
        // C1A13 (0x5A13) com tipo de falha 0x08 -> "C1A13-08".
        assert_eq!(dtc_code(0x5A, 0x13, 0x08), "C1A13-08");
        // Sem tipo de falha (0x00) -> só o código base.
        assert_eq!(dtc_code(0x5A, 0x13, 0x00), "C1A13");
        assert_eq!(dtc_code(0x06, 0x71, 0x00), "P0671");
        assert_eq!(dtc_code(0xC0, 0x40, 0x11), "U0040-11");
    }

    #[test]
    fn numero_de_16_bits_casa_com_gdx() {
        // A chave de busca no gdx é byte0<<8 | byte1.
        assert_eq!(dtc_number(0x5A, 0x13), 0x5A13);
        assert_eq!(dtc_number(0x06, 0x71), 0x0671);
    }

    #[test]
    fn decode_dtcs_separa_registros() {
        // 59 02 FF  [5A 13 08 | status 2A]  [06 71 2F | status 24]
        let svc = [
            0x59, 0x02, 0xFF, 0x5A, 0x13, 0x08, 0x2A, 0x06, 0x71, 0x2F, 0x24,
        ];
        let v = decode_dtcs(&svc);
        assert_eq!(v, vec![([0x5A, 0x13, 0x08], 0x2A), ([0x06, 0x71, 0x2F], 0x24)]);
    }
}
