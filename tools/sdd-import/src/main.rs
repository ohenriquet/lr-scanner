//! CLI de importação dos dados do JLR SDD.
//!
//! Lê a instalação local do SDD e gera pacotes SQLite por veículo+ano. Também
//! lista os veículos disponíveis e imprime um resumo do CCF (útil para
//! verificar a importação sem uma UI).

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rusqlite::Connection;
use sdd_data::{ImportSpec, SddInstall};

/// Mapeia um apelido de idioma amigável para a ordem de códigos do SDD.
fn lang_order(alias: &str) -> Vec<String> {
    match alias {
        "pt" | "ptb" | "pt-br" => vec!["ptb".into(), "ptg".into(), "eng".into()],
        "ptg" | "pt-pt" => vec!["ptg".into(), "ptb".into(), "eng".into()],
        "en" | "eng" => vec!["eng".into()],
        other => vec![other.to_string(), "eng".into()],
    }
}

#[derive(Parser)]
#[command(name = "sdd-import", about = "Importa o JLR SDD em pacotes SQLite")]
struct Cli {
    /// Raiz da instalação do SDD (padrão: C:\Program Files (x86)\JLR).
    #[arg(long, global = true)]
    sdd: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Lista os veículos+ano disponíveis na instalação.
    List,
    /// Importa um veículo+ano para um pacote SQLite.
    Vehicle {
        /// Código do modelo (ex.: L319).
        #[arg(long)]
        model: String,
        /// Ano numérico da pasta (ex.: 201000).
        #[arg(long)]
        year: String,
        /// Ano-modelo interno (ex.: MY10). Se omitido, é derivado do ano.
        #[arg(long)]
        my: Option<String>,
        /// Idioma preferido (pt, en, ...).
        #[arg(long, default_value = "pt")]
        lang: String,
        /// Caminho do pacote SQLite de saída.
        #[arg(long)]
        out: PathBuf,
    },
    /// Imprime os parâmetros de CCF de um pacote (filtro opcional por texto).
    CcfDump {
        /// Pacote SQLite.
        #[arg(long)]
        pack: PathBuf,
        /// Filtra por substring no nome/título/grupo (ex.: LIGHT).
        #[arg(long)]
        filter: Option<String>,
    },
    /// Decodifica um CCF bruto (.bin lido do carro) contra um pacote.
    CcfDecode {
        /// Pacote SQLite do veículo/ano certo.
        #[arg(long)]
        pack: PathBuf,
        /// Arquivo .bin com o CCF lido do BCM (resposta sem o 62 DE 00).
        #[arg(long)]
        ccf: PathBuf,
        /// Mostra só parâmetros cujo nome/título/grupo contenham este texto.
        #[arg(long)]
        filter: Option<String>,
        /// Mostra todos (por padrão, oculta os "Indefinida"/sem opção).
        #[arg(long)]
        all: bool,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    let install = match &cli.sdd {
        Some(p) => SddInstall::new(p.clone()),
        None => SddInstall::default_windows(),
    };

    match cli.cmd {
        Command::List => cmd_list(&install),
        Command::Vehicle {
            model,
            year,
            my,
            lang,
            out,
        } => {
            let model_year = my.unwrap_or_else(|| derive_my(&year));
            let spec = ImportSpec {
                model,
                model_year,
                year_code: year,
                langs: lang_order(&lang),
            };
            let summary = sdd_data::import_vehicle(&install, &spec, &out)
                .with_context(|| format!("importar {} {}", spec.model, spec.year_code))?;
            tracing::info!(
                "pacote gravado em {}: {} módulos, {} redes, {} blocos CCF, {} parâmetros, {} opções",
                out.display(),
                summary.modules,
                summary.networks,
                summary.ccf_blocks,
                summary.ccf_params,
                summary.ccf_options,
            );
            Ok(())
        }
        Command::CcfDump { pack, filter } => cmd_ccf_dump(&pack, filter.as_deref()),
        Command::CcfDecode {
            pack,
            ccf,
            filter,
            all,
        } => cmd_ccf_decode(&pack, &ccf, filter.as_deref(), all),
    }
}

/// Deriva um MYxx a partir do ano numérico (ex.: 201000 -> MY10).
fn derive_my(year: &str) -> String {
    if year.len() >= 4 {
        let yy = &year[2..4];
        format!("MY{yy}")
    } else {
        year.to_string()
    }
}

fn cmd_list(install: &SddInstall) -> Result<()> {
    if !install.exists() {
        anyhow::bail!("SDD não encontrado em {}", install.xml_dir().display());
    }
    let dir = install.xml_dir();
    let mut vehicles: Vec<(String, String, bool)> = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // PLATFORM_<MODEL>_<YEAR>.exml
        if let Some(rest) = name
            .strip_prefix("PLATFORM_")
            .and_then(|r| r.strip_suffix(".exml"))
        {
            if let Some((model, year)) = rest.rsplit_once('_') {
                let ccf = dir.join(format!("CCF_DATA_{model}_{year}.exml")).exists();
                vehicles.push((model.to_string(), year.to_string(), ccf));
            }
        }
    }
    vehicles.sort();
    println!("{:<8} {:<8} {:<8}", "MODELO", "ANO", "CCF?");
    for (model, year, ccf) in &vehicles {
        println!(
            "{:<8} {:<8} {:<8}",
            model,
            year,
            if *ccf { "sim" } else { "-" }
        );
    }
    println!("\n{} veículos+ano com PLATFORM.", vehicles.len());
    Ok(())
}

fn cmd_ccf_dump(pack: &PathBuf, filter: Option<&str>) -> Result<()> {
    let conn = Connection::open(pack).with_context(|| format!("abrir {}", pack.display()))?;
    let like = filter
        .map(|f| format!("%{}%", f.to_uppercase()))
        .unwrap_or_else(|| "%".into());

    let mut stmt = conn.prepare(
        "SELECT id, grp, name, raw_id, mask, ptype, editable, title
         FROM ccf_param
         WHERE UPPER(name) LIKE ?1 OR UPPER(IFNULL(grp,'')) LIKE ?1 OR UPPER(IFNULL(title,'')) LIKE ?1
         ORDER BY byte_start, bit_start",
    )?;

    let rows = stmt.query_map([&like], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<i64>>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, i64>(6)?,
            r.get::<_, Option<String>>(7)?,
        ))
    })?;

    let mut count = 0;
    for row in rows {
        let (id, grp, name, raw_id, mask, ptype, editable, title) = row?;
        count += 1;
        println!(
            "\n[{}] {} {}{}",
            raw_id.unwrap_or_default(),
            name,
            title.map(|t| format!("— {t} ")).unwrap_or_default(),
            if editable == 1 { "(editável)" } else { "" },
        );
        println!(
            "    tipo={} máscara={} grupo={}",
            ptype.unwrap_or_default(),
            mask.map(|m| format!("0x{m:02X}")).unwrap_or_default(),
            grp.unwrap_or_default(),
        );
        let mut ostmt = conn.prepare(
            "SELECT value, name, label FROM ccf_option WHERE param_id=?1 ORDER BY value",
        )?;
        let opts = ostmt.query_map([id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?;
        for o in opts {
            let (value, oname, label) = o?;
            println!(
                "      0x{:02X} {} {}",
                value,
                oname.unwrap_or_default(),
                label.map(|l| format!("— {l}")).unwrap_or_default(),
            );
        }
    }
    println!("\n{count} parâmetros.");
    Ok(())
}

/// Decodifica um CCF bruto contra o pacote: para cada parâmetro, calcula o
/// offset absoluto na imagem (base do bloco + byte do parâmetro) e mostra a
/// opção correspondente ao valor atual.
fn cmd_ccf_decode(
    pack: &PathBuf,
    ccf_path: &PathBuf,
    filter: Option<&str>,
    all: bool,
) -> Result<()> {
    let ccf = std::fs::read(ccf_path).with_context(|| format!("ler {}", ccf_path.display()))?;
    let conn = Connection::open(pack).with_context(|| format!("abrir {}", pack.display()))?;

    // Offset de cada bloco na imagem do DID = menor start_address do bloco no
    // BCM menos o start_address do bloco 1 (a base da imagem).
    let base_addr: i64 = conn
        .query_row(
            "SELECT MIN(start_address) FROM ccf_address WHERE module LIKE 'BCM%'",
            [],
            |r| r.get(0),
        )
        .context("sem endereços de CCF no BCM")?;
    let mut block_base: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT block_id, MIN(start_address) FROM ccf_address
             WHERE module LIKE 'BCM%' AND start_address IS NOT NULL GROUP BY block_id",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        for row in rows {
            let (bid, sa) = row?;
            block_base.insert(bid, sa - base_addr);
        }
    }

    let like = filter
        .map(|f| format!("%{}%", f.to_uppercase()))
        .unwrap_or_else(|| "%".into());
    let mut stmt = conn.prepare(
        "SELECT id, block_id, name, byte_start, mask, title, ptype FROM ccf_param
         WHERE UPPER(name) LIKE ?1 OR UPPER(IFNULL(grp,'')) LIKE ?1 OR UPPER(IFNULL(title,'')) LIKE ?1
         ORDER BY block_id, byte_start, bit_start",
    )?;
    let rows = stmt.query_map([&like], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, Option<i64>>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, Option<String>>(6)?,
        ))
    })?;

    let mut count = 0;
    for row in rows {
        let (id, block_id, name, byte_start, mask, title, ptype) = row?;
        let Some(&base) = block_base.get(&block_id) else {
            continue;
        };
        let off = (base + byte_start) as usize;
        let Some(&raw) = ccf.get(off) else { continue };
        let mask = mask.unwrap_or(0xFF) as u8;
        let val = raw & mask;

        // Opção correspondente ao valor mascarado.
        let opt: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT name, label FROM ccf_option WHERE param_id=?1 AND value=?2",
                rusqlite::params![id, val as i64],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();

        let atual = match &opt {
            Some((n, l)) => l.clone().unwrap_or_else(|| n.clone()),
            None => format!("(valor 0x{val:02X})"),
        };
        // Sem --all, oculta "Indefinida" e sem opção, para focar no que importa.
        if !all {
            let low = atual.to_lowercase();
            if opt.is_none() || low.contains("indefinid") {
                continue;
            }
        }
        count += 1;
        let rotulo = title.clone().unwrap_or_else(|| name.clone());
        println!(
            "\n• {rotulo}\n    atual: {atual}   [byte {byte_start} bloco {block_id}, bruto 0x{raw:02X}, tipo {}]",
            ptype.unwrap_or_default()
        );
    }
    println!("\n{count} parâmetros.");
    Ok(())
}
