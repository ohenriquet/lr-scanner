# App (interface)

`prototype.html` é o protótipo interativo de alta fidelidade da interface do
scanner, responsivo para celular e tablet. Abre em qualquer navegador; usa dados
reais do L319 (VIN, módulos, DTCs, opções de CCF) e um gráfico de tempo real com
dados simulados (o carro não precisa estar conectado para avaliar o layout).

Telas: Painel (veículo + módulos), Ao vivo (seleção de parâmetros + gráfico
empilhado/sobreposto com crosshair), Falhas (DTCs por módulo), Config (editor de
CCF). Tema claro/escuro.

Próximo passo: portar este desenho para o app Flutter (Windows/macOS/Android),
ligado ao núcleo Rust (mongoose + diag + ccf) via flutter_rust_bridge, trocando
os dados simulados pela telemetria real do veículo.
