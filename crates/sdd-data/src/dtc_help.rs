//! Parser do help de diagnóstico por DTC (`<lang>/rds-dtc-help/rdsDtcHelp<CODE>.xml`),
//! indexado por **módulo + código** (um mesmo código significa coisas diferentes
//! em módulos diferentes).
//!
//! Estrutura do arquivo:
//! - `<dtcCode>` o código (ex.: `C1A13`).
//! - `<dtcDescriptionSelection>` com `<dtcQualifier module=.. model=.. year=..>`:
//!   a quais módulos/modelos/anos o código se aplica.
//! - `<helpScreen name="DTC_C1A13_RLM_L319_012">` com `<helpScreenItem
//!   mnemonicName=..>`: o conteúdo (os itens que têm texto). O nome traz o
//!   módulo (`DTC_<CODE>_<MODULE>_...`, com prefixo opcional `FLT_TYPE_<n>_`).
//! - `<helpScreenSelection helpScreenName=..>` com `<helpScreenQualifier
//!   model=.. year=..>`: a que modelo/ano cada tela se aplica.
//! - `<mnemonic name=..>texto</mnemonic>`: o dicionário de textos (pt_br quando
//!   o idioma escolhido tem tradução).
//!
//! Os itens são segmentados pelos marcadores `J_I_SUPP_DESC` / `J_I_SYMPTOMS` /
//! `J_I_POSSIBLE_CAUSES` / `J_I_ACTIONS_REQUIRED`.

use std::collections::{HashMap, HashSet};

use quick_xml::events::Event;
use quick_xml::Reader;

/// Help estruturado de um DTC em um módulo específico.
#[derive(Debug, Clone, Default)]
pub struct DtcHelp {
    /// Código legível (ex.: `C1A13`).
    pub code: String,
    /// Módulo a que este help se aplica (ex.: `RLM`).
    pub module: String,
    /// Descrição complementar.
    pub supp_desc: Vec<String>,
    /// Sintomas percebidos.
    pub symptoms: Vec<String>,
    /// Causas prováveis.
    pub causes: Vec<String>,
    /// Ações recomendadas.
    pub actions: Vec<String>,
}

impl DtcHelp {
    /// Verdadeiro quando nenhuma seção tem conteúdo.
    pub fn is_empty(&self) -> bool {
        self.supp_desc.is_empty()
            && self.symptoms.is_empty()
            && self.causes.is_empty()
            && self.actions.is_empty()
    }

    /// Itera as seções não vazias como (nome, itens).
    pub fn sections(&self) -> Vec<(&'static str, &[String])> {
        let mut v = Vec::new();
        if !self.supp_desc.is_empty() {
            v.push(("supp_desc", self.supp_desc.as_slice()));
        }
        if !self.symptoms.is_empty() {
            v.push(("symptoms", self.symptoms.as_slice()));
        }
        if !self.causes.is_empty() {
            v.push(("causes", self.causes.as_slice()));
        }
        if !self.actions.is_empty() {
            v.push(("actions", self.actions.as_slice()));
        }
        v
    }
}

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key.as_bytes())
        .and_then(|a| String::from_utf8(a.value.into_owned()).ok())
}

/// Extrai o módulo do nome de uma tela: `[FLT_TYPE_<n>_]DTC_<CODE>_<MODULE>_...`.
fn module_from_screen(name: &str) -> Option<&str> {
    let n = match name.strip_prefix("FLT_TYPE_") {
        Some(rest) => rest.split_once('_').map(|(_, r)| r).unwrap_or(rest),
        None => name,
    };
    let mut parts = n.split('_');
    if parts.next()? != "DTC" {
        return None;
    }
    parts.next()?; // código
    parts.next() // módulo
}

/// Conteúdo bruto extraído de um arquivo de help, antes de escolher por módulo.
struct Raw {
    code: String,
    dict: HashMap<String, String>,
    /// Telas com itens: (nome, mnemônicos dos itens em ordem).
    screens: Vec<(String, Vec<String>)>,
    /// Nome da tela -> (modelo, ano) a que se aplica.
    sel_quals: HashMap<String, Vec<(String, String)>>,
    /// (módulo, modelo) a que o código se aplica.
    applies: HashSet<(String, String)>,
}

fn parse_raw(xml: &str) -> Result<Raw, crate::DataError> {
    let mut reader = Reader::from_str(xml);
    let mut r = Raw {
        code: String::new(),
        dict: HashMap::new(),
        screens: Vec::new(),
        sel_quals: HashMap::new(),
        applies: HashSet::new(),
    };

    let mut cur_mnemonic: Option<String> = None;
    let mut in_code = false;
    let mut buf = String::new();
    let mut in_screen: Option<(String, Vec<String>)> = None;
    let mut in_sel: Option<(String, Vec<(String, String)>)> = None;

    loop {
        match reader.read_event() {
            Err(e) => {
                return Err(crate::DataError::Other(format!(
                    "help de DTC inválido na posição {}: {e}",
                    reader.buffer_position()
                )))
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"mnemonic" => {
                    cur_mnemonic = attr(&e, "name");
                    buf.clear();
                }
                b"dtcCode" => {
                    in_code = true;
                    buf.clear();
                }
                b"helpScreen" => {
                    let name = attr(&e, "name").or_else(|| attr(&e, "helpScreenName"));
                    in_screen = name.map(|n| (n, Vec::new()));
                }
                b"helpScreenItem" => {
                    if let (Some((_, items)), Some(mn)) =
                        (in_screen.as_mut(), attr(&e, "mnemonicName"))
                    {
                        items.push(mn);
                    }
                }
                b"helpScreenSelection" => {
                    in_sel = attr(&e, "helpScreenName").map(|n| (n, Vec::new()));
                }
                b"helpScreenQualifier" => {
                    if let Some((_, quals)) = in_sel.as_mut() {
                        quals.push((
                            attr(&e, "model").unwrap_or_default(),
                            attr(&e, "year").unwrap_or_default(),
                        ));
                    }
                }
                b"dtcQualifier" => {
                    if let (Some(m), Some(md)) = (attr(&e, "module"), attr(&e, "model")) {
                        r.applies.insert((m, md));
                    }
                }
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if cur_mnemonic.is_some() || in_code {
                    let raw = String::from_utf8_lossy(&t.into_inner()).into_owned();
                    buf.push_str(
                        &quick_xml::escape::unescape(&raw)
                            .map(|c| c.into_owned())
                            .unwrap_or_else(|_| raw.clone()),
                    );
                }
            }
            Ok(Event::CData(t)) => {
                if cur_mnemonic.is_some() || in_code {
                    buf.push_str(&String::from_utf8_lossy(&t.into_inner()));
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"mnemonic" => {
                    if let Some(name) = cur_mnemonic.take() {
                        let txt = buf.trim();
                        if !txt.is_empty() {
                            r.dict.entry(name).or_insert_with(|| txt.to_string());
                        }
                    }
                }
                b"dtcCode" if in_code => {
                    r.code = buf.trim().to_string();
                    in_code = false;
                }
                b"helpScreen" => {
                    if let Some((name, items)) = in_screen.take() {
                        if !items.is_empty() {
                            r.screens.push((name, items));
                        }
                    }
                }
                b"helpScreenSelection" => {
                    if let Some((name, quals)) = in_sel.take() {
                        r.sel_quals.entry(name).or_default().extend(quals);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(r)
}

/// Faz o parse de um arquivo de help e devolve um [`DtcHelp`] por módulo do
/// veículo a que o código se aplica (modelo `model`, ano `year`).
///
/// Para cada módulo escolhe as telas na ordem: ano exato > mesmo modelo
/// (qualquer ano) > todas. Retorna vazio quando o código não se aplica a
/// nenhum módulo do veículo.
pub fn parse(
    xml: &str,
    model: &str,
    year: &str,
    modules: &HashSet<String>,
) -> Result<Vec<DtcHelp>, crate::DataError> {
    let r = parse_raw(xml)?;

    // Agrupa as telas de conteúdo por módulo (só módulos do veículo a que o
    // código se aplica neste modelo).
    type Screen<'a> = (&'a [(String, String)], &'a [String]);
    let mut by_module: HashMap<&str, Vec<Screen>> = HashMap::new();
    for (name, items) in &r.screens {
        let Some(module) = module_from_screen(name) else {
            continue;
        };
        if !modules.contains(module) {
            continue;
        }
        if !r.applies.contains(&(module.to_string(), model.to_string())) {
            continue;
        }
        let quals = r.sel_quals.get(name).map(|q| q.as_slice()).unwrap_or(&[]);
        by_module.entry(module).or_default().push((quals, items));
    }

    let mut out = Vec::new();
    for (module, screens) in by_module {
        // Seleção em cascata por ano.
        let exact: Vec<&Screen> = screens
            .iter()
            .filter(|(q, _)| q.iter().any(|(m, y)| m == model && y == year))
            .collect();
        let same_model: Vec<&Screen> = screens
            .iter()
            .filter(|(q, _)| q.iter().any(|(m, _)| m == model))
            .collect();
        let chosen: Vec<&Screen> = if !exact.is_empty() {
            exact
        } else if !same_model.is_empty() {
            same_model
        } else {
            screens.iter().collect()
        };

        let ordered: Vec<&String> = chosen.iter().flat_map(|(_, items)| items.iter()).collect();
        let help = segment(module, &r.code, &ordered, &r.dict);
        if !help.is_empty() {
            out.push(help);
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy)]
enum Section {
    SuppDesc = 0,
    Symptoms = 1,
    Causes = 2,
    Actions = 3,
}

fn section_marker(name: &str) -> Option<Section> {
    match name {
        "J_I_SUPP_DESC" => Some(Section::SuppDesc),
        "J_I_SYMPTOMS" => Some(Section::Symptoms),
        "J_I_POSSIBLE_CAUSES" => Some(Section::Causes),
        "J_I_ACTIONS_REQUIRED" => Some(Section::Actions),
        _ => None,
    }
}

/// Segmenta uma lista ordenada de mnemônicos nas quatro seções, deduplicando.
fn segment(
    module: &str,
    code: &str,
    order: &[&String],
    dict: &HashMap<String, String>,
) -> DtcHelp {
    let mut help = DtcHelp {
        code: code.to_string(),
        module: module.to_string(),
        ..Default::default()
    };
    let mut cur: Option<Section> = None;
    let mut seen: HashSet<(u8, String)> = HashSet::new();
    for name in order {
        if let Some(sec) = section_marker(name) {
            cur = Some(sec);
            continue;
        }
        let Some(sec) = cur else { continue };
        let Some(txt) = dict.get(name.as_str()) else {
            continue;
        };
        let txt = txt.trim();
        if txt.is_empty() || !seen.insert((sec as u8, txt.to_string())) {
            continue;
        }
        match sec {
            Section::SuppDesc => help.supp_desc.push(txt.to_string()),
            Section::Symptoms => help.symptoms.push(txt.to_string()),
            Section::Causes => help.causes.push(txt.to_string()),
            Section::Actions => help.actions.push(txt.to_string()),
        }
    }
    help
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(list: &[&str]) -> HashSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    const SAMPLE: &str = r#"<dtcHelp><dtcCode>C1A13</dtcCode>
      <dtcDescriptionList><dtcDescriptionSelection>
        <dtcQualifier module="RLM" model="L319" year="MY12"/>
        <dtcQualifier module="RLM" model="L319" year="MY10"/>
        <dtcQualifier module="CHCM" model="L405" year="MY14"/>
      </dtcDescriptionSelection></dtcDescriptionList>
      <helpScreenDataName>
        <helpScreenSelection helpScreenName="DTC_C1A13_RLM_L319_012">
          <helpScreenQualifier model="L319" year="MY12"/>
        </helpScreenSelection>
        <helpScreenSelection helpScreenName="DTC_C1A13_RLM_L319_100">
          <helpScreenQualifier model="L319" year="MY10"/>
        </helpScreenSelection>
        <helpScreenSelection helpScreenName="DTC_C1A13_CHCM_014">
          <helpScreenQualifier model="L405" year="MY14"/>
        </helpScreenSelection>
      </helpScreenDataName>
      <helpScreenMnemonicList>
        <mnemonic name="J_I_POSSIBLE_CAUSES"><![CDATA[Causas]]></mnemonic>
        <mnemonic name="H_CAUSE_A"><![CDATA[Sensor de pressão (MY12)]]></mnemonic>
        <mnemonic name="H_CAUSE_B"><![CDATA[Válvula travada (MY10)]]></mnemonic>
        <mnemonic name="H_CAUSE_C"><![CDATA[Compressor CHCM]]></mnemonic>
        <mnemonic name="J_I_ACTIONS_REQUIRED"><![CDATA[Ações]]></mnemonic>
        <mnemonic name="H_ACT_A"><![CDATA[Verifique o sensor]]></mnemonic>
      </helpScreenMnemonicList>
      <helpScreenList>
        <helpScreen name="DTC_C1A13_RLM_L319_012">
          <helpScreenItem mnemonicName="J_I_POSSIBLE_CAUSES"/>
          <helpScreenItem mnemonicName="H_CAUSE_A"/>
          <helpScreenItem mnemonicName="J_I_ACTIONS_REQUIRED"/>
          <helpScreenItem mnemonicName="H_ACT_A"/>
        </helpScreen>
        <helpScreen name="DTC_C1A13_RLM_L319_100">
          <helpScreenItem mnemonicName="J_I_POSSIBLE_CAUSES"/>
          <helpScreenItem mnemonicName="H_CAUSE_B"/>
        </helpScreen>
        <helpScreen name="DTC_C1A13_CHCM_014">
          <helpScreenItem mnemonicName="J_I_POSSIBLE_CAUSES"/>
          <helpScreenItem mnemonicName="H_CAUSE_C"/>
        </helpScreen>
      </helpScreenList></dtcHelp>"#;

    #[test]
    fn extrai_modulo_do_nome() {
        assert_eq!(module_from_screen("DTC_C1A13_RLM_L319_012"), Some("RLM"));
        assert_eq!(
            module_from_screen("FLT_TYPE_64_DTC_C1A13_RLM_HLP_L319_100"),
            Some("RLM")
        );
        assert_eq!(module_from_screen("DTC_C1A13_CHCM_HLP_013"), Some("CHCM"));
        assert_eq!(module_from_screen("lixo"), None);
    }

    #[test]
    fn indexa_por_modulo_e_ano() {
        // L319/MY12, veículo com RLM: usa a tela do ano exato (MY12), não a MY10.
        let v = parse(SAMPLE, "L319", "MY12", &mods(&["RLM", "PCM"])).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].module, "RLM");
        assert_eq!(v[0].causes, ["Sensor de pressão (MY12)"]);
        assert_eq!(v[0].actions, ["Verifique o sensor"]);
    }

    #[test]
    fn cascata_de_ano() {
        // L319/MY10: não há MY10 exato na tela _012; usa a _100 (MY10).
        let v = parse(SAMPLE, "L319", "MY10", &mods(&["RLM"])).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].causes, ["Válvula travada (MY10)"]);
    }

    #[test]
    fn nao_emite_modulo_de_outra_plataforma() {
        // CHCM não é módulo do L319: não deve sair help de CHCM.
        let v = parse(SAMPLE, "L319", "MY12", &mods(&["RLM", "CHCM"])).unwrap();
        // CHCM não se aplica ao L319 (applies tem (CHCM,L405), não (CHCM,L319)).
        assert!(v.iter().all(|h| h.module != "CHCM"));
    }

    #[test]
    fn codigo_fora_do_veiculo() {
        // Veículo sem RLM: nada.
        let v = parse(SAMPLE, "L319", "MY12", &mods(&["PCM"])).unwrap();
        assert!(v.is_empty());
    }
}
