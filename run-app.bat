@echo off
rem ==========================================================================
rem  JLR OpenDiag - sobe o servidor local (car-scan serve) e abre o app no
rem  navegador. Duplo-clique, ou:   run-app.bat [PORTA_COM]   (padrao COM5)
rem ==========================================================================
setlocal
set "ROOT=%~dp0"
cd /d "%ROOT%"

rem DLLs de runtime do build GNU (mingw) + binarios do cargo
set "PATH=C:\dev\mingw64\bin;%USERPROFILE%\.cargo\bin;%PATH%"

set "PORT=%~1"
if "%PORT%"=="" set "PORT=COM5"

set "EXE=%ROOT%target\debug\car-scan.exe"
set "WEB=%ROOT%app\jlr_app\build\web"

if not exist "%EXE%" (
  echo [erro] nao achei %EXE%
  echo         rode primeiro:  cargo build -p car-scan
  pause
  exit /b 1
)
if not exist "%WEB%\index.html" (
  echo [erro] build web ausente em %WEB%
  echo         rode primeiro:  flutter build web   (dentro de app\jlr_app^)
  pause
  exit /b 1
)

echo.
echo  Mongoose na porta %PORT% . Subindo servidor em http://127.0.0.1:8080
echo  Feche a janela do servidor (ou Ctrl-C nela) para parar.
echo.

rem servidor em janela propria (mostra os logs da leitura do carro)
start "JLR OpenDiag - servidor" "%EXE%" --port %PORT% serve --web "%WEB%"

rem espera o servidor subir e abre o navegador padrao
timeout /t 2 /nobreak >nul
start "" "http://127.0.0.1:8080/"

endlocal
