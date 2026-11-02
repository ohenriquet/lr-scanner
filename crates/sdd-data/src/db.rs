//! Escritor do pacote SQLite de um veículo+ano.
//!
//! O esquema é propositalmente simples e legível; a UI e o crate `ccf`
//! consultam estas tabelas. Textos já vêm resolvidos no idioma preferido.

use std::path::Path;

use rusqlite::{params, Connection};

use crate::ccf::CcfData;
use crate::model::CcfRole;
use crate::platform::Platform;
use crate::text::TextResolver;

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;

CREATE TABLE vehicle (
    model         TEXT NOT NULL,
    model_year    TEXT NOT NULL,
    year_code     TEXT NOT NULL,
    platform_name TEXT
);

CREATE TABLE network (
    net_id        TEXT NOT NULL,
    main_net      TEXT,
    rate_kbps     INTEGER,
    diag_protocol TEXT,
    net_protocol  TEXT
);

CREATE TABLE module (
    acronym   TEXT NOT NULL,
    iso_name  TEXT,
    system_id TEXT NOT NULL,
    tx_diag   INTEGER,
    rx_diag   INTEGER,
    tx_prog   INTEGER,
    rx_prog   INTEGER,
    network   TEXT,
    fitment   TEXT,
    name      TEXT            -- texto resolvido do tm_id
);
CREATE INDEX idx_module_acronym ON module(acronym);

CREATE TABLE ccf_source (
    role             TEXT NOT NULL,   -- master | backup | as_built
    module           TEXT NOT NULL,
    upload           TEXT,
    engine_qualifier TEXT
);

CREATE TABLE ccf_block (
    block_id    TEXT NOT NULL,
    name        TEXT,
    service_rd  INTEGER,
    service_wr  INTEGER,
    length      INTEGER
);

CREATE TABLE ccf_address (
    block_id         TEXT NOT NULL,
    module           TEXT NOT NULL,
    t_ccf            TEXT,
    parameter_id     INTEGER,
    start_address    INTEGER,
    stop_address     INTEGER,
    merge            INTEGER,
    engine_qualifier TEXT
);

CREATE TABLE ccf_param (
    id            INTEGER PRIMARY KEY,
    block_id      TEXT NOT NULL,
    grp           TEXT,
    name          TEXT NOT NULL,
    raw_id        TEXT,
    byte_start    INTEGER,
    byte_stop     INTEGER,
    bit_start     INTEGER,
    bit_stop      INTEGER,
    mask          INTEGER,
    ptype         TEXT,
    display       INTEGER,
    editable      INTEGER,
    scope         TEXT,
    title         TEXT             -- texto resolvido (ou literal)
);
CREATE INDEX idx_param_name ON ccf_param(name);
CREATE INDEX idx_param_group ON ccf_param(grp);

CREATE TABLE ccf_option (
    param_id  INTEGER NOT NULL REFERENCES ccf_param(id),
    value     INTEGER NOT NULL,
    name      TEXT,
    code      TEXT,
    label     TEXT              -- texto resolvido (ou literal)
);
CREATE INDEX idx_option_param ON ccf_option(param_id);
"#;

/// Cria (ou recria) o pacote SQLite e grava todos os dados.
pub fn write_pack(
    out_db: &Path,
    platform: &Platform,
    ccf: &CcfData,
    resolver: &mut TextResolver,
) -> Result<(), crate::DataError> {
    if out_db.exists() {
        std::fs::remove_file(out_db).map_err(|e| crate::DataError::Io(out_db.to_path_buf(), e))?;
    }
    let mut conn = Connection::open(out_db)?;
    conn.execute_batch(SCHEMA)?;

    let tx = conn.transaction()?;

    // vehicle
    tx.execute(
        "INSERT INTO vehicle (model, model_year, year_code, platform_name) VALUES (?,?,?,?)",
        params![
            platform.vehicle.model,
            platform.vehicle.model_year,
            platform.vehicle.year_code,
            platform.vehicle.platform_name,
        ],
    )?;

    // networks
    for n in &platform.networks {
        tx.execute(
            "INSERT INTO network (net_id, main_net, rate_kbps, diag_protocol, net_protocol)
             VALUES (?,?,?,?,?)",
            params![
                n.net_id,
                n.main_net,
                n.rate_kbps,
                n.diag_protocol,
                n.net_protocol
            ],
        )?;
    }

    // modules (resolve nome)
    for m in &platform.modules {
        let name = m.tm_id.as_deref().and_then(|id| resolver.resolve(id));
        tx.execute(
            "INSERT INTO module
             (acronym, iso_name, system_id, tx_diag, rx_diag, tx_prog, rx_prog, network, fitment, name)
             VALUES (?,?,?,?,?,?,?,?,?,?)",
            params![
                m.acronym, m.iso_name, m.system_id,
                m.tx_diag, m.rx_diag, m.tx_prog, m.rx_prog,
                m.network, m.fitment, name,
            ],
        )?;
    }

    // ccf sources
    for s in &platform.ccf_sources {
        tx.execute(
            "INSERT INTO ccf_source (role, module, upload, engine_qualifier) VALUES (?,?,?,?)",
            params![role_str(s.role), s.module, s.upload, s.engine_qualifier],
        )?;
    }

    // ccf blocks / addresses
    for b in &ccf.blocks {
        tx.execute(
            "INSERT INTO ccf_block (block_id, name, service_rd, service_wr, length)
             VALUES (?,?,?,?,?)",
            params![b.block_id, b.name, b.service_rd, b.service_wr, b.length],
        )?;
    }
    for a in &ccf.addresses {
        tx.execute(
            "INSERT INTO ccf_address
             (block_id, module, t_ccf, parameter_id, start_address, stop_address, merge, engine_qualifier)
             VALUES (?,?,?,?,?,?,?,?)",
            params![
                a.block_id, a.module, a.t_ccf, a.parameter_id,
                a.start_address, a.stop_address, a.merge as i64, a.engine_qualifier,
            ],
        )?;
    }

    // ccf params + options
    for (i, p) in ccf.params.iter().enumerate() {
        let param_id = (i + 1) as i64;
        let title = p
            .title_tm
            .as_deref()
            .and_then(|id| resolver.resolve(id))
            .or_else(|| p.title_literal.clone());
        tx.execute(
            "INSERT INTO ccf_param
             (id, block_id, grp, name, raw_id, byte_start, byte_stop, bit_start, bit_stop,
              mask, ptype, display, editable, scope, title)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            params![
                param_id,
                p.block_id,
                p.group,
                p.name,
                p.raw_id,
                p.byte_start,
                p.byte_stop,
                p.bit_start,
                p.bit_stop,
                p.mask,
                p.ptype,
                p.display as i64,
                p.editable as i64,
                p.scope,
                title,
            ],
        )?;
        for o in &p.options {
            let label = o
                .tm_id
                .as_deref()
                .and_then(|id| resolver.resolve(id))
                .or_else(|| o.tm_literal.clone());
            tx.execute(
                "INSERT INTO ccf_option (param_id, value, name, code, label) VALUES (?,?,?,?,?)",
                params![param_id, o.value, o.name, o.code, label],
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}

fn role_str(r: CcfRole) -> &'static str {
    match r {
        CcfRole::Master => "master",
        CcfRole::Backup => "backup",
        CcfRole::AsBuilt => "as_built",
    }
}
