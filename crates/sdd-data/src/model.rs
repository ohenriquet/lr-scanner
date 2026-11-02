//! Tipos de domínio produzidos pelos parsers e gravados no pacote SQLite.

/// Um veículo+ano importado (ex.: L319 / MY10).
#[derive(Debug, Clone)]
pub struct Vehicle {
    /// Código de plataforma (ex.: `L319`).
    pub model: String,
    /// Ano-modelo interno do SDD (ex.: `MY10`).
    pub model_year: String,
    /// Ano numérico da pasta (ex.: `201000`).
    pub year_code: String,
    /// Nome legível da plataforma (ex.: `L319_10MY`).
    pub platform_name: String,
}

/// Uma rede de barramento do veículo.
#[derive(Debug, Clone)]
pub struct Network {
    pub net_id: String,
    pub main_net: String,
    /// Taxa em kbps (500, 125...).
    pub rate_kbps: Option<u32>,
    /// Protocolo diagnóstico (ex.: `ISO14229`).
    pub diag_protocol: Option<String>,
    /// Protocolo de rede (ex.: `ISO15765`, `MOST`).
    pub net_protocol: Option<String>,
}

/// Um módulo (ECU) e seus endereços de diagnóstico.
#[derive(Debug, Clone)]
pub struct Module {
    /// Sigla (ex.: `ABS`, `BCM`).
    pub acronym: String,
    /// Nome ISO.
    pub iso_name: Option<String>,
    /// Identificador de sistema (ex.: `ABS_SYSTEM_A`).
    pub system_id: String,
    /// Endereço de requisição (tx) em sessão de diagnóstico, se houver.
    pub tx_diag: Option<u32>,
    /// Endereço de resposta (rx) em sessão de diagnóstico, se houver.
    pub rx_diag: Option<u32>,
    /// Endereço de requisição em sessão de programação.
    pub tx_prog: Option<u32>,
    /// Endereço de resposta em sessão de programação.
    pub rx_prog: Option<u32>,
    pub network: Option<String>,
    /// `mandatory` / `optional`.
    pub fitment: Option<String>,
    /// id do texto descritivo (ex.: `@J_14229_M_DESC_ABS`).
    pub tm_id: Option<String>,
}

/// Papel de uma fonte de CCF no veículo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CcfRole {
    Master,
    Backup,
    AsBuilt,
}

/// Uma fonte de CCF (módulo que guarda uma cópia do CCF).
#[derive(Debug, Clone)]
pub struct CcfSource {
    pub role: CcfRole,
    /// Módulo que guarda a cópia (ex.: `BCM_SYSTEM_A`).
    pub module: String,
    /// Método de upload (`did`, `mem`, `vbf`).
    pub upload: Option<String>,
    /// Qualificador de motor, quando a fonte depende do motor (ex.: `V6DIESEL/3L`).
    pub engine_qualifier: Option<String>,
}

/// Um bloco de CCF (faixa de endereços + serviço de leitura/escrita).
#[derive(Debug, Clone)]
pub struct CcfBlock {
    pub block_id: String,
    pub name: String,
    /// Serviço UDS de leitura (ex.: `0x22`).
    pub service_rd: Option<u8>,
    /// Serviço UDS de escrita (ex.: `0x2E`).
    pub service_wr: Option<u8>,
    /// Tamanho do bloco em bytes (ex.: 196).
    pub length: Option<u32>,
}

/// Endereçamento de um bloco de CCF num módulo/motor específico.
#[derive(Debug, Clone)]
pub struct CcfAddress {
    pub block_id: String,
    pub module: String,
    /// Template VBF (ex.: `L319_201000_BCM_CCF.vbf`).
    pub t_ccf: Option<String>,
    /// DID que lê/grava o CCF (ex.: `0xDE00`).
    pub parameter_id: Option<u32>,
    pub start_address: Option<u32>,
    pub stop_address: Option<u32>,
    pub merge: bool,
    pub engine_qualifier: Option<String>,
}

/// Um parâmetro configurável do CCF (uma opção que o usuário pode ler/editar).
#[derive(Debug, Clone)]
pub struct CcfParam {
    pub block_id: String,
    /// Grupo (ex.: `GROUP_CCF_EUCD_PUDDLE_LIGHT_OUTER_MIRRORS`).
    pub group: String,
    /// Nome do parâmetro (ex.: `PARAM_CCF_INT_LIGHT_APPROACH_LAMP`).
    pub name: String,
    /// id bruto `BBB_BBB_bbb_bbb`.
    pub raw_id: String,
    /// Primeiro byte do parâmetro dentro do bloco.
    pub byte_start: u16,
    /// Último byte do parâmetro.
    pub byte_stop: u16,
    /// Bit inicial (0..7) dentro do byte.
    pub bit_start: u8,
    /// Bit final (0..7).
    pub bit_stop: u8,
    /// Máscara aplicada (ex.: `0x04`).
    pub mask: Option<u32>,
    /// Tipo (`BOOL`, `ENUM`, ...).
    pub ptype: Option<String>,
    /// Pode ser exibido ao usuário.
    pub display: bool,
    /// Pode ser editado.
    pub editable: bool,
    /// Escopo (`base`, ...).
    pub scope: Option<String>,
    /// id do texto do título (ex.: `@APPROACH_LAMPS`), se houver.
    pub title_tm: Option<String>,
    /// Texto literal do título, quando embutido no XML.
    pub title_literal: Option<String>,
    /// Opções possíveis.
    pub options: Vec<CcfOption>,
}

/// Uma opção possível de um parâmetro de CCF.
#[derive(Debug, Clone)]
pub struct CcfOption {
    /// Valor cru (ex.: `0x04`).
    pub value: u32,
    /// Nome simbólico (ex.: `TRUE`, `PUDDLE_LIGHT_DRV`).
    pub name: String,
    /// Códigos de opção de fábrica (WERS), separados por vírgula.
    pub code: Option<String>,
    /// id do texto (ex.: `@J_QV_FITTED`).
    pub tm_id: Option<String>,
    /// Texto literal, quando embutido.
    pub tm_literal: Option<String>,
}

/// Decodifica o `id` de um parâmetro de CCF (`BBB_BBB_bbb_bbb`).
pub fn parse_param_id(raw: &str) -> Option<(u16, u16, u8, u8)> {
    let parts: Vec<&str> = raw.split('_').collect();
    if parts.len() != 4 {
        return None;
    }
    let byte_start = parts[0].parse().ok()?;
    let byte_stop = parts[1].parse().ok()?;
    let bit_start = parts[2].parse().ok()?;
    let bit_stop = parts[3].parse().ok()?;
    Some((byte_start, byte_stop, bit_start, bit_stop))
}

/// Converte uma string hex/decimal do SDD (ex.: `0x760`, `196`) em u32.
pub fn parse_num(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        s.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodifica_id_de_parametro() {
        assert_eq!(parse_param_id("047_047_002_002"), Some((47, 47, 2, 2)));
        assert_eq!(parse_param_id("086_086_000_007"), Some((86, 86, 0, 7)));
        assert_eq!(parse_param_id("lixo"), None);
    }

    #[test]
    fn converte_numeros() {
        assert_eq!(parse_num("0x760"), Some(0x760));
        assert_eq!(parse_num("196"), Some(196));
        assert_eq!(parse_num("0xDE00"), Some(0xDE00));
    }
}
