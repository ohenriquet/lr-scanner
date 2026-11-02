# Protocolo do Mongoose Pro JLR — notas e validação

Implementado no crate `mongoose`. Baseado no `PROTOCOL.md` de
`dgcarr/Mongoose-USB-2-OBDII-Linux-Driver` (LGPL-2.1) e validado contra o
adaptador físico do usuário.

## Descoberta importante: no Windows é uma porta COM (CDC-ACM)

Plugado no Windows 11, o adaptador (USB `18E1:0104`, "Drew Technologies Inc.")
enumera como **Dispositivo Serial USB** ligado ao driver embutido `usbser`,
criando uma porta COM (ex.: COM5). **Não é preciso o driver KMDF do fornecedor
nem o Visual Studio.** Falamos o protocolo nativo direto pela porta serial, o
mesmo caminho do Linux (`/dev/ttyACM*`) e macOS (`/dev/cu.usbmodem*`). Isso
simplifica o porte em todas as plataformas e viabiliza o Android via USB-host
CDC-ACM.

## Enquadramento (implementado em `frame.rs`)

- Prefixo de transporte (4 bytes): comprimento `N` do corpo em `u16` LE, depois
  `N XOR 0x51E6` em `u16` LE. `1 <= N <= 0x1800`.
- Cabeçalho do corpo (12 bytes): dest `u16` LE, source `u16` LE, opcode
  `u16` LE, seq `u16` LE, 4 bytes reservados. Dados após o byte 12.
- Respostas: `req | 0x8000`; a partir de `corpo+12` há `u32` status,
  `corpo+16` contador de microssegundos, `corpo+20` dados (ou texto de erro).

## Handshake de leitura validado (bancada, sem carro)

`mongoose-bench --port COM5`, adaptador do usuário (serial AOLHE0000003666A):

| Comando | Opcode | Resultado |
|---|---|---|
| cEchoPacket | 0x0100 | ecoou `01 02 03 04` exatamente |
| cOpenDevice | 0x0003 | status 0x00000000 |
| cGetBoardInfo | 0x0109 | status 0, dados de placa (abaixo) |

Board info (dados após status+timestamp), observado:
`05 02 01 01 B4 B0 00 00 D8 55 00 00 00 08 01 01 00 10 01 01 ...`
Os blocos `00 10 01 01` e `00 08 01 01` correspondem a firmware 1.1.16.0 e
bootloader 1.1.8.0 (o `GatewayManager.ini` do SDD lista "FW:1.1.16.0
BL:1.1.8.0"). O layout completo será formalizado ao implementar a parte CAN.

## Pendente (precisa do carro ou de captura adicional)

- Abrir canal CAN (0x0006, canal `0x0501`) e ISO15765 (`0x0601`), baud e flags.
- cSetPin (0x0012): corpo `(1,6,14)` para HS-CAN (pinos 6/14); o corpo para
  MS-CAN (pinos 3/11), onde fica o broadcast do CCF, ainda não é conhecido —
  obter por captura USB do driver oficial (USBPcap).
- cOutboundData / cInboundData (0x0008 / 0x0009), filtros de flow control ISO-TP
  (cTableAddEntry 0x000D, seletor 2).
- Leitura de tensão da bateria (cIoctl / cSetPin).
