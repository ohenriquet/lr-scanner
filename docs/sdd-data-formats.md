# Formatos de dados do JLR SDD (notas de engenharia reversa)

Referência para os parsers do crate `sdd-data`. Baseado na instalação do SDD
v165 em `C:\Program Files (x86)\JLR`. Caminhos relativos a essa raiz.

## Criptografia `.exml`

3DES (EDE, três chaves) em modo ECB, sem IV, preenchimento PKCS#5. Chave de 24
bytes pública (ver crate `sdd-crypto`). Confirmado contra o par plano/cifrado
`IDS\Xml\JLR_VSV_CONFIG.xml` ↔ `.exml` e contra toda a pasta `IDS\Xml`.
`SDD\Xml` é cópia de `IDS\Xml` (mesmo SHA-256 por arquivo).

## `PLATFORM_<VEH>_<ANO>.exml` (schema `platform.xsd`)

Topologia e endereçamento de uma plataforma. Para o L319 MY10:

- `network_architecture > network`: redes `CAN_HS` (500 kbps), `CAN_MS`
  (125 kbps) e `SUB_MOST` (gateway pelo ACM). Cada uma com protocolo
  diagnóstico `ISO14229` (UDS) e de rede `ISO15765`, endereçamento `normal`
  (11 bits).
- `car_configuration`: define a fonte mestre do CCF e os backups.
  - `master > ccf_source upload="did"`: `BCM_SYSTEM_A` (o CCF mestre é lido/
    gravado por DID no BCM).
  - `backup`: `PCM_SYSTEM_A`, com `upload="did"` ou `upload="mem"` conforme o
    tipo de motor (`V8NA`, `V6NA`, `V6DIESEL/3L`, `V6DIESEL/2.7L`).
  - `as_built > ccf_source upload="vbf"`: `AS_BUILT`.
- `module_fitment > module` (53 módulos): cada um com
  `module_code_name acronym=... ISOName=...`, endereços
  `address type="can_tx|can_rx" session="diag|prog"` (ex.: ABS tx `0x760`,
  rx `0x768`), `network`, conjuntos de DID (`data_identifier_set`), `fitment`
  (mandatory/optional) e título `tm id="@..."`.

Este arquivo é a fonte primária da tabela `module` (endereços, barramento,
sessões) e do vínculo veículo→módulos.

## `MDX_<MOD>.exml` e `<MOD>.exml` (DTD `MDX-002.0.dtd`)

Definição por módulo. `IDS\Xml\<VEH>_<ANO>\`. Estrutura:

- `ADMINISTRATION`: nome do ECU, shortname.
- `ECU_DATA > DATA_IDENTIFIERS > DID`: cada DID tem `NUMBER` (ex. `0xDD01`),
  `BYTE_SIZE`, `DID_TYPE` (`single_value`, `packeted`), `ACCESS_PARAMETERS`
  (`READABLE`/`WRITEABLE` com `SERVICE_REFS` = `service_22`/`service_2E`,
  `SESSION_REFS`, `SECURITY_REFS`) e `SUB_FIELD` com `LEAST_SIG_BIT`/
  `MOST_SIG_BIT` e `DATA_DEFINITION` (tipo, `NUMERIC_PARAMETERS` com
  `RESOLUTION`/`OFFSET`/`UNITS`/`RANGE_*`, ou `ENUMERATED_PARAMETERS`).
- Também contém `DTC`, `ROUTINE` (parâmetros de start/stop/results) e dados de
  protocolo/segurança por módulo (endereços, sessões, níveis).

Fonte da tabela `did` (número, subcampo, bits, escala, unidade, enum) e
`routine`.

## `CCF_DATA_<VEH>_<ANO>.exml` (schema `ccf_data.xsd`)

**O mapa de bits do CCF** — núcleo da edição de configuração. Para o L319 MY10:
cabeçalho cita "SA5AC/01/96 Rev 4.01, EUCD 31830271 Rev 6", `vbf_version 2.0`.

- `block id="1" name="CCF" serviceIdRd="0x22" serviceIdWr="0x2E"`: o CCF é
  lido com ReadDataByIdentifier e gravado com WriteDataByIdentifier.
  - `address module=... t_ccf="L319_201000_BCM_CCF.vbf" parameterId="0xDE00"
    start_address="0x00FFAC00" stop_address="0x00FFACC3" merge="true"`:
    por módulo/motor, qual template VBF, qual DID (`0xDE00` no BCM, `0xF106`
    no PCM) e a faixa de endereços. O bloco tem 196 bytes (`length="196"`).
  - `group start=N stop=M name="GROUP_CCF_..."`: agrupa parâmetros por byte(s).
  - `parameter id="BBB_BBB_bbb_bbb" name="PARAM_CCF_..." mask="0xFF"
    type="ENUM|BOOL|..."`: o `id` codifica byte inicial, byte final, bit
    inicial e bit final (ex. `047_047_002_002` = byte 47, bits 2..2, máscara
    `0x04`). `category display/edit/scope` controla visibilidade/edição.
    `select > option value="0x.." name=".." code=".."`: valores possíveis, com
    `code` = códigos de opção de fábrica (WERS) e `tm id="@..."` = texto.

Exemplos de iluminação de conforto no L319 MY10 (confirmados):
`PARAM_CCF_INT_LIGHT_APPROACH_LAMP` (byte 47 bit 2, BOOL),
`PARAM_CCF_EXT_LIGHT_FOLLOW_ME_HOME_REVERSE` (byte 46 bit 7, BOOL),
`PARAM_CCF_EUCD_PUDDLE_LIGHT_OUTER_MIRRORS` (byte 86, ENUM: nenhum/ambos/
motorista/passageiro), `PARAM_CCF_EUCD_HOMESAFE_LIGHTING` (byte 148, ENUM:
desligado/30..180 s).

Fonte das tabelas `ccf_block`, `ccf_param`, `ccf_option`.

## `Security.exml` (schema `Security.xsd`)

Dados de SecurityAccess por grupo de módulo: constantes (`DataConstant`) e
grupos (`SecDataGroup Index=.. Title="L319_<MOD>" ProgDataRef=.. DiagDataRef=..`)
que referenciam as constantes de programação e de diagnóstico. Combina-se com
os níveis/serviços de seed-key dos arquivos MDX e com a desmontagem de
`SecAlg.dll` para reconstruir o algoritmo 0x27 (Fase 5). **Conteúdo sensível:
não versionar nem exibir as constantes.**

## `CONFIG_MENU_<VEH>_<ANO>.exml` / `MDX_PROFILE_*` / `CCF_RULES_*`

`CONFIG_MENU`: menus das funções de configuração (itens, `job_list`, `funcName`,
teste SPA). `MDX_PROFILE`: perfis de rotina (`run_routine`,
`routine_start_parameters`, `rw_identifier`, `sec_level`). `CCF_RULES`: regras de
sincronização/consistência entre parâmetros do CCF. Fontes das rotinas (Fase 6)
e da validação do CCF (Fase 5).

## `VINDecode.exml` / `VEHICLE_MANIFEST*`

Decodifica o VIN em modelo/ano/motor; o manifesto liga variante→arquivos. Usado
para detectar o veículo a partir do VIN lido (`0x22 F190`).

## Arquivos planos de apoio (fora de `.exml`)

- `Common\SDD\Data\ids\MODEL_YEAR_BREAKPOINTS.xml`: VIN-ano → pasta
  `MYxx`/`<ano>`. L319: 2005-09→MY05/200500, 2010-11→MY10/201000,
  2012-13→MY12/201200, 2014-16→MY14/201400.
- `ProgramData\JLR\IDS\OnlinePIDHelp\Data\PROCS\mdl.xml`: dicionário de 3.956
  DIDs (nome, bits, módulos, linhas de veículo, códigos de motor).
- `Common\SDD\Data\<lang>\gradex\Converters\CVT_*.xml`: conversores de escala/
  enumeração para live data.
- `Common\SDD\Data\<lang>\gradex\Snapshot\*DID Formatting.xml`: layout de
  bit-field dos DIDs com link para o conversor.
- `IDS\Xml\text\@<L>\@<MNEMONICO>.xml`: textos por mnemônico, 15 idiomas
  (inclui PTG). Resolve os `tm id="@..."` dos arquivos acima.
- `IDS\Xml\gdx.xml`: texto de DTC por número.
- `Common\Flash\Vbf\*_CCF.vbf`: templates VBF `sw_part_type=CARCFG` (cabeçalho
  plano) para montar o pacote de gravação do CCF.

## Templates VBF de CCF (L319 MY10)

| Módulo | Arquivo | ecu_address | start..stop | erase |
|---|---|---|---|---|
| BCM (mestre) | `L319_201000_BCM_CCF.vbf` | 0x726 | 0x00FFAC00..C3 | 0x00FFAC00 len 0x400 |
| PCM diesel 3L | `L319_201000_PCM_DIESEL_3L_CCF.vbf` | 0x7E0 | 0x80800000..C3 | 0x80800000 len 0x2000 |
| GWM (MY14) | `L319_201400_GWM_CCF.vbf` | 0x716 | — | — |
