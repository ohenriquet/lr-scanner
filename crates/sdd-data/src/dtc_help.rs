//! Parser do help de diagnóstico por DTC (`<lang>/rds-dtc-help/rdsDtcHelp<CODE>.xml`).
//!
//! Cada arquivo descreve um código (ex.: `C1A13`) com, por módulo/modelo/ano,
//! uma tela de ajuda. O conteúdo são `<mnemonic name=.. >texto</mnemonic>`
//! (dicionário) referenciados por `<helpScreenItem mnemonicName=..>` em ordem
//! de documento. Marcadores de seção `J_I_*` delimitam as partes:
//!
//! - `J_I_SUPP_DESC`       -> descrição complementar
//! - `J_I_SYMPTOMS`        -> sintomas
//! - `J_I_POSSIBLE_CAUSES` -> causas prováveis
//! - `J_I_ACTIONS_REQUIRED`-> ações
//!
//! O texto vem traduzido (`pt_br`) quando o importador escolhe esse idioma.
//! Cada arquivo lista os `<dtcQualifier model=.. year=..>` a que se aplica, o
//! que permite ficar só com os códigos do veículo importado.

use std::collections::HashSet;

use quick_xml::events::Event;
use quick_xml::Reader;

/// Help estruturado de um DTC.
#[derive(Debug, Clone, Default)]
pub struct DtcHelp {
    /// Código legível (ex.: `C1A13`).
    pub code: String,
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

/// Faz o parse de um arquivo de help de DTC.
///
/// Se `model_filter` for `Some(modelo)` e o arquivo listar modelos aos quais
/// se aplica, retorna `None` quando o modelo não está entre eles (assim o
/// pacote fica só com os códigos do veículo). Retorna `None` também quando não
/// há conteúdo.
pub fn parse(xml: &str, model_filter: Option<&str>) -> Result<Option<DtcHelp>, crate::DataError> {
    let mut reader = Reader::from_str(xml);

    let mut dict: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut order: Vec<String> = Vec::new(); // mnemonicName dos helpScreenItem, em ordem
    let mut models: HashSet<String> = HashSet::new();
    let mut code = String::new();

    // captura de texto corrente
    let mut cur_mnemonic: Option<String> = None;
    let mut in_code = false;
    let mut buf = String::new();

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
                    let name = attr(&e, "name");
                    // Empty = sem texto; Start = captura até o fim.
                    cur_mnemonic = name;
                    buf.clear();
                }
                b"helpScreenItem" => {
                    if let Some(n) = attr(&e, "mnemonicName") {
                        order.push(n);
                    }
                }
                b"dtcQualifier" => {
                    if let Some(m) = attr(&e, "model") {
                        models.insert(m);
                    }
                }
                b"dtcCode" => {
                    in_code = true;
                    buf.clear();
                }
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if cur_mnemonic.is_some() || in_code {
                    let raw = String::from_utf8_lossy(&t.into_inner()).into_owned();
                    let s = quick_xml::escape::unescape(&raw)
                        .map(|c| c.into_owned())
                        .unwrap_or(raw);
                    buf.push_str(&s);
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
                            dict.entry(name).or_insert_with(|| txt.to_string());
                        }
                    }
                }
                b"dtcCode" => {
                    if in_code {
                        code = buf.trim().to_string();
                        in_code = false;
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    // Filtro por modelo: se o arquivo declara modelos e o nosso não está, pula.
    if let Some(model) = model_filter {
        if !models.is_empty() && !models.contains(model) {
            return Ok(None);
        }
    }

    // Segmenta os itens pelos marcadores de seção.
    let mut help = DtcHelp {
        code,
        ..Default::default()
    };
    let mut cur: Option<Section> = None;
    let mut seen: HashSet<(u8, String)> = HashSet::new();
    for name in &order {
        if let Some(sec) = section_marker(name) {
            cur = Some(sec);
            continue;
        }
        let Some(sec) = cur else { continue };
        let Some(txt) = dict.get(name) else { continue };
        let txt = txt.trim();
        if txt.is_empty() {
            continue;
        }
        if !seen.insert((sec as u8, txt.to_string())) {
            continue; // dedup por seção
        }
        match sec {
            Section::SuppDesc => help.supp_desc.push(txt.to_string()),
            Section::Symptoms => help.symptoms.push(txt.to_string()),
            Section::Causes => help.causes.push(txt.to_string()),
            Section::Actions => help.actions.push(txt.to_string()),
        }
    }

    if help.is_empty() {
        Ok(None)
    } else {
        Ok(Some(help))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<dtcHelp><dtcCode>C1A13</dtcCode>
      <dtcDescriptionList><dtcDescriptionSelection>
        <dtcQualifier module="RLM" model="L319" year="MY12"/>
        <dtcQualifier module="RLM" model="L320" year="MY10"/>
      </dtcDescriptionSelection></dtcDescriptionList>
      <helpScreenMnemonicList>
        <mnemonic name="J_I_SUPP_DESC"><![CDATA[Descrição complementar:]]></mnemonic>
        <mnemonic name="H_DESC_1"><![CDATA[A pressão não cai ao ventilar.]]></mnemonic>
        <mnemonic name="J_I_SYMPTOMS"><![CDATA[Sintomas:]]></mnemonic>
        <mnemonic name="H_SYMP_1"><![CDATA[Suspensão inoperante.]]></mnemonic>
        <mnemonic name="J_I_POSSIBLE_CAUSES"><![CDATA[Causas prováveis]]></mnemonic>
        <mnemonic name="H_CAUSE_1"><![CDATA[Sensor de pressão com falha]]></mnemonic>
        <mnemonic name="H_CAUSE_2"><![CDATA[Válvula de escape emperrada]]></mnemonic>
        <mnemonic name="J_I_ACTIONS_REQUIRED"><![CDATA[Ações:]]></mnemonic>
        <mnemonic name="H_ACTION_1"><![CDATA[Verifique o circuito do sensor.]]></mnemonic>
        <mnemonic name="H_ACTION_1"><![CDATA[Verifique o circuito do sensor.]]></mnemonic>
      </helpScreenMnemonicList>
      <helpScreenList><helpScreen>
        <helpScreenItem mnemonicName="J_I_SUPP_DESC"/>
        <helpScreenItem mnemonicName="H_DESC_1"/>
        <helpScreenItem mnemonicName="J_I_SYMPTOMS"/>
        <helpScreenItem mnemonicName="H_SYMP_1"/>
        <helpScreenItem mnemonicName="J_I_POSSIBLE_CAUSES"/>
        <helpScreenItem mnemonicName="H_CAUSE_1"/>
        <helpScreenItem mnemonicName="H_CAUSE_2"/>
        <helpScreenItem mnemonicName="J_I_ACTIONS_REQUIRED"/>
        <helpScreenItem mnemonicName="H_ACTION_1"/>
        <helpScreenItem mnemonicName="H_ACTION_1"/>
      </helpScreen></helpScreenList></dtcHelp>"#;

    #[test]
    fn segmenta_secoes() {
        let h = parse(SAMPLE, Some("L319")).unwrap().unwrap();
        assert_eq!(h.code, "C1A13");
        assert_eq!(h.supp_desc, ["A pressão não cai ao ventilar."]);
        assert_eq!(h.symptoms, ["Suspensão inoperante."]);
        assert_eq!(
            h.causes,
            ["Sensor de pressão com falha", "Válvula de escape emperrada"]
        );
        // dedup: a ação repetida aparece uma vez só.
        assert_eq!(h.actions, ["Verifique o circuito do sensor."]);
    }

    #[test]
    fn filtra_por_modelo() {
        // L462 não está nos qualifiers -> pula.
        assert!(parse(SAMPLE, Some("L462")).unwrap().is_none());
        // Sem filtro -> inclui.
        assert!(parse(SAMPLE, None).unwrap().is_some());
    }
}
