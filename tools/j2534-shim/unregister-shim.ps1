#Requires -RunAsAdministrator
# Reverte o shim: restaura o FunctionLibrary para a monpj432.dll real.

$ErrorActionPreference = 'Stop'
$cfgDir = 'C:\ProgramData\jlr_shim'
$entryFile = Join-Path $cfgDir 'entry.txt'
$realFile  = Join-Path $cfgDir 'real.txt'
if (-not (Test-Path $entryFile) -or -not (Test-Path $realFile)) {
  Write-Host "Config do shim não encontrada; nada a reverter." -ForegroundColor Yellow
  exit 0
}
$entryPath = (Get-Content $entryFile -Raw).Trim()
$realPath  = (Get-Content $realFile  -Raw).Trim()
Set-ItemProperty $entryPath -Name FunctionLibrary -Value $realPath
Write-Host "Restaurado: FunctionLibrary -> $realPath" -ForegroundColor Green
Write-Host "O SDD volta a falar direto com a monpj432.dll." -ForegroundColor Green
