//! Testes de integração contra os arquivos reais da instalação do SDD.
//!
//! Estes testes só rodam quando o SDD está instalado no caminho padrão
//! (`C:\Program Files (x86)\JLR\IDS\Xml`). Quando não está, cada teste é
//! ignorado com uma mensagem, para que a suíte continue passando em máquinas
//! sem o SDD (por exemplo, em CI).

use std::path::{Path, PathBuf};

fn xml_dir() -> Option<PathBuf> {
    let p = PathBuf::from(r"C:\Program Files (x86)\JLR\IDS\Xml");
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
}

/// O par plano/cifrado existente na instalação: decifrar o `.exml` deve
/// reproduzir exatamente o `.xml` plano ao lado dele.
#[test]
fn decifra_par_conhecido_jlr_vsv_config() {
    let Some(dir) = xml_dir() else {
        eprintln!("SDD não instalado; teste ignorado");
        return;
    };
    let enc = std::fs::read(dir.join("JLR_VSV_CONFIG.exml")).expect("ler .exml");
    let plano = std::fs::read(dir.join("JLR_VSV_CONFIG.xml")).expect("ler .xml");
    let dec = sdd_crypto::decrypt(&enc).expect("decifrar");
    assert_eq!(
        dec, plano,
        "o texto decifrado deve bater byte a byte com o plano"
    );
}

/// Arquivos-chave do L319 devem decifrar para XML bem-formado no início.
#[test]
fn decifra_arquivos_l319() {
    let Some(dir) = xml_dir() else {
        eprintln!("SDD não instalado; teste ignorado");
        return;
    };
    let casos: &[(&str, &str)] = &[
        ("CCF_DATA_L319_201000.exml", "<?xml"),
        ("PLATFORM_L319_201000.exml", "<?xml"),
        ("Security.exml", "<?xml"),
        ("VINDecode.exml", "<?xml"),
    ];
    for (nome, prefixo) in casos {
        let caminho = dir.join(nome);
        if !caminho.exists() {
            eprintln!("{nome} ausente; pulando");
            continue;
        }
        let enc = std::fs::read(&caminho).unwrap_or_else(|_| panic!("ler {nome}"));
        let dec = sdd_crypto::decrypt(&enc).unwrap_or_else(|e| panic!("decifrar {nome}: {e}"));
        let inicio = String::from_utf8_lossy(&dec[..dec.len().min(32)]);
        assert!(
            inicio.trim_start().starts_with(prefixo),
            "{nome} deveria começar com {prefixo:?}, veio {inicio:?}"
        );
    }
}

/// Varredura ampla: todo `.exml` da raiz deve decifrar sem erro de bloco ou
/// de preenchimento. Confirma que a chave vale para toda a base, não só L319.
#[test]
fn decifra_toda_a_raiz_sem_erro() {
    let Some(dir) = xml_dir() else {
        eprintln!("SDD não instalado; teste ignorado");
        return;
    };
    let mut ok = 0usize;
    let mut falhas = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("listar Xml") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("exml") {
            continue;
        }
        match decifra_arquivo(&path) {
            Ok(()) => ok += 1,
            Err(e) => falhas.push(format!(
                "{}: {e}",
                path.file_name().unwrap().to_string_lossy()
            )),
        }
    }
    assert!(ok > 0, "nenhum .exml encontrado na raiz");
    assert!(falhas.is_empty(), "{} falhas: {:?}", falhas.len(), falhas);
    eprintln!("{ok} arquivos .exml decifrados sem erro");
}

fn decifra_arquivo(path: &Path) -> Result<(), sdd_crypto::ExmlError> {
    let enc = std::fs::read(path).map_err(|_| sdd_crypto::ExmlError::Empty)?;
    sdd_crypto::decrypt(&enc).map(|_| ())
}
