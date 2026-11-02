//! Parsers dos dados do JLR SDD e geração de pacotes SQLite.
//!
//! Lê a instalação local do SDD (decifrando os `.exml` com [`sdd_crypto`]) e
//! produz um banco SQLite por veículo+ano com módulos, endereços e o mapa do
//! CCF. Nenhum dado da JLR é embutido no binário: tudo vem da máquina do
//! usuário em tempo de importação.

pub mod ccf;
pub mod db;
pub mod model;
pub mod platform;
pub mod text;

use std::path::{Path, PathBuf};

/// Erros da camada de dados.
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("E/S em {0}: {1}")]
    Io(PathBuf, #[source] std::io::Error),
    #[error("falha ao decifrar {0}: {1}")]
    Crypto(PathBuf, #[source] sdd_crypto::ExmlError),
    #[error("XML inválido: {0}")]
    Xml(#[from] quick_xml::de::DeError),
    #[error("SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("arquivo não encontrado: {0}")]
    NotFound(PathBuf),
    #[error("{0}")]
    Other(String),
}

/// Localização de uma instalação do SDD.
#[derive(Debug, Clone)]
pub struct SddInstall {
    /// Raiz (ex.: `C:\Program Files (x86)\JLR`).
    pub root: PathBuf,
}

impl SddInstall {
    /// Usa o caminho padrão de instalação no Windows.
    pub fn default_windows() -> Self {
        Self {
            root: PathBuf::from(r"C:\Program Files (x86)\JLR"),
        }
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Pasta dos XML de definição (`IDS\Xml`).
    pub fn xml_dir(&self) -> PathBuf {
        self.root.join("IDS").join("Xml")
    }

    /// Pasta de textos (`IDS\Xml\text`).
    pub fn text_dir(&self) -> PathBuf {
        self.xml_dir().join("text")
    }

    /// Existe de fato?
    pub fn exists(&self) -> bool {
        self.xml_dir().is_dir()
    }
}

/// Lê e decifra um arquivo `.exml`.
pub fn read_exml(path: &Path) -> Result<String, DataError> {
    let bytes = std::fs::read(path).map_err(|e| DataError::Io(path.to_path_buf(), e))?;
    let xml = sdd_crypto::decrypt(&bytes).map_err(|e| DataError::Crypto(path.to_path_buf(), e))?;
    String::from_utf8(xml)
        .map_err(|e| DataError::Other(format!("{} não é UTF-8: {e}", path.display())))
}

/// Parâmetros de uma importação de veículo+ano.
#[derive(Debug, Clone)]
pub struct ImportSpec {
    pub model: String,
    pub model_year: String,
    pub year_code: String,
    /// Ordem de idiomas para resolver textos (ex.: `["ptb","ptg","eng"]`).
    pub langs: Vec<String>,
}

/// Importa um veículo+ano para um pacote SQLite novo em `out_db`.
///
/// Retorna um resumo (contagens) para log.
pub fn import_vehicle(
    install: &SddInstall,
    spec: &ImportSpec,
    out_db: &Path,
) -> Result<ImportSummary, DataError> {
    let xml_dir = install.xml_dir();

    // PLATFORM_<MODEL>_<YEAR>.exml
    let platform_path = xml_dir.join(format!("PLATFORM_{}_{}.exml", spec.model, spec.year_code));
    if !platform_path.exists() {
        return Err(DataError::NotFound(platform_path));
    }
    let platform_xml = read_exml(&platform_path)?;
    let platform = platform::parse(
        &platform_xml,
        &spec.model,
        &spec.model_year,
        &spec.year_code,
    )?;

    // CCF_DATA_<MODEL>_<YEAR>.exml (pode não existir p/ alguns veículos).
    let ccf_path = xml_dir.join(format!("CCF_DATA_{}_{}.exml", spec.model, spec.year_code));
    let ccf = if ccf_path.exists() {
        let ccf_xml = read_exml(&ccf_path)?;
        ccf::parse(&ccf_xml)?
    } else {
        ccf::CcfData::default()
    };

    let mut resolver = text::TextResolver::new(install.text_dir(), spec.langs.clone());

    let summary = ImportSummary {
        modules: platform.modules.len(),
        networks: platform.networks.len(),
        ccf_blocks: ccf.blocks.len(),
        ccf_params: ccf.params.len(),
        ccf_options: ccf.params.iter().map(|p| p.options.len()).sum(),
    };

    db::write_pack(out_db, &platform, &ccf, &mut resolver)?;

    Ok(summary)
}

/// Contagens de uma importação, para log e verificação.
#[derive(Debug, Clone, Copy)]
pub struct ImportSummary {
    pub modules: usize,
    pub networks: usize,
    pub ccf_blocks: usize,
    pub ccf_params: usize,
    pub ccf_options: usize,
}
