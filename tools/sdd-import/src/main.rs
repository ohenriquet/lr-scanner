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
        /// Tipo de motor para desempatar live data (ex.: V6DIESEL).
        #[arg(long)]
        engine: Option<String>,
        /// Subtipo de motor (ex.: 3L).
        #[arg(long)]
        engine_sub: Option<String>,
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
    /// Mostra o help de diagnóstico de um DTC (descrição, sintomas, causas, ações).
    DtcHelp {
        /// Pacote SQLite.
        #[arg(long)]
        pack: PathBuf,
        /// Código do DTC (ex.: C1A13).
        #[arg(long)]
        code: String,
        /// Módulo (ex.: RLM). Sem isso, mostra todos os módulos do código.
        #[arg(long)]
        module: Option<String>,
    },
    /// Lista o live data (DIDs legíveis) de um módulo, com escala e unidade.
    DidDump {
        /// Pacote SQLite.
        #[arg(long)]
        pack: PathBuf,
        /// Sigla do módulo (ex.: PCM). Sem isso, resume por módulo.
        #[arg(long)]
        module: Option<String>,
        /// Filtra por substring no nome do parâmetro (ex.: temperatura).
        #[arg(long)]
        filter: Option<String>,
    },
    /// Exporta um pacote para JSON (consumido pelo app Flutter).
    PackExport {
        /// Pacote SQLite.
        #[arg(long)]
        pack: PathBuf,
        /// Arquivo JSON de saída.
        #[arg(long)]
        out: PathBuf,
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
            engine,
            engine_sub,
            out,
        } => {
            let model_year = my.unwrap_or_else(|| derive_my(&year));
            let spec = ImportSpec {
                model,
                model_year,
                year_code: year,
                langs: lang_order(&lang),
                engine,
                engine_sub,
            };
            let summary = sdd_data::import_vehicle(&install, &spec, &out)
                .with_context(|| format!("importar {} {}", spec.model, spec.year_code))?;
            tracing::info!(
                "pacote gravado em {}: {} módulos, {} redes, {} blocos CCF, {} parâmetros, {} opções, {} textos de DTC, {} DTCs com help, {} parâmetros de live data",
                out.display(),
                summary.modules,
                summary.networks,
                summary.ccf_blocks,
                summary.ccf_params,
                summary.ccf_options,
                summary.dtc_texts,
                summary.dtc_help,
                summary.did_params,
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
        Command::DtcHelp {
            pack,
            code,
            module,
        } => cmd_dtc_help(&pack, &code, module.as_deref()),
        Command::DidDump {
            pack,
            module,
            filter,
        } => cmd_did_dump(&pack, module.as_deref(), filter.as_deref()),
        Command::PackExport { pack, out } => cmd_pack_export(&pack, &out),
    }
}

/// Exporta o pacote para um JSON compacto que o app Flutter carrega.
fn cmd_pack_export(pack: &std::path::Path, out: &std::path::Path) -> Result<()> {
    use serde_json::{json, Map, Value};
    let conn = Connection::open(pack).with_context(|| format!("abrir {}", pack.display()))?;

    // vehicle
    let vehicle = conn.query_row(
        "SELECT model, model_year, year_code, platform_name FROM vehicle LIMIT 1",
        [],
        |r| {
            Ok(json!({
                "model": r.get::<_, String>(0)?,
                "model_year": r.get::<_, String>(1)?,
                "year_code": r.get::<_, String>(2)?,
                "platform": r.get::<_, Option<String>>(3)?,
            }))
        },
    )?;

    // modules
    let mut mstmt = conn.prepare(
        "SELECT acronym, name, tx_diag, rx_diag, network, fitment FROM module ORDER BY acronym",
    )?;
    let modules: Vec<Value> = mstmt
        .query_map([], |r| {
            Ok(json!({
                "acronym": r.get::<_, String>(0)?,
                "name": r.get::<_, Option<String>>(1)?,
                "tx": r.get::<_, Option<i64>>(2)?,
                "rx": r.get::<_, Option<i64>>(3)?,
                "network": r.get::<_, Option<String>>(4)?,
                "fitment": r.get::<_, Option<String>>(5)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    // live data por módulo
    let mut lstmt = conn.prepare(
        "SELECT d.module, d.did, p.name, p.unit, p.kind, p.multiplier, p.offset,
                p.offset_first, p.bit_level, p.byte_lsb, p.byte_msb, p.source
         FROM did d JOIN did_param p ON p.did_ref = d.id
         ORDER BY d.module, d.did, p.ord",
    )?;
    let mut live: Map<String, Value> = Map::new();
    let rows = lstmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<f64>>(5)?,
            r.get::<_, Option<f64>>(6)?,
            r.get::<_, Option<i64>>(7)?,
            r.get::<_, Option<i64>>(8)?,
            r.get::<_, Option<i64>>(9)?,
            r.get::<_, Option<i64>>(10)?,
            r.get::<_, Option<String>>(11)?,
        ))
    })?;
    for row in rows.filter_map(|r| r.ok()) {
        let (module, did, name, unit, kind, mult, off, off_first, bit, lsb, msb, source) = row;
        let entry = json!({
            "did": format!("0x{:04X}", did as u16),
            "name": name.unwrap_or_default(),
            "unit": unit,
            "kind": kind,
            "mult": mult,
            "offset": off,
            "offset_first": off_first.map(|v| v != 0),
            "bit": bit.map(|v| v != 0),
            "lsb": lsb,
            "msb": msb,
            "source": source,
        });
        live.entry(module)
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .unwrap()
            .push(entry);
    }

    // dtc_text (code -> desc)
    let mut tstmt = conn.prepare("SELECT code, description FROM dtc_text ORDER BY code")?;
    let mut dtc_text: Map<String, Value> = Map::new();
    for row in tstmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })?
        .filter_map(|r| r.ok())
    {
        if let Some(desc) = row.1 {
            dtc_text.insert(row.0, Value::String(desc));
        }
    }

    // dtc_help: code -> module -> {section: [..]}
    let mut hstmt = conn.prepare(
        "SELECT code, module, section, text FROM dtc_help ORDER BY code, module, section, ord",
    )?;
    let mut dtc_help: Map<String, Value> = Map::new();
    for row in hstmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?
        .filter_map(|r| r.ok())
    {
        let (code, module, section, text) = row;
        let by_mod = dtc_help
            .entry(code)
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .unwrap();
        let secs = by_mod
            .entry(module)
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .unwrap();
        secs.entry(section)
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .unwrap()
            .push(Value::String(text));
    }

    let doc = json!({
        "vehicle": vehicle,
        "modules": modules,
        "live": live,
        "dtc_text": dtc_text,
        "dtc_help": dtc_help,
    });
    let s = serde_json::to_string(&doc)?;
    std::fs::write(out, &s).with_context(|| format!("gravar {}", out.display()))?;
    tracing::info!(
        "exportado {} ({} módulos, {} grupos de live data, {} textos de DTC, {} códigos com help, {:.1} MB)",
        out.display(),
        doc["modules"].as_array().map_or(0, |a| a.len()),
        doc["live"].as_object().map_or(0, |o| o.len()),
        doc["dtc_text"].as_object().map_or(0, |o| o.len()),
        doc["dtc_help"].as_object().map_or(0, |o| o.len()),
        s.len() as f64 / 1e6,
    );
    Ok(())
}

/// Lista o live data de um pacote: resumo por módulo, ou os DIDs de um módulo.
fn cmd_did_dump(pack: &std::path::Path, module: Option<&str>, filter: Option<&str>) -> Result<()> {
    let conn = Connection::open(pack).with_context(|| format!("abrir {}", pack.display()))?;

    let Some(module) = module else {
        // Resumo por módulo.
        let mut stmt = conn.prepare(
            "SELECT module, COUNT(DISTINCT id) AS dids,
                    (SELECT COUNT(*) FROM did_param p WHERE p.did_ref IN
                        (SELECT id FROM did d2 WHERE d2.module = d.module)) AS params
             FROM did d GROUP BY module ORDER BY module",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        println!("Live data por módulo (DID = leitura UDS 0x22):");
        for row in rows {
            let (m, dids, params) = row?;
            println!("  {m:<6} {dids:>4} DIDs, {params:>5} parâmetros");
        }
        return Ok(());
    };

    let module = module.to_uppercase();
    let mut stmt = conn.prepare(
        "SELECT d.did, p.name, p.byte_lsb, p.byte_msb, p.mask, p.bit_level,
                p.kind, p.unit, p.multiplier, p.offset, p.offset_first, p.source
         FROM did d JOIN did_param p ON p.did_ref = d.id
         WHERE d.module = ?1
         ORDER BY d.did, p.ord",
    )?;
    let rows = stmt.query_map([&module], |r| {
        Ok(DidRow {
            did: r.get::<_, i64>(0)? as u16,
            name: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            kind: r.get::<_, Option<String>>(6)?,
            unit: r.get::<_, Option<String>>(7)?,
            mult: r.get::<_, Option<f64>>(8)?,
            off: r.get::<_, Option<f64>>(9)?,
            off_first: r.get::<_, Option<i64>>(10)?.unwrap_or(1) != 0,
            bit: r.get::<_, Option<i64>>(5)?.unwrap_or(0) != 0,
            lsb: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
            msb: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
            source: r.get::<_, Option<String>>(11)?.unwrap_or_default(),
        })
    })?;

    let mut n = 0;
    for row in rows {
        let r = row?;
        if let Some(f) = filter {
            if !r.name.to_lowercase().contains(&f.to_lowercase()) {
                continue;
            }
        }
        n += 1;
        let scale = match r.kind.as_deref() {
            Some("enum") => " (enum)".to_string(),
            Some("linear") => {
                let unit = r.unit.as_deref().unwrap_or("");
                let m = r.mult.unwrap_or(1.0);
                let o = r.off.unwrap_or(0.0);
                let formula = if r.off_first {
                    format!("(raw+{o})*{m}")
                } else {
                    format!("raw*{m}+{o}")
                };
                format!(" = {formula} {unit}")
            }
            _ => String::new(),
        };
        let loc = if r.source == "mdl" {
            format!("bits {}..{} cru", r.lsb, r.msb)
        } else if r.bit {
            format!("byte {}..{} bits", r.lsb, r.msb)
        } else {
            format!("byte {}..{}", r.lsb, r.msb)
        };
        println!("  0x{:04X}  {:<42} [{}]{}", r.did, r.name, loc, scale);
    }
    println!("\n{n} parâmetros em {module}.");
    Ok(())
}

struct DidRow {
    did: u16,
    name: String,
    kind: Option<String>,
    unit: Option<String>,
    mult: Option<f64>,
    off: Option<f64>,
    off_first: bool,
    bit: bool,
    lsb: i64,
    msb: i64,
    source: String,
}

/// Imprime o help de diagnóstico de um DTC (por módulo) a partir de um pacote.
fn cmd_dtc_help(pack: &std::path::Path, code: &str, module: Option<&str>) -> Result<()> {
    let conn = Connection::open(pack).with_context(|| format!("abrir {}", pack.display()))?;
    let code = code.to_uppercase();

    // Descrição curta (gdx), se houver.
    match conn.query_row(
        "SELECT description FROM dtc_text WHERE code = ?1",
        [&code],
        |r| r.get::<_, String>(0),
    ) {
        Ok(desc) => println!("{code}  {desc}"),
        Err(_) => println!("{code}"),
    }

    // Módulos com help para este código.
    let mut mstmt = conn.prepare(
        "SELECT DISTINCT module FROM dtc_help WHERE code = ?1 ORDER BY module",
    )?;
    let all_modules: Vec<String> = mstmt
        .query_map([&code], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    let modules: Vec<String> = match module {
        Some(m) => all_modules
            .into_iter()
            .filter(|x| x.eq_ignore_ascii_case(m))
            .collect(),
        None => all_modules,
    };
    if modules.is_empty() {
        println!("\n(sem help de diagnóstico para {code} neste pacote)");
        return Ok(());
    }

    let titles = [
        ("supp_desc", "Descrição complementar"),
        ("symptoms", "Sintomas"),
        ("causes", "Causas prováveis"),
        ("actions", "Ações recomendadas"),
    ];
    for m in modules {
        println!("\n=== módulo {m} ===");
        for (sec, title) in titles {
            let mut stmt = conn.prepare(
                "SELECT text FROM dtc_help WHERE code = ?1 AND module = ?2 AND section = ?3 ORDER BY ord",
            )?;
            let rows: Vec<String> = stmt
                .query_map(rusqlite::params![code, m, sec], |r| r.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            if rows.is_empty() {
                continue;
            }
            println!("{title}:");
            for t in rows {
                println!("  • {t}");
            }
        }
    }
    Ok(())
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
