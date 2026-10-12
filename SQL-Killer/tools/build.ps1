param([switch]$SkipChecks)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
Set-Location $projectRoot
$env:CARGO_HOME=Join-Path $projectRoot 'build/cargo'
$env:RC='C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\rc.exe'
if(-not $SkipChecks){
  cargo test -p sql-killer-core --locked
  if($LASTEXITCODE){throw 'Rust 测试失败'}
  cargo clippy --workspace --all-targets --locked -- -D warnings
  if($LASTEXITCODE){throw 'Clippy 失败'}
  pnpm lint
  if($LASTEXITCODE){throw 'Lint 失败'}
  pnpm test
  if($LASTEXITCODE){throw 'Vitest 失败'}
}
pnpm tauri build
if($LASTEXITCODE){throw '应用构建失败'}
Write-Output 'NSIS 位于 target/release/bundle/nsis；便携归档请运行 tools/package.ps1。'
