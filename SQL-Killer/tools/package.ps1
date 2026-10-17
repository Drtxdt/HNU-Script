param([string]$Label='',[switch]$PortableOnly)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
if(-not $Label){$Label=(Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json).version}
Set-Location $projectRoot
$out=Join-Path $projectRoot 'release'
$portable=Join-Path $out "SQL-Killer-$Label-Windows-x64"
New-Item -ItemType Directory -Force $portable | Out-Null
Copy-Item -LiteralPath target/release/sql-killer.exe -Destination (Join-Path $portable 'SQL-Killer.exe')
$runtime=Join-Path $projectRoot 'build/runtime-expanded/Microsoft.WebView2.FixedVersionRuntime.154.0.4258.48.x64'
$runtimeTarget=Join-Path $portable 'WebView2'
New-Item -ItemType Directory -Force $runtimeTarget | Out-Null
Get-ChildItem -LiteralPath $runtime -Force | Copy-Item -Destination $runtimeTarget -Recurse -Force
Set-Content -LiteralPath (Join-Path $portable 'portable.flag') -Value 'SQL-Killer portable data directory'
foreach($file in @('README.md','VALIDATION.md','runtime.lock.json','LICENSE')){Copy-Item -LiteralPath $file -Destination $portable}
foreach($directory in @('curriculum','examples','licenses')){Copy-Item -LiteralPath $directory -Destination $portable -Recurse -Force}
$source=Join-Path $portable 'source'
New-Item -ItemType Directory -Force $source | Out-Null
$excluded=@('build','target','dist','node_modules','release','.git')
Get-ChildItem -LiteralPath $projectRoot -Force | Where-Object {$_.Name -notin $excluded} | ForEach-Object {Copy-Item -LiteralPath $_.FullName -Destination $source -Recurse -Force}
$zip=Join-Path $out "SQL-Killer-$Label-Windows-x64.zip"
tar.exe -a -cf $zip -C $out (Split-Path $portable -Leaf)
if($LASTEXITCODE){throw 'ZIP 创建失败'}
if(-not $PortableOnly){Get-ChildItem target/release/bundle/nsis -Filter '*.exe' | Copy-Item -Destination $out -Force}
Get-ChildItem -LiteralPath $out -File | Where-Object {$_.Extension -in '.zip','.exe'} | ForEach-Object {"$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLower())  $($_.Name)"} | Set-Content -LiteralPath (Join-Path $out 'SHA256SUMS.txt')
Write-Output $zip
