//! Verificação de ponta a ponta: importar o L319 MY10 da instalação real e
//! conferir fatos conhecidos do veículo. Ignorado quando o SDD não está
//! instalado no caminho padrão.

use rusqlite::Connection;
use sdd_data::{ImportSpec, SddInstall};

fn install() -> Option<SddInstall> {
    let i = SddInstall::default_windows();
    if i.exists() {
        Some(i)
    } else {
        eprintln!("SDD não instalado; teste ignorado");
        None
    }
}

fn import_tmp(install: &SddInstall) -> (Connection, tempdir_guard::TempFile) {
    let tmp = tempdir_guard::TempFile::new("l319_test.sqlite");
    let spec = ImportSpec {
        model: "L319".into(),
        model_year: "MY10".into(),
        year_code: "201000".into(),
        langs: vec!["ptb".into(), "ptg".into(), "eng".into()],
        engine: Some("V6DIESEL".into()),
        engine_sub: Some("3L".into()),
    };
    sdd_data::import_vehicle(install, &spec, tmp.path()).expect("importar L319 MY10");
    (Connection::open(tmp.path()).unwrap(), tmp)
}

#[test]
fn modulos_e_enderecos_conhecidos() {
    let Some(install) = install() else { return };
    let (conn, _tmp) = import_tmp(&install);

    // BCM: mestre do CCF, HS-CAN, tx 0x726 / rx 0x72E.
    let (tx, rx, net): (i64, i64, String) = conn
        .query_row(
            "SELECT tx_diag, rx_diag, network FROM module WHERE acronym='BCM'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("BCM presente");
    assert_eq!(tx, 0x726, "BCM tx");
    assert_eq!(rx, 0x72E, "BCM rx");
    assert_eq!(net, "CAN_HS");

    // PCM na HS-CAN em 0x7E0.
    let pcm_tx: i64 = conn
        .query_row("SELECT tx_diag FROM module WHERE acronym='PCM'", [], |r| {
            r.get(0)
        })
        .expect("PCM presente");
    assert_eq!(pcm_tx, 0x7E0);

    // Deve haver 45 módulos no fitment do L319 MY10.
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM module", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 45, "quantidade de módulos");
}

#[test]
fn ccf_mestre_no_bcm_por_did() {
    let Some(install) = install() else { return };
    let (conn, _tmp) = import_tmp(&install);

    let (module, upload): (String, String) = conn
        .query_row(
            "SELECT module, upload FROM ccf_source WHERE role='master'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("fonte mestre do CCF");
    assert!(module.starts_with("BCM"), "mestre = BCM");
    assert_eq!(upload, "did");

    // Bloco 1 lido por 0x22 e gravado por 0x2E, 196 bytes.
    let (rd, wr, len): (i64, i64, i64) = conn
        .query_row(
            "SELECT service_rd, service_wr, length FROM ccf_block WHERE block_id='1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(rd, 0x22);
    assert_eq!(wr, 0x2E);
    assert_eq!(len, 196);

    // Endereço do CCF no BCM: DID 0xDE00 em 0x00FFAC00.
    let (did, start): (i64, i64) = conn
        .query_row(
            "SELECT parameter_id, start_address FROM ccf_address
             WHERE module LIKE 'BCM%' AND parameter_id=0xDE00 LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("endereço do CCF no BCM");
    assert_eq!(did, 0xDE00);
    assert_eq!(start, 0x00FFAC00);
}

#[test]
fn parametro_approach_lamp_com_bits_e_opcoes() {
    let Some(install) = install() else { return };
    let (conn, _tmp) = import_tmp(&install);

    // O parâmetro de luzes de acesso: byte 47, bit 2, máscara 0x04, BOOL.
    let (raw_id, byte_start, bit_start, mask, ptype): (String, i64, i64, i64, String) = conn
        .query_row(
            "SELECT raw_id, byte_start, bit_start, mask, ptype FROM ccf_param
             WHERE name='PARAM_CCF_INT_LIGHT_APPROACH_LAMP'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .expect("parâmetro approach lamp");
    assert_eq!(raw_id, "047_047_002_002");
    assert_eq!(byte_start, 47);
    assert_eq!(bit_start, 2);
    assert_eq!(mask, 0x04);
    assert_eq!(ptype, "BOOL");

    // Duas opções: 0x00 (FALSE) e 0x04 (TRUE).
    let id: i64 = conn
        .query_row(
            "SELECT id FROM ccf_param WHERE name='PARAM_CCF_INT_LIGHT_APPROACH_LAMP'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let mut stmt = conn
        .prepare("SELECT value FROM ccf_option WHERE param_id=? ORDER BY value")
        .unwrap();
    let valores: Vec<i64> = stmt
        .query_map([id], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    assert_eq!(valores, vec![0x00, 0x04]);
}

/// Pequeno utilitário local para um arquivo temporário autodestruível, sem
/// dependências externas.
mod tempdir_guard {
    use std::path::{Path, PathBuf};

    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    pub struct TempFile(PathBuf);

    impl TempFile {
        pub fn new(name: &str) -> Self {
            let mut p = std::env::temp_dir();
            let pid = std::process::id();
            let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            p.push(format!("{pid}_{nanos}_{seq}_{name}"));
            Self(p)
        }
        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(self.0.with_extension("sqlite-wal"));
            let _ = std::fs::remove_file(self.0.with_extension("sqlite-shm"));
        }
    }
}
