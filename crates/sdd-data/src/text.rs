//! Resolvedor de textos por mnemônico.
//!
//! Os arquivos em `IDS\Xml\text\@<C>\@<MNEMONICO>.xml` guardam o texto de
//! cada mnemônico (`tm id`) em vários idiomas:
//!
//! ```xml
//! <tm id="@APPROACH_LAMPS">
//!   <tu xmlns:lang="eng">Approach lamps</tu>
//!   <tu xmlns:lang="ptb">Luzes de acesso</tu>
//! </tm>
//! ```
//!
//! O atributo `xmlns:lang` é usado como marcador de idioma. Códigos de
//! interesse: `ptb` (português do Brasil), `ptg` (Portugal), `eng` (inglês).

use std::collections::HashMap;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::Reader;

/// Resolve `tm id` em texto, com cache, a partir da pasta `text` do SDD.
pub struct TextResolver {
    root: PathBuf,
    /// Ordem de preferência de idiomas (ex.: `["ptb", "ptg", "eng"]`).
    langs: Vec<String>,
    cache: HashMap<String, Option<String>>,
}

impl TextResolver {
    /// Cria um resolvedor. `text_root` é `IDS\Xml\text`; `langs` é a ordem de
    /// fallback de idiomas.
    pub fn new(text_root: impl Into<PathBuf>, langs: Vec<String>) -> Self {
        Self {
            root: text_root.into(),
            langs,
            cache: HashMap::new(),
        }
    }

    /// Resolve um id (`@APPROACH_LAMPS`) no primeiro idioma disponível da
    /// ordem de preferência. Retorna `None` se o arquivo não existir ou não
    /// tiver nenhum dos idiomas.
    pub fn resolve(&mut self, tm_id: &str) -> Option<String> {
        if let Some(hit) = self.cache.get(tm_id) {
            return hit.clone();
        }
        let resolved = self.load(tm_id);
        self.cache.insert(tm_id.to_string(), resolved.clone());
        resolved
    }

    fn path_for(&self, tm_id: &str) -> Option<PathBuf> {
        // tm_id vem como "@APPROACH_LAMPS"; o diretório é "@<primeira letra>".
        let name = tm_id.strip_prefix('@')?;
        let first = name.chars().next()?.to_ascii_uppercase();
        Some(
            self.root
                .join(format!("@{first}"))
                .join(format!("{tm_id}.xml")),
        )
    }

    fn load(&self, tm_id: &str) -> Option<String> {
        let path = self.path_for(tm_id)?;
        if !path.exists() {
            return None;
        }
        let by_lang = parse_tm_file(&path).ok()?;
        for lang in &self.langs {
            if let Some(txt) = by_lang.get(lang) {
                if !txt.is_empty() {
                    return Some(txt.clone());
                }
            }
        }
        None
    }
}

/// Lê um arquivo de mnemônico e devolve o mapa idioma→texto.
fn parse_tm_file(path: &Path) -> std::io::Result<HashMap<String, String>> {
    let file = std::fs::File::open(path)?;
    let mut reader = Reader::from_reader(BufReader::new(file));
    let config = reader.config_mut();
    config.trim_text(true);

    let mut out = HashMap::new();
    let mut buf = Vec::new();
    let mut cur_lang: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"tu" => {
                cur_lang = lang_attr(&e);
            }
            Ok(Event::Text(t)) => {
                if let Some(lang) = &cur_lang {
                    if let Ok(txt) = t.decode() {
                        out.entry(lang.clone()).or_insert_with(|| txt.to_string());
                    }
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"tu" => {
                cur_lang = None;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

/// Extrai o código de idioma do atributo `xmlns:lang` de um `<tu>`.
fn lang_attr(e: &quick_xml::events::BytesStart) -> Option<String> {
    for attr in e.attributes().flatten() {
        let key = attr.key.as_ref();
        if key == b"xmlns:lang" || key == b"lang" {
            return Some(String::from_utf8_lossy(&attr.value).to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_mnemonic_real_em_portugues() {
        let root = PathBuf::from(r"C:\Program Files (x86)\JLR\IDS\Xml\text");
        if !root.is_dir() {
            eprintln!("SDD não instalado; teste ignorado");
            return;
        }
        let mut r = TextResolver::new(root, vec!["ptb".into(), "ptg".into(), "eng".into()]);
        let txt = r.resolve("@APPROACH_LAMPS");
        assert_eq!(txt.as_deref(), Some("Luzes de acesso"));
        // Cache: segunda chamada devolve o mesmo.
        assert_eq!(
            r.resolve("@APPROACH_LAMPS").as_deref(),
            Some("Luzes de acesso")
        );
        // Inexistente devolve None sem panicar.
        assert_eq!(r.resolve("@NAO_EXISTE_XYZ_123"), None);
    }
}
