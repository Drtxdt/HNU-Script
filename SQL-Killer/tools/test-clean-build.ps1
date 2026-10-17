$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
$root=Join-Path $project ('build/clean-tests/'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $root|Out-Null
Set-Content -LiteralPath (Join-Path $root 'package.json') '{"version":"1.3.0"}'
foreach($path in @('release/SQL-Killer-1.2.0-Windows-x64/data','build/cargo','build/qa/old-app','build/qa/current')){New-Item -ItemType Directory -Force (Join-Path $root $path)|Out-Null}
Set-Content -LiteralPath (Join-Path $root 'release/SQL-Killer-1.2.0-Windows-x64/data/recovery.sqlkproj') 'user data'
Set-Content -LiteralPath (Join-Path $root 'release/SQL-Killer-1.2.0-Windows-x64/my-investigation.sqlkproj') 'user project'
$failed=$false
try{& "$PSScriptRoot/clean-build.ps1" -Stage publish -Root $root}catch{$failed=$true}
if(-not $failed -or -not(Test-Path -LiteralPath (Join-Path $root 'release/SQL-Killer-1.2.0-Windows-x64'))){throw 'Failed builds must retain previous delivery'}
Set-Content -LiteralPath (Join-Path $root 'release/SQL-Killer-1.3.0-Windows-x64.zip') 'current delivery'
& "$PSScriptRoot/clean-build.ps1" -Stage publish -Root $root
& "$PSScriptRoot/clean-build.ps1" -Stage qa -Root $root
if(Test-Path -LiteralPath (Join-Path $root 'release/SQL-Killer-1.2.0-Windows-x64')){throw 'Old release survived'}
if(Test-Path -LiteralPath (Join-Path $root 'build/qa/old-app')){throw 'Old QA survived'}
if(-not(Test-Path -LiteralPath (Join-Path $root 'build/cargo'))){throw 'Cache lost'}
if(-not(Get-ChildItem -LiteralPath (Join-Path $root 'local-data/retained') -Recurse -Filter recovery.sqlkproj)){throw 'User data lost'}
try{& "$PSScriptRoot/clean-build.ps1" -Root (Split-Path $project -Parent);throw 'Out-of-root accepted'}catch{if($_ -match 'Out-of-root accepted'){throw}}
& "$PSScriptRoot/clean-build.ps1" -Stage publish -Root $root
if(-not(Get-ChildItem -LiteralPath (Join-Path $root 'local-data/retained') -Recurse -Filter my-investigation.sqlkproj)){throw 'Portable root project lost'}
'Cleanup lifecycle tests passed'

$junction=Join-Path $root 'build/qa/unsafe-link'
New-Item -ItemType Junction -Path $junction -Target (Join-Path $root 'build/cargo')|Out-Null
$rejected=$false
try{& "$PSScriptRoot/clean-build.ps1" -Stage qa -Root $root}catch{$rejected=$true}
if(-not $rejected){throw 'Reparse point accepted'}
Remove-Item -LiteralPath $junction -Force
'Reparse point test passed'

$rootAlias=Join-Path $project ('build/clean-tests/alias-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Junction -Path $rootAlias -Target $root|Out-Null
$rootRejected=$false
try{& "$PSScriptRoot/clean-build.ps1" -Stage qa -Root $rootAlias}catch{$rootRejected=$true}
if(-not $rootRejected){throw 'Reparse cleanup root accepted'}
Remove-Item -LiteralPath $rootAlias -Force
$resolvedTestRoot=[IO.Path]::GetFullPath($root)
$testBase=[IO.Path]::GetFullPath((Join-Path $project 'build/clean-tests'))+[IO.Path]::DirectorySeparatorChar
if(-not $resolvedTestRoot.StartsWith($testBase,[StringComparison]::OrdinalIgnoreCase)){throw 'Unsafe test cleanup path'}
if(Get-ChildItem -LiteralPath $resolvedTestRoot -Recurse -Force|Where-Object{$_.Attributes -band [IO.FileAttributes]::ReparsePoint}){throw 'Unexpected test reparse point'}
Remove-Item -LiteralPath $resolvedTestRoot -Recurse -Force
