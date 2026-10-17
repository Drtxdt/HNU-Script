param([string]$Label='',[switch]$PortableOnly)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$expectedVersion=(Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json).version
if(-not $Label){$Label=$expectedVersion}
if($Label -ne $expectedVersion){throw 'Packaging label must match the current source version'}
Set-Location $projectRoot
$delivery=Join-Path $projectRoot 'release'
$out=Join-Path $delivery '.staging'
if(git status --porcelain --untracked-files=normal){throw 'Commit source changes before packaging'}
& "$PSScriptRoot/clean-build.ps1" -Stage prepare
New-Item -ItemType Directory -Force $out | Out-Null
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
$tracked=git -c core.quotepath=false ls-files -- .
foreach($relative in $tracked){$origin=Join-Path $projectRoot $relative;$destination=Join-Path $source $relative;New-Item -ItemType Directory -Force (Split-Path $destination -Parent)|Out-Null;Copy-Item -LiteralPath $origin -Destination $destination}
$zip=Join-Path $out "SQL-Killer-$Label-Windows-x64.zip"
tar.exe -a -cf $zip -C $out (Split-Path $portable -Leaf)
if($LASTEXITCODE){throw 'ZIP 创建失败'}
if(-not $PortableOnly){Copy-Item -LiteralPath "target/release/bundle/nsis/SQL-Killer_${Label}_x64-setup.exe" -Destination $out -Force}
$commit=(git rev-parse HEAD).Trim()
@{version=$Label;source_commit=$commit;status='release-candidate';generated_at=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $out "$Label-VERIFICATION.json")
& "$PSScriptRoot/clean-build.ps1" -Stage replace -Version $Label
Get-ChildItem -LiteralPath $out -Force | Move-Item -Destination $delivery -Force
& "$PSScriptRoot/clean-build.ps1" -Stage publish -Version $Label
Get-ChildItem -LiteralPath $delivery -File | Where-Object {$_.Extension -in '.zip','.exe'} | ForEach-Object {"$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLower())  $($_.Name)"} | Set-Content -LiteralPath (Join-Path $delivery 'SHA256SUMS.txt')
Write-Output (Join-Path $delivery "SQL-Killer-$Label-Windows-x64.zip")
