//! Parser dos "live data" do SDD (DIDs de leitura por módulo).
//!
//! O SDD descreve cada parâmetro ao vivo em dois lugares:
//!
//! 1. **DID Formatting** (`Common/SDD/Data/<lang>/gradex/Snapshot/`): seis
//!    arquivos (Fully/Module/Unqualified × normal/Bit). Cada `<KeyedData
//!    id="DID 0x1945 AWDCM">` descreve um DID UDS 0x22 de um módulo, com um
//!    teste de habilitação (módulo, e p/ os "Fully" também modelo/ano/motor) e
//!    uma lista de `<ReadParameter>` — cada um com nome, faixa de bytes
//!    (`lsbNumber`/`msbNumber`), máscara/`shift` e um atalho para um conversor.
//! 2. **Converters** (`.../gradex/Converters/<id>.xml`): a escala. Um
//!    `<LinearConverter multiplier offset offsetFirst>` com `<outType
//!    unit=.. quantity=..>` (valor numérico) e/ou vários `<quantityState>`
//!    (enum: faixas de valor -> rótulo).
//!
//! Aqui só se faz o parse; o filtro por veículo e a gravação no pacote ficam
//! em [`crate::db`] / [`crate::import_vehicle`].

use quick_xml::events::Event;
use quick_xml::Reader;

/// Nível de qualificação de um DID (define a precedência na hora de escolher).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qual {
    /// Sem teste de módulo — vale como fallback geral.
    Unqualified = 0,
    /// Vale para um módulo específico.
    Module = 1,
    /// Vale para módulo + modelo + ano + motor específicos.
    Fully = 2,
}

/// Origem de um parâmetro: do DID Formatting (com escala) ou do mdl.xml (cru).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParamSource {
    /// DID Formatting: `byte_lsb`/`byte_msb` são índices de byte + `mask`/`shift`.
    #[default]
    Formatting,
    /// mdl.xml: `byte_lsb`/`byte_msb` são índices de BIT; valor cru (sem conversor).
    Mdl,
}

/// Um parâmetro de leitura dentro de um DID.
#[derive(Debug, Clone, Default)]
pub struct DidParam {
    pub name: String,
    /// Índice do byte (Formatting) ou do bit (Mdl) menos significativo.
    pub byte_lsb: i32,
    /// Índice do byte (Formatting) ou do bit (Mdl) mais significativo.
    pub byte_msb: i32,
    /// Máscara aplicada (0 = sem máscara).
    pub mask: u32,
    /// Deslocamento de bits (parâmetros a nível de bit).
    pub shift: u8,
    /// Verdadeiro para os arquivos "Bit" (parâmetro empacotado em bits).
    pub bit_level: bool,
    /// Id do conversor de escala (`CVT_*` / `CNV_*`), se houver.
    pub converter: Option<String>,
    /// De onde veio (define como interpretar `byte_lsb`/`byte_msb`).
    pub source: ParamSource,
}

/// Um DID de leitura de um módulo.
#[derive(Debug, Clone)]
pub struct DidEntry {
    /// Identificador UDS 0x22 (16 bits).
    pub did: u16,
    pub module: Option<String>,
    pub model: Option<String>,
    /// Ano-modelo (ex.: `MY10`).
    pub year: Option<String>,
    /// Tipo de motor (ex.: `V6DIESEL`).
    pub vtype: Option<String>,
    /// Subtipo de motor (ex.: `3L`).
    pub subtype: Option<String>,
    pub data_size: Option<u32>,
    pub qual: Qual,
    pub params: Vec<DidParam>,
}

/// Tipo de um conversor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConverterKind {
    /// `raw * multiplier (+ offset)` -> valor com unidade.
    Linear,
    /// Faixas de valor mapeadas para rótulos (enum/estado).
    Enum,
}

/// Catálogo de live data de um veículo: DIDs (já filtrados por módulo/modelo)
/// e os conversores que eles referenciam.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub entries: Vec<DidEntry>,
    pub converters: std::collections::HashMap<String, Converter>,
}

/// Um conversor de escala resolvido.
#[derive(Debug, Clone)]
pub struct Converter {
    pub id: String,
    pub kind: ConverterKind,
    pub multiplier: f64,
    pub offset: f64,
    /// Se verdadeiro: `(raw + offset) * multiplier`; senão `raw * multiplier + offset`.
    pub offset_first: bool,
    pub unit: Option<String>,
    pub quantity: Option<String>,
    /// Estados do enum: (valor baixo, valor alto, rótulo).
    pub states: Vec<(i64, i64, String)>,
}

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key.as_bytes())
        .and_then(|a| String::from_utf8(a.value.into_owned()).ok())
}

/// Extrai o número do DID de um id tipo `"DID 0x1945 AWDCM"`.
fn did_from_id(id: &str) -> Option<u16> {
    let rest = id.strip_prefix("DID ")?.trim_start();
    let hex = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X"))?;
    let end = hex
        .find(|c: char| !c.is_ascii_hexdigit())
        .unwrap_or(hex.len());
    u16::from_str_radix(&hex[..hex.len().min(4).min(end)], 16).ok()
}

/// O que um trecho de texto corrente está preenchendo.
enum Capture {
    None,
    Name,
    String2,
    DataSize,
}

/// Faz o parse de um arquivo de DID Formatting.
///
/// `qual` indica o nível (Fully/Module/Unqualified) e `bit_level` se é a
/// variante "Bit".
pub fn parse_did_formatting(
    xml: &str,
    qual: Qual,
    bit_level: bool,
) -> Result<Vec<DidEntry>, crate::DataError> {
    let mut reader = Reader::from_str(xml);

    let mut out = Vec::new();
    let mut entry: Option<DidEntry> = None;
    let mut param: Option<DidParam> = None;
    let mut pending_test: Option<String> = None;
    let mut cap = Capture::None;
    let mut buf = String::new();

    loop {
        match reader.read_event() {
            Err(e) => {
                return Err(crate::DataError::Other(format!(
                    "XML de DID inválido na posição {}: {e}",
                    reader.buffer_position()
                )))
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                match e.name().as_ref() {
                    b"KeyedData" => {
                        if let Some(id) = attr(&e, "id") {
                            if let Some(did) = did_from_id(&id) {
                                entry = Some(DidEntry {
                                    did,
                                    module: None,
                                    model: None,
                                    year: None,
                                    vtype: None,
                                    subtype: None,
                                    data_size: None,
                                    qual,
                                    params: Vec::new(),
                                });
                            }
                        }
                    }
                    b"STRING-TEST-TACTIC" => {
                        pending_test = attr(&e, "id");
                    }
                    b"string2" => {
                        if pending_test.is_some() {
                            cap = Capture::String2;
                            buf.clear();
                        }
                    }
                    b"dataSize" => {
                        if entry.is_some() && param.is_none() {
                            cap = Capture::DataSize;
                            buf.clear();
                        }
                    }
                    b"ReadParameter" => {
                        let mut p = DidParam {
                            bit_level,
                            ..Default::default()
                        };
                        p.mask = attr(&e, "mask")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        p.shift = attr(&e, "shift").and_then(|s| s.parse().ok()).unwrap_or(0);
                        p.byte_lsb = attr(&e, "lsbNumber")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        p.byte_msb = attr(&e, "msbNumber")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        param = Some(p);
                    }
                    b"name" => {
                        if param.is_some() {
                            cap = Capture::Name;
                            buf.clear();
                        }
                    }
                    b"SHORTCUT" => {
                        if let Some(p) = param.as_mut() {
                            p.converter = attr(&e, "id");
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if !matches!(cap, Capture::None) {
                    let raw = String::from_utf8_lossy(&t.into_inner()).into_owned();
                    buf.push_str(
                        &quick_xml::escape::unescape(&raw)
                            .map(|c| c.into_owned())
                            .unwrap_or_else(|_| raw.clone()),
                    );
                }
            }
            Ok(Event::CData(t)) => {
                if !matches!(cap, Capture::None) {
                    buf.push_str(&String::from_utf8_lossy(&t.into_inner()));
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"string2" => {
                    if let (Some(id), Some(ent)) = (pending_test.take(), entry.as_mut()) {
                        let v = buf.trim().to_string();
                        match id.as_str() {
                            "module" => ent.module = Some(v),
                            "model" => ent.model = Some(v),
                            "year" => ent.year = Some(v),
                            "type" => ent.vtype = Some(v),
                            "subType" => ent.subtype = Some(v),
                            _ => {}
                        }
                    }
                    cap = Capture::None;
                }
                b"name" => {
                    if let Some(p) = param.as_mut() {
                        p.name = buf.trim().to_string();
                    }
                    cap = Capture::None;
                }
                b"dataSize" => {
                    if let Some(ent) = entry.as_mut() {
                        if ent.data_size.is_none() {
                            ent.data_size = buf.trim().parse().ok();
                        }
                    }
                    cap = Capture::None;
                }
                b"ReadParameter" => {
                    if let (Some(p), Some(ent)) = (param.take(), entry.as_mut()) {
                        ent.params.push(p);
                    }
                }
                b"KeyedData" => {
                    if let Some(ent) = entry.take() {
                        out.push(ent);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(out)
}

/// Um sinal de live data do `mdl.xml`: um campo de um DID (bit range + nome),
/// com os módulos que o leem. Um DID pode ter vários sinais (sub-campos).
#[derive(Debug, Clone)]
pub struct MdlSignal {
    pub did: u16,
    pub name: String,
    /// Bit menos significativo do campo dentro do dado do DID.
    pub lsb: i32,
    /// Bit mais significativo.
    pub msb: i32,
    pub modules: std::collections::HashSet<String>,
}

/// Faz o parse do `mdl.xml` (dicionário de PIDs): devolve cada sinal (campo de
/// um DID) com nome, faixa de bits e os módulos que o leem.
///
/// Serve para (a) ligar os DIDs "Unqualified" do DID Formatting aos módulos do
/// veículo e (b) complementar o catálogo com sinais que o formatting não cobre
/// (ex.: correntes das válvulas e tensão do compressor da suspensão no RLM).
/// Ex.: `<s id="3B03" lsb="0" msb="15" name="Front left corner height">...<m
/// id="RLM"/>...`.
pub fn parse_mdl(xml: &str) -> Vec<MdlSignal> {
    use std::collections::HashSet;
    let mut reader = Reader::from_str(xml);
    let mut out = Vec::new();

    let mut s_depth = 0u32;
    let mut cur: Option<MdlSignal> = None;

    loop {
        match reader.read_event() {
            Err(_) | Ok(Event::Eof) => break,
            // `<s>` com conteúdo: abre um sinal (normalmente não aninha).
            Ok(Event::Start(e)) if e.name().as_ref() == b"s" => {
                if s_depth == 0 {
                    cur = attr(&e, "id")
                        .and_then(|s| u16::from_str_radix(&s, 16).ok())
                        .map(|did| MdlSignal {
                            did,
                            name: attr(&e, "name").unwrap_or_default(),
                            lsb: attr(&e, "lsb").and_then(|s| s.parse().ok()).unwrap_or(0),
                            msb: attr(&e, "msb").and_then(|s| s.parse().ok()).unwrap_or(0),
                            modules: HashSet::new(),
                        });
                }
                s_depth += 1;
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"s" => {
                s_depth = s_depth.saturating_sub(1);
                if s_depth == 0 {
                    if let Some(sig) = cur.take() {
                        out.push(sig);
                    }
                }
            }
            // `<m id=MOD/>`: módulo que lê o sinal corrente.
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if e.name().as_ref() == b"m" && s_depth > 0 =>
            {
                if let (Some(id), Some(sig)) = (attr(&e, "id"), cur.as_mut()) {
                    sig.modules.insert(id);
                }
            }
            _ => {}
        }
    }
    out
}

/// Faz o parse de um arquivo de conversor.
pub fn parse_converter(xml: &str, id: &str) -> Converter {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut conv = Converter {
        id: id.to_string(),
        kind: ConverterKind::Linear,
        multiplier: 1.0,
        offset: 0.0,
        offset_first: true,
        unit: None,
        quantity: None,
        states: Vec::new(),
    };
    let mut cur_range: Option<(i64, i64)> = None;
    let mut in_state = false;
    let mut cap_name = false;
    let mut buf = String::new();

    loop {
        match reader.read_event() {
            Err(_) | Ok(Event::Eof) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                n if n.ends_with(b"Converter") => {
                    if let Some(m) = attr(&e, "multiplier").and_then(|s| s.parse().ok()) {
                        conv.multiplier = m;
                    }
                    if let Some(o) = attr(&e, "offset").and_then(|s| s.parse().ok()) {
                        conv.offset = o;
                    }
                    if let Some(of) = attr(&e, "offsetFirst") {
                        conv.offset_first = of == "true";
                    }
                }
                b"outType" => {
                    conv.unit = attr(&e, "unit");
                    conv.quantity = attr(&e, "quantity");
                }
                b"quantityState" => {
                    in_state = true;
                    cur_range = None;
                }
                b"quantity" => {
                    if in_state {
                        let lo = attr(&e, "lowValue").and_then(|s| s.parse::<f64>().ok());
                        let hi = attr(&e, "highValue").and_then(|s| s.parse::<f64>().ok());
                        if let (Some(lo), Some(hi)) = (lo, hi) {
                            cur_range = Some((lo as i64, hi as i64));
                        }
                    }
                }
                b"name" if in_state => {
                    cap_name = true;
                    buf.clear();
                }
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if cap_name {
                    let raw = String::from_utf8_lossy(&t.into_inner()).into_owned();
                    buf.push_str(
                        &quick_xml::escape::unescape(&raw)
                            .map(|c| c.into_owned())
                            .unwrap_or_else(|_| raw.clone()),
                    );
                }
            }
            Ok(Event::CData(t)) => {
                if cap_name {
                    buf.push_str(&String::from_utf8_lossy(&t.into_inner()));
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"name" => {
                    cap_name = false;
                }
                b"quantityState" => {
                    if let Some((lo, hi)) = cur_range.take() {
                        conv.states.push((lo, hi, buf.trim().to_string()));
                    }
                    in_state = false;
                }
                _ => {}
            },
            _ => {}
        }
    }
    if !conv.states.is_empty() {
        conv.kind = ConverterKind::Enum;
    }
    conv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrai_numero_do_did() {
        assert_eq!(did_from_id("DID 0x1945 AWDCM"), Some(0x1945));
        assert_eq!(did_from_id("DID 0x0347 PCM 1"), Some(0x0347));
        assert_eq!(did_from_id("DID 0x201C PBM"), Some(0x201C));
        assert_eq!(did_from_id("lixo"), None);
    }

    #[test]
    fn parse_modulo_qualificado() {
        let xml = r#"<COMPONENT>
        <KeyedData id="DID 0x1945 AWDCM">
          <Properties><enable>
            <STRING-TEST-TACTIC id="module"><Properties>
              <string1 xlink:href="../snapshot.xml#ModuleId"/><string2>AWDCM</string2>
            </Properties></STRING-TEST-TACTIC>
          </enable><keySize>6469</keySize><dataSize>2</dataSize></Properties>
          <ReadParameter id="1" mask="65535" lsbNumber="0" msbNumber="1">
            <Properties><name><![CDATA[Valve output]]></name></Properties>
            <SHORTCUT id="CVT_N_AMP_OFF_0_RES_0PNT001" xlink:href="../Converters/CVT_N_AMP_OFF_0_RES_0PNT001.xml#x"/>
          </ReadParameter>
        </KeyedData></COMPONENT>"#;
        let v = parse_did_formatting(xml, Qual::Module, false).unwrap();
        assert_eq!(v.len(), 1);
        let d = &v[0];
        assert_eq!(d.did, 0x1945);
        assert_eq!(d.module.as_deref(), Some("AWDCM"));
        assert_eq!(d.data_size, Some(2));
        assert_eq!(d.params.len(), 1);
        assert_eq!(d.params[0].name, "Valve output");
        assert_eq!(d.params[0].mask, 65535);
        assert_eq!(d.params[0].byte_msb, 1);
        assert_eq!(
            d.params[0].converter.as_deref(),
            Some("CVT_N_AMP_OFF_0_RES_0PNT001")
        );
    }

    #[test]
    fn parse_fully_qualificado() {
        let xml = r#"<COMPONENT>
        <KeyedData id="DID 0x0347 PCM 1"><Properties><enable><EXPRESSION><AND>
          <STRING-TEST-TACTIC id="module"><Properties><string1/><string2>PCM</string2></Properties></STRING-TEST-TACTIC>
          <STRING-TEST-TACTIC id="model"><Properties><string1><VDATA-QUALIFIER id="CM_MODEL"/></string1><string2>L319</string2></Properties></STRING-TEST-TACTIC>
          <STRING-TEST-TACTIC id="year"><Properties><string1/><string2>MY10</string2></Properties></STRING-TEST-TACTIC>
          <STRING-TEST-TACTIC id="type"><Properties><string1/><string2>V6DIESEL</string2></Properties></STRING-TEST-TACTIC>
          <STRING-TEST-TACTIC id="subType"><Properties><string1/><string2>3L</string2></Properties></STRING-TEST-TACTIC>
        </AND></EXPRESSION></enable><dataSize>2</dataSize></Properties>
        <ReadParameter id="1" mask="65535" lsbNumber="0" msbNumber="1"><Properties><name><![CDATA[Boost]]></name></Properties>
          <SHORTCUT id="CVT_N_PCT" xlink:href="x"/></ReadParameter>
        </KeyedData></COMPONENT>"#;
        let v = parse_did_formatting(xml, Qual::Fully, false).unwrap();
        assert_eq!(v.len(), 1);
        let d = &v[0];
        assert_eq!(d.module.as_deref(), Some("PCM"));
        assert_eq!(d.model.as_deref(), Some("L319"));
        assert_eq!(d.year.as_deref(), Some("MY10"));
        assert_eq!(d.vtype.as_deref(), Some("V6DIESEL"));
        assert_eq!(d.subtype.as_deref(), Some("3L"));
    }

    #[test]
    fn converter_linear() {
        let xml = r#"<COMPONENT><LinearConverter offsetFirst="true" multiplier="0.001" offset="0">
          <Properties><inType quantity="Dimensionless" unit="int"/><outType quantity="Current" unit="A"/></Properties>
        </LinearConverter></COMPONENT>"#;
        let c = parse_converter(xml, "CVT_N_AMP_OFF_0_RES_0PNT001");
        assert_eq!(c.kind, ConverterKind::Linear);
        assert_eq!(c.multiplier, 0.001);
        assert_eq!(c.unit.as_deref(), Some("A"));
        assert_eq!(c.quantity.as_deref(), Some("Current"));
    }

    #[test]
    fn mdl_liga_did_a_modulos() {
        let xml = r#"<mdl>
          <s id="3B03" lsb="0" msb="15" name="Front left corner height"><ms><m id="RLM"/><m id="CHCM"/></ms>
            <ht><qs id="1"><ms><m id="ARCM"/></ms></qs></ht></s>
          <s id="0504" name="MAF"><ms><m id="PCM"/></ms></s>
        </mdl>"#;
        let sigs = parse_mdl(xml);
        assert_eq!(sigs.len(), 2);
        let h = sigs.iter().find(|s| s.did == 0x3B03).unwrap();
        assert_eq!(h.name, "Front left corner height");
        assert_eq!((h.lsb, h.msb), (0, 15));
        assert_eq!(
            h.modules,
            ["RLM", "CHCM", "ARCM"].into_iter().map(String::from).collect()
        );
        let maf = sigs.iter().find(|s| s.did == 0x0504).unwrap();
        assert_eq!(maf.modules, ["PCM"].into_iter().map(String::from).collect());
    }

    #[test]
    fn converter_enum() {
        let xml = r#"<COMPONENT><LinearConverter offsetFirst="true" multiplier="1" offset="0">
          <Properties><outType quantity="Dimensionless" unit="int"/></Properties>
          <quantityState id="s1"><Properties><range><quantity lowValue="0" highValue="0"/></range><name><![CDATA[Off]]></name></Properties></quantityState>
          <quantityState id="s2"><Properties><range><quantity lowValue="1" highValue="1"/></range><name><![CDATA[Level 1]]></name></Properties></quantityState>
        </LinearConverter></COMPONENT>"#;
        let c = parse_converter(xml, "CNV_N_LEVEL_STATUS");
        assert_eq!(c.kind, ConverterKind::Enum);
        assert_eq!(c.states.len(), 2);
        assert_eq!(c.states[0], (0, 0, "Off".to_string()));
        assert_eq!(c.states[1], (1, 1, "Level 1".to_string()));
    }
}
