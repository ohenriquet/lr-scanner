//! Parser do `CCF_DATA_<VEH>_<ANO>.exml` (schema `ccf_data.xsd`).
//!
//! Extrai o mapa de bits do CCF: blocos, endereçamento por módulo/motor e
//! todos os parâmetros configuráveis com suas opções. É a base do editor de
//! configuração (welcome/approach/puddle lights etc.).

use serde::Deserialize;

use crate::model::{parse_num, parse_param_id, CcfAddress, CcfBlock, CcfOption, CcfParam};

/// Resultado do parse de um `CCF_DATA`.
#[derive(Debug, Default)]
pub struct CcfData {
    pub blocks: Vec<CcfBlock>,
    pub addresses: Vec<CcfAddress>,
    pub params: Vec<CcfParam>,
}

#[derive(Debug, Deserialize)]
struct RawRoot {
    #[serde(default)]
    block: Vec<RawBlock>,
}

#[derive(Debug, Deserialize)]
struct RawBlock {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "@name")]
    attr_name: Option<String>,
    #[serde(rename = "@serviceIdRd")]
    svc_rd: Option<String>,
    #[serde(rename = "@serviceIdWr")]
    svc_wr: Option<String>,
    name: Option<String>,
    #[serde(default)]
    address: Vec<RawAddr>,
    block_data: Option<RawBlockData>,
    #[serde(default)]
    group: Vec<RawGroup>,
}

#[derive(Debug, Deserialize)]
struct RawAddr {
    #[serde(rename = "@module")]
    module: String,
    #[serde(rename = "@t_ccf")]
    t_ccf: Option<String>,
    #[serde(rename = "@parameterId")]
    parameter_id: Option<String>,
    #[serde(rename = "@start_address")]
    start_address: Option<String>,
    #[serde(rename = "@stop_address")]
    stop_address: Option<String>,
    #[serde(rename = "@merge")]
    merge: Option<String>,
    qualifier: Option<RawQualifier>,
}

#[derive(Debug, Deserialize)]
struct RawQualifier {
    #[serde(default)]
    qual: Vec<RawQual>,
}

#[derive(Debug, Deserialize)]
struct RawQual {
    #[serde(rename = "@type")]
    qtype: String,
    #[serde(rename = "@value")]
    value: String,
}

#[derive(Debug, Deserialize)]
struct RawBlockData {
    #[serde(default)]
    data: Vec<RawData>,
}

#[derive(Debug, Deserialize)]
struct RawData {
    #[serde(rename = "@length")]
    length: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawGroup {
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(default)]
    parameter: Vec<RawParam>,
}

#[derive(Debug, Deserialize)]
struct RawParam {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@mask")]
    mask: Option<String>,
    #[serde(rename = "@type")]
    ptype: Option<String>,
    category: Option<RawCategory>,
    parameter_title: Option<RawTitle>,
    title: Option<RawTitle>,
    #[serde(default)]
    select: Vec<RawSelect>,
}

#[derive(Debug, Deserialize)]
struct RawCategory {
    #[serde(rename = "@display")]
    display: Option<String>,
    #[serde(rename = "@edit")]
    edit: Option<String>,
    #[serde(rename = "@scope")]
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTitle {
    tm: Option<RawTm>,
}

#[derive(Debug, Deserialize)]
struct RawTm {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "$text")]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSelect {
    #[serde(default)]
    option: Vec<RawOption>,
}

#[derive(Debug, Deserialize)]
struct RawOption {
    #[serde(rename = "@value")]
    value: Option<String>,
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@code")]
    code: Option<String>,
    tm: Option<RawTm>,
}

/// Faz o parse de um `CCF_DATA` já decifrado (XML em texto).
pub fn parse(xml: &str) -> Result<CcfData, crate::DataError> {
    let raw: RawRoot = quick_xml::de::from_str(xml)?;
    let mut out = CcfData::default();

    for b in raw.block {
        let length = b
            .block_data
            .as_ref()
            .and_then(|bd| bd.data.first())
            .and_then(|d| d.length.as_deref())
            .and_then(parse_num);

        out.blocks.push(CcfBlock {
            name: b.name.clone().or(b.attr_name.clone()).unwrap_or_default(),
            service_rd: b.svc_rd.as_deref().and_then(parse_num).map(|n| n as u8),
            service_wr: b.svc_wr.as_deref().and_then(parse_num).map(|n| n as u8),
            length,
            block_id: b.id.clone(),
        });

        for a in b.address {
            out.addresses.push(CcfAddress {
                block_id: b.id.clone(),
                engine_qualifier: a.qualifier.as_ref().and_then(engine_from_qual),
                module: a.module.trim().to_string(),
                t_ccf: a.t_ccf.filter(|s| !s.is_empty()),
                parameter_id: a.parameter_id.as_deref().and_then(parse_num),
                start_address: a.start_address.as_deref().and_then(parse_num),
                stop_address: a.stop_address.as_deref().and_then(parse_num),
                merge: a.merge.as_deref() == Some("true"),
            });
        }

        for g in b.group {
            let group = g.name.clone().unwrap_or_default();
            for p in g.parameter {
                if let Some(param) = map_param(&b.id, &group, p) {
                    out.params.push(param);
                }
            }
        }
    }

    Ok(out)
}

fn map_param(block_id: &str, group: &str, p: RawParam) -> Option<CcfParam> {
    let (byte_start, byte_stop, bit_start, bit_stop) = parse_param_id(&p.id)?;
    let cat = p.category.unwrap_or(RawCategory {
        display: None,
        edit: None,
        scope: None,
    });
    // O título pode estar em <parameter_title> ou <title>.
    let title = p.parameter_title.or(p.title);
    let (title_tm, title_literal) = match title.and_then(|t| t.tm) {
        Some(tm) => (tm.id, tm.text),
        None => (None, None),
    };

    let options: Vec<_> = p
        .select
        .into_iter()
        .flat_map(|s| s.option.into_iter().filter_map(map_option))
        .collect();

    Some(CcfParam {
        block_id: block_id.to_string(),
        group: group.to_string(),
        name: p.name.unwrap_or_default(),
        raw_id: p.id,
        byte_start,
        byte_stop,
        bit_start,
        bit_stop,
        mask: p.mask.as_deref().and_then(parse_num),
        ptype: p.ptype,
        display: cat.display.as_deref() != Some("false"),
        editable: cat.edit.as_deref() == Some("true"),
        scope: cat.scope,
        title_tm,
        title_literal,
        options,
    })
}

fn map_option(o: RawOption) -> Option<CcfOption> {
    let value = o.value.as_deref().and_then(parse_num)?;
    let (tm_id, tm_literal) = match o.tm {
        Some(tm) => (tm.id, tm.text),
        None => (None, None),
    };
    Some(CcfOption {
        value,
        name: o.name.unwrap_or_default(),
        code: o.code.filter(|s| !s.is_empty()),
        tm_id,
        tm_literal,
    })
}

fn engine_from_qual(q: &RawQualifier) -> Option<String> {
    let kind = q
        .qual
        .iter()
        .find(|x| x.qtype.eq_ignore_ascii_case("Type"))?;
    let sub = q
        .qual
        .iter()
        .find(|x| x.qtype.eq_ignore_ascii_case("subtype"))
        .map(|x| x.value.clone());
    Some(match sub {
        Some(s) => format!("{}/{}", kind.value, s),
        None => kind.value.clone(),
    })
}
