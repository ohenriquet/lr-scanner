//! Parsers dos dados do JLR SDD e geração de pacotes SQLite.
//!
//! Lê a instalação local do SDD (decifrando os `.exml` com [`sdd_crypto`]) e
//! produz um banco SQLite por veículo+ano com módulos, endereços e o mapa do
//! CCF. Nenhum dado da JLR é embutido no binário: tudo vem da máquina do
//! usuário em tempo de importação.

pub mod ccf;
pub mod db;
pub mod did;
pub mod dtc;
pub mod dtc_help;
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

    /// Arquivo global de descrições de DTC (`IDS\Xml\gdx.xml`, texto plano).
    pub fn gdx_path(&self) -> PathBuf {
        self.xml_dir().join("gdx.xml")
    }

    /// Pasta de dados por idioma (`Common\SDD\Data`).
    pub fn sdd_data_dir(&self) -> PathBuf {
        self.root.join("Common").join("SDD").join("Data")
    }

    /// Pasta do help de DTC para um idioma (`Common\SDD\Data\<lang>\rds-dtc-help`).
    pub fn rds_dtc_help_dir(&self, lang: &str) -> PathBuf {
        self.sdd_data_dir().join(lang).join("rds-dtc-help")
    }

    /// Pasta gradex de um idioma (`Common\SDD\Data\<lang>\gradex`): dentro dela
    /// `Snapshot/` tem os DID Formatting e `Converters/` os conversores.
    pub fn gradex_dir(&self, lang: &str) -> PathBuf {
        self.sdd_data_dir().join(lang).join("gradex")
    }

    /// Caminho do `mdl.xml` (dicionário de PIDs, liga DID->módulos). Fica em
    /// `ProgramData`, fora da raiz da instalação; usa o caminho padrão do Windows.
    pub fn mdl_path(&self) -> PathBuf {
        PathBuf::from(r"C:\ProgramData\JLR\IDS\OnlinePIDHelp\Data\PROCS\mdl.xml")
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
    /// Tipo de motor para desempatar live data por motor (ex.: `V6DIESEL`).
    pub engine: Option<String>,
    /// Subtipo de motor (ex.: `3L`).
    pub engine_sub: Option<String>,
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

    // gdx.xml: descrições de DTC (global, texto plano). Opcional — se faltar,
    // os DTCs aparecem só como código.
    let gdx_path = install.gdx_path();
    let dtc_texts = if gdx_path.exists() {
        let xml =
            std::fs::read_to_string(&gdx_path).map_err(|e| DataError::Io(gdx_path.clone(), e))?;
        dtc::parse(&xml)?
    } else {
        Vec::new()
    };

    // Help de diagnóstico por DTC (pt_br preferido, en de reserva), indexado por
    // módulo+código. Só os códigos que se aplicam a um módulo do veículo entram.
    let dtc_help = load_dtc_help(install, spec, &platform.modules)?;

    // Live data por módulo (DIDs + conversores), filtrado para este veículo.
    let did_catalog = load_did_catalog(install, spec, &platform.modules)?;

    let summary = ImportSummary {
        modules: platform.modules.len(),
        networks: platform.networks.len(),
        ccf_blocks: ccf.blocks.len(),
        ccf_params: ccf.params.len(),
        ccf_options: ccf.params.iter().map(|p| p.options.len()).sum(),
        dtc_texts: dtc_texts.len(),
        dtc_help: dtc_help.len(),
        did_params: did_catalog.entries.iter().map(|e| e.params.len()).sum(),
    };

    db::write_pack(
        out_db,
        &platform,
        &ccf,
        &dtc_texts,
        &dtc_help,
        &did_catalog,
        &mut resolver,
    )?;

    Ok(summary)
}

/// Carrega o catálogo de live data (DIDs legíveis por módulo + conversores).
///
/// Usa os "DID Formatting" qualificados por módulo e por módulo+modelo+ano+motor
/// (os "Unqualified", sem módulo, não entram: não há como endereçar a leitura).
/// Fica com um DID por (módulo, id), preferindo o de maior qualificação e o do
/// motor do veículo. Idioma: pt_br (nomes/rótulos) com fallback en.
fn load_did_catalog(
    install: &SddInstall,
    spec: &ImportSpec,
    modules: &[model::Module],
) -> Result<did::Catalog, DataError> {
    use std::collections::{HashMap, HashSet};

    let lang = ["pt_br", "en"]
        .into_iter()
        .find(|l| install.gradex_dir(l).join("Snapshot").is_dir());
    let Some(lang) = lang else {
        return Ok(did::Catalog::default());
    };
    let snap = install.gradex_dir(lang).join("Snapshot");
    let conv_dir = install.gradex_dir(lang).join("Converters");
    let conv_dir_en = install.gradex_dir("en").join("Converters");

    let module_set: HashSet<&str> = modules.iter().map(|m| m.acronym.as_str()).collect();

    // Pontuação para desempate: mais qualificado e com motor do veículo ganha.
    let score = |e: &did::DidEntry| -> u8 {
        let mut s = (e.qual as u8) * 4;
        if spec.engine.is_some() && e.vtype.as_deref() == spec.engine.as_deref() {
            s += 2;
        }
        if spec.engine_sub.is_some() && e.subtype.as_deref() == spec.engine_sub.as_deref() {
            s += 1;
        }
        s
    };

    let files = [
        ("Fully Qualified DID Formatting.xml", did::Qual::Fully, false),
        ("Fully Qualified Bit DID Formatting.xml", did::Qual::Fully, true),
        ("Module Qualified DID Formatting.xml", did::Qual::Module, false),
        ("Module Qualified Bit DID Formatting.xml", did::Qual::Module, true),
    ];

    let mut best: HashMap<(String, u16), did::DidEntry> = HashMap::new();
    for (fname, qual, bit) in files {
        let Ok(xml) = std::fs::read_to_string(snap.join(fname)) else {
            continue;
        };
        for e in did::parse_did_formatting(&xml, qual, bit)? {
            let Some(module) = e.module.clone() else { continue };
            if !module_set.contains(module.as_str()) || e.params.is_empty() {
                continue;
            }
            if qual == did::Qual::Fully {
                // Exige o modelo do pacote; ano ausente ou igual.
                if e.model.as_deref() != Some(spec.model.as_str()) {
                    continue;
                }
                if let Some(y) = &e.year {
                    if y != &spec.model_year {
                        continue;
                    }
                }
            }
            let key = (module, e.did);
            let keep = best.get(&key).map(|p| score(&e) > score(p)).unwrap_or(true);
            if keep {
                best.insert(key, e);
            }
        }
    }

    // mdl.xml liga cada DID aos módulos que o leem (e traz nome/faixa de bits).
    let mdl_signals = std::fs::read_to_string(install.mdl_path())
        .ok()
        .map(|xml| did::parse_mdl(&xml))
        .unwrap_or_default();
    let mut did_modules: HashMap<u16, HashSet<String>> = HashMap::new();
    let mut did_signals: HashMap<u16, Vec<&did::MdlSignal>> = HashMap::new();
    for s in &mdl_signals {
        did_modules
            .entry(s.did)
            .or_default()
            .extend(s.modules.iter().cloned());
        did_signals.entry(s.did).or_default().push(s);
    }

    // (a) DIDs "Unqualified" do formatting (sem módulo): ligá-los aos módulos do
    // veículo pelo mdl. Resolve, p.ex., as alturas da suspensão (0x3B03-06 no
    // RLM) com a escala do formatting. Module/Fully têm precedência.
    if !did_modules.is_empty() {
        let unq = [
            ("Unqualified DID Formatting.xml", false),
            ("Unqualified Bit DID Formatting.xml", true),
        ];
        for (fname, bit) in unq {
            let Ok(xml) = std::fs::read_to_string(snap.join(fname)) else {
                continue;
            };
            for e in did::parse_did_formatting(&xml, did::Qual::Unqualified, bit)? {
                if e.params.is_empty() {
                    continue;
                }
                let Some(mods) = did_modules.get(&e.did) else {
                    continue;
                };
                for m in mods {
                    if !module_set.contains(m.as_str()) {
                        continue;
                    }
                    best.entry((m.clone(), e.did)).or_insert_with(|| {
                        let mut bound = e.clone();
                        bound.module = Some(m.clone());
                        bound
                    });
                }
            }
        }
    }

    // (b) Complemento: sinais do mdl que o formatting não cobre entram como
    // valor cru (nome + faixa de bits), por módulo do veículo. Traz, p.ex., a
    // tensão do compressor e as correntes das válvulas da suspensão (RLM).
    for (did_num, sigs) in &did_signals {
        let mods: HashSet<&str> = sigs
            .iter()
            .flat_map(|s| s.modules.iter().map(String::as_str))
            .filter(|m| module_set.contains(m))
            .collect();
        for m in mods {
            let key = (m.to_string(), *did_num);
            if best.contains_key(&key) {
                continue; // formatting (com escala) tem precedência
            }
            let params: Vec<did::DidParam> = sigs
                .iter()
                .filter(|s| s.modules.iter().any(|x| x == m))
                .map(|s| did::DidParam {
                    name: s.name.clone(),
                    byte_lsb: s.lsb,
                    byte_msb: s.msb,
                    mask: 0,
                    shift: 0,
                    bit_level: true,
                    converter: None,
                    source: did::ParamSource::Mdl,
                })
                .collect();
            if params.is_empty() {
                continue;
            }
            best.insert(
                key,
                did::DidEntry {
                    did: *did_num,
                    module: Some(m.to_string()),
                    model: None,
                    year: None,
                    vtype: None,
                    subtype: None,
                    data_size: None,
                    qual: did::Qual::Unqualified,
                    params,
                },
            );
        }
    }

    let entries: Vec<did::DidEntry> = best.into_values().collect();

    // Conversores referenciados (pt_br preferido, en de reserva por arquivo).
    let mut conv_ids: HashSet<String> = HashSet::new();
    for e in &entries {
        for p in &e.params {
            if let Some(c) = &p.converter {
                conv_ids.insert(c.clone());
            }
        }
    }
    let mut converters = HashMap::new();
    for id in conv_ids {
        let fname = format!("{id}.xml");
        let xml = std::fs::read_to_string(conv_dir.join(&fname))
            .or_else(|_| std::fs::read_to_string(conv_dir_en.join(&fname)));
        if let Ok(xml) = xml {
            converters.insert(id.clone(), did::parse_converter(&xml, &id));
        }
    }

    Ok(did::Catalog {
        entries,
        converters,
    })
}

/// Carrega o help de DTC do idioma disponível (pt_br > en), indexado por
/// módulo+código, só para os módulos do veículo a que cada código se aplica
/// (modelo+ano de `spec`). Pasta ausente -> vazio (os DTCs ficam sem help).
fn load_dtc_help(
    install: &SddInstall,
    spec: &ImportSpec,
    modules: &[model::Module],
) -> Result<Vec<dtc_help::DtcHelp>, DataError> {
    let lang = ["pt_br", "en"]
        .into_iter()
        .find(|l| install.rds_dtc_help_dir(l).is_dir());
    let Some(lang) = lang else {
        return Ok(Vec::new());
    };
    let module_set: std::collections::HashSet<String> =
        modules.iter().map(|m| m.acronym.clone()).collect();
    let dir = install.rds_dtc_help_dir(lang);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| DataError::Io(dir.clone(), e))? {
        let entry = entry.map_err(|e| DataError::Io(dir.clone(), e))?;
        let path = entry.path();
        let is_help = path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.starts_with("rdsDtcHelp") && n.ends_with(".xml"));
        if !is_help {
            continue;
        }
        let Ok(xml) = std::fs::read_to_string(&path) else {
            continue;
        };
        out.extend(dtc_help::parse(&xml, &spec.model, &spec.model_year, &module_set)?);
    }
    Ok(out)
}

/// Contagens de uma importação, para log e verificação.
#[derive(Debug, Clone, Copy)]
pub struct ImportSummary {
    pub modules: usize,
    pub networks: usize,
    pub ccf_blocks: usize,
    pub ccf_params: usize,
    pub ccf_options: usize,
    pub dtc_texts: usize,
    pub dtc_help: usize,
    pub did_params: usize,
}
