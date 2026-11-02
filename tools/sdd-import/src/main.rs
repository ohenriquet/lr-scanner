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
