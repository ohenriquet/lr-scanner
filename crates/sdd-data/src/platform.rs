//! Parser do `PLATFORM_<VEH>_<ANO>.exml` (schema `platform.xsd`).
//!
//! Extrai as redes de barramento, os módulos com seus endereços de
//! diagnóstico e as fontes do CCF (mestre/backup/as-built).

use serde::Deserialize;

use crate::model::{parse_num, CcfRole, CcfSource, Module, Network, Vehicle};

/// Resultado do parse de um `PLATFORM`.
#[derive(Debug)]
pub struct Platform {
    pub vehicle: Vehicle,
    pub networks: Vec<Network>,
    pub modules: Vec<Module>,
    pub ccf_sources: Vec<CcfSource>,
}

#[derive(Debug, Deserialize)]
struct RawPlatform {
    platform_name: Option<String>,
    network_architecture: Option<RawNetArch>,
    car_configuration: Option<RawCarConfig>,
    module_fitment: Option<RawModuleFitment>,
}

#[derive(Debug, Deserialize)]
struct RawNetArch {
    #[serde(default)]
    network: Vec<RawNetwork>,
}

#[derive(Debug, Deserialize)]
struct RawNetwork {
    #[serde(rename = "@net_id")]
    net_id: String,
    #[serde(rename = "@main_net")]
    main_net: Option<String>,
    #[serde(default)]
    protocol: Vec<RawProtocol>,
    can: Option<RawCan>,
}

#[derive(Debug, Deserialize)]
struct RawProtocol {
    #[serde(rename = "@type")]
    ptype: String,
    #[serde(rename = "$text")]
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCan {
    rate: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCarConfig {
    master: Option<RawCcfHolder>,
    #[serde(default)]
    backup: Vec<RawCcfHolder>,
    as_built: Option<RawCcfHolder>,
}

#[derive(Debug, Deserialize)]
struct RawCcfHolder {
    ccf_source: RawCcfSource,
    qualifier: Option<RawQualifier>,
}

#[derive(Debug, Deserialize)]
struct RawCcfSource {
    #[serde(rename = "@upload")]
    upload: Option<String>,
    #[serde(rename = "$text")]
    module: String,
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
struct RawModuleFitment {
    #[serde(default)]
    module: Vec<RawModule>,
}

#[derive(Debug, Deserialize)]
struct RawModule {
    module_code_name: RawCodeName,
    #[serde(default)]
    address: Vec<RawAddress>,
    network: Option<String>,
    fitment: Option<String>,
    tm: Option<RawTm>,
}

#[derive(Debug, Deserialize)]
struct RawCodeName {
    #[serde(rename = "@acronym")]
    acronym: Option<String>,
    #[serde(rename = "@ISOName")]
    iso_name: Option<String>,
    #[serde(rename = "$text")]
    system_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawAddress {
    #[serde(rename = "@type")]
    atype: String,
    #[serde(rename = "@session")]
    session: Option<String>,
    #[serde(rename = "$text")]
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTm {
    #[serde(rename = "@id")]
    id: Option<String>,
}

/// Faz o parse de um `PLATFORM` já decifrado (XML em texto).
///
/// `model` e `model_year` vêm do nome da pasta (ex.: `L319`, `MY10`),
/// `year_code` do sufixo numérico (ex.: `201000`).
pub fn parse(
    xml: &str,
    model: &str,
    model_year: &str,
    year_code: &str,
) -> Result<Platform, crate::DataError> {
    let raw: RawPlatform = quick_xml::de::from_str(xml)?;

    let vehicle = Vehicle {
        model: model.to_string(),
        model_year: model_year.to_string(),
        year_code: year_code.to_string(),
        platform_name: raw.platform_name.unwrap_or_default(),
    };

    let networks = raw
        .network_architecture
        .map(|na| {
            na.network
                .into_iter()
                .map(|n| Network {
                    rate_kbps: n
                        .can
                        .and_then(|c| c.rate)
                        .and_then(|r| r.trim().parse().ok()),
                    diag_protocol: proto(&n.protocol, "diagnostic"),
                    net_protocol: proto(&n.protocol, "network"),
                    net_id: n.net_id,
                    main_net: n.main_net.unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    let modules = raw
        .module_fitment
        .map(|mf| mf.module.into_iter().filter_map(map_module).collect())
        .unwrap_or_default();

    let mut ccf_sources = Vec::new();
    if let Some(cc) = raw.car_configuration {
        if let Some(m) = cc.master {
            ccf_sources.push(map_source(m, CcfRole::Master));
        }
        for b in cc.backup {
            ccf_sources.push(map_source(b, CcfRole::Backup));
        }
        if let Some(a) = cc.as_built {
            ccf_sources.push(map_source(a, CcfRole::AsBuilt));
        }
    }

    Ok(Platform {
        vehicle,
        networks,
        modules,
        ccf_sources,
    })
}

fn proto(list: &[RawProtocol], kind: &str) -> Option<String> {
    list.iter()
        .find(|p| p.ptype == kind)
        .and_then(|p| p.value.clone())
        .map(|s| s.trim().to_string())
}

fn map_module(m: RawModule) -> Option<Module> {
    let system_id = m.module_code_name.system_id?.trim().to_string();
    if system_id.is_empty() {
        return None;
    }
    let addr = |atype: &str, session: &str| -> Option<u32> {
        m.address
            .iter()
            .find(|a| a.atype == atype && a.session.as_deref() == Some(session))
            .and_then(|a| a.value.as_deref())
            .and_then(parse_num)
    };
    Some(Module {
        acronym: m.module_code_name.acronym.unwrap_or_default(),
        iso_name: m.module_code_name.iso_name,
        tx_diag: addr("can_tx", "diag"),
        rx_diag: addr("can_rx", "diag"),
        tx_prog: addr("can_tx", "prog"),
        rx_prog: addr("can_rx", "prog"),
        network: m.network.map(|s| s.trim().to_string()),
        fitment: m.fitment.map(|s| s.trim().to_string()),
        tm_id: m.tm.and_then(|t| t.id),
        system_id,
    })
}

fn map_source(h: RawCcfHolder, role: CcfRole) -> CcfSource {
    let engine_qualifier = h.qualifier.as_ref().and_then(engine_from_qual);
    CcfSource {
        role,
        module: h.ccf_source.module.trim().to_string(),
        upload: h.ccf_source.upload,
        engine_qualifier,
    }
}

/// Monta o qualificador de motor a partir de `qual` Type (+ subtype).
fn engine_from_qual(q: &RawQualifier) -> Option<String> {
    let kind = q
        .qual
        .iter()
        .find(|x| x.qtype.eq_ignore_ascii_case("Type"))?;
    let base = kind.value.clone();
    let sub = q
        .qual
        .iter()
        .find(|x| x.qtype.eq_ignore_ascii_case("subtype"))
        .map(|x| x.value.clone());
    Some(match sub {
        Some(s) => format!("{base}/{s}"),
        None => base,
    })
}
