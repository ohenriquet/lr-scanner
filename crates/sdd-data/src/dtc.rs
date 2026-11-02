//! Parser do `gdx.xml` (descrições de DTC por número).
//!
//! O `IDS\Xml\gdx.xml` mapeia cada DTC (número de 16 bits) para a sua
//! descrição em inglês. O número é `byte0 << 8 | byte1`, com os 2 bits mais
//! altos do byte0 indicando a letra (00=P, 01=C, 10=B, 11=U). Ex.: `0x5A13`
//! = `C1A13`, `0x0671` = `P0671`.

use serde::Deserialize;

/// Uma descrição de DTC.
#[derive(Debug, Clone)]
pub struct DtcText {
    /// Número de 16 bits (byte0<<8 | byte1).
    pub number: u16,
    /// Código legível (ex.: `C1A13`).
    pub code: String,
    /// Descrição (inglês).
    pub description: String,
}

#[derive(Debug, Deserialize)]
struct RawGdx {
    #[serde(rename = "DIAGNOSTIC_TROUBLE_CODES")]
    dtcs: Option<RawList>,
}

#[derive(Debug, Deserialize)]
struct RawList {
    #[serde(rename = "DTC", default)]
    items: Vec<RawDtc>,
}

#[derive(Debug, Deserialize)]
struct RawDtc {
    #[serde(rename = "NUMBER")]
    number: Option<String>,
    #[serde(rename = "DESCRIPTION")]
    description: Option<String>,
}

/// Faz o parse do conteúdo de um `gdx.xml`.
pub fn parse(xml: &str) -> Result<Vec<DtcText>, crate::DataError> {
    let raw: RawGdx = quick_xml::de::from_str(xml)?;
    let mut out = Vec::new();
    if let Some(list) = raw.dtcs {
        for d in list.items {
            let Some(num_s) = d.number else { continue };
            let Some(number) = crate::model::parse_num(&num_s) else {
                continue;
            };
            let number = number as u16;
            out.push(DtcText {
                number,
                code: code_from_number(number),
                description: d.description.unwrap_or_default().trim().to_string(),
            });
        }
    }
    Ok(out)
}

/// Converte o número de 16 bits no código legível (ex.: 0x5A13 -> C1A13).
pub fn code_from_number(n: u16) -> String {
    let b0 = (n >> 8) as u8;
    let b1 = (n & 0xff) as u8;
    let letter = match b0 >> 6 {
        0 => 'P',
        1 => 'C',
        2 => 'B',
        _ => 'U',
    };
    let d1 = (b0 >> 4) & 0x3;
    let d2 = b0 & 0x0f;
    format!("{letter}{d1}{d2:X}{b1:02X}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_conhecidos() {
        assert_eq!(code_from_number(0x5A13), "C1A13");
        assert_eq!(code_from_number(0x0671), "P0671");
        assert_eq!(code_from_number(0x0000), "P0000");
    }

    #[test]
    fn parse_minimo() {
        let xml = r#"<GDX><DIAGNOSTIC_TROUBLE_CODES>
            <DTC><NUMBER>0x0671</NUMBER><DESCRIPTION><![CDATA[Cylinder 1 Glow Plug Circuit / Open]]></DESCRIPTION></DTC>
            <DTC><NUMBER>0x5A13</NUMBER><DESCRIPTION><![CDATA[Pressure Does Not Decrease When Venting Gallery]]></DESCRIPTION></DTC>
            </DIAGNOSTIC_TROUBLE_CODES></GDX>"#;
        let v = parse(xml).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].code, "P0671");
        assert!(v[0].description.contains("Glow Plug"));
        assert_eq!(v[1].code, "C1A13");
    }
}
