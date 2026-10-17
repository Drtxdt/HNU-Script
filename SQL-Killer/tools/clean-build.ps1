param([ValidateSet('prepare','qa','publish','replace','finish')][string]$Stage='prepare',[string]$Version='', [switch]$DryRun,[string]$Root='')
$ErrorActionPreference='Stop'
$actualRoot=[IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
if(-not $Root){$Root=$actualRoot}
$Root=[IO.Path]::GetFullPath($Root)
if($Root -ne $actualRoot -and -not $Root.StartsWith((Join-Path $actualRoot 'build/clean-tests')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Cleanup test root is outside the allowed directory'}
$rootCursor=$Root
while($rootCursor){$rootEntry=Get-Item -LiteralPath $rootCursor -Force -ErrorAction SilentlyContinue;if($rootEntry -and $rootEntry.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Cleanup root contains a reparse point'};if($rootCursor -eq $actualRoot){break};$rootCursor=[IO.Path]::GetDirectoryName($rootCursor)}
if(-not $Version){$Version=(Get-Content -LiteralPath (Join-Path $Root 'package.json') -Raw|ConvertFrom-Json).version}
$records=[Collections.Generic.List[string]]::new()
function Assert-ArtifactPath([string]$Path){
 $absolute=[IO.Path]::GetFullPath($Path)
 $allowed=@('build','release','target/release/bundle/nsis','dist')|ForEach-Object{[IO.Path]::GetFullPath((Join-Path $Root $_))}
 if(-not ($allowed|Where-Object{$absolute.StartsWith($_+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)})){throw "Refusing cleanup outside artifact directories: $absolute"}
 $cursor=$absolute
 while($cursor -and $cursor -ne $Root){$entry=Get-Item -LiteralPath $cursor -Force -ErrorAction SilentlyContinue;if($entry -and $entry.Attributes -band [IO.FileAttributes]::ReparsePoint){throw "Refusing reparse point: $cursor"};$cursor=[IO.Path]::GetDirectoryName($cursor)}
 return $absolute
}
function Remove-Artifact([string]$Path){
 $absolute=Assert-ArtifactPath $Path
 if(-not(Test-Path -LiteralPath $absolute)){return}
 $linked=Get-ChildItem -LiteralPath $absolute -Recurse -Force -ErrorAction Stop | Where-Object {$_.Attributes -band [IO.FileAttributes]::ReparsePoint}|Select-Object -First 1
 if($linked){throw "Refusing nested reparse point: $($linked.FullName)"}
 $records.Add("remove $absolute")
 if(-not $DryRun){Remove-Item -LiteralPath $absolute -Recurse -Force}
}
function Preserve-PortableData([string]$Path){
 $absolute=Assert-ArtifactPath $Path
 $packaged=@('SQL-Killer.exe','portable.flag','WebView2','source','curriculum','examples','licenses','README.md','VALIDATION.md','runtime.lock.json','LICENSE')
 $entries=@(Get-ChildItem -LiteralPath $absolute -Force|Where-Object{$_.Name -notin $packaged})
 if($entries.Count){
  $destination=Join-Path $Root ("local-data/retained/"+(Split-Path $absolute -Leaf)+'-'+[guid]::NewGuid().ToString('N'))
  foreach($entry in $entries){
   $null=Assert-ArtifactPath $entry.FullName
   if(Get-ChildItem -LiteralPath $entry.FullName -Recurse -Force|Where-Object{$_.Attributes -band [IO.FileAttributes]::ReparsePoint}){throw 'User data contains a reparse point'}
   $records.Add("preserve $($entry.FullName) -> $destination")
  }
  $cursor=[IO.Path]::GetFullPath($destination)
  while($cursor -and $cursor -ne $Root){$entry=Get-Item -LiteralPath $cursor -Force -ErrorAction SilentlyContinue;if($entry -and $entry.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Backup destination contains a reparse point'};$cursor=[IO.Path]::GetDirectoryName($cursor)}
  if(-not $DryRun){New-Item -ItemType Directory -Force $destination|Out-Null;foreach($entry in $entries){Copy-Item -LiteralPath $entry.FullName -Destination $destination -Recurse}}
 }
}
if($Stage -eq 'finish'){
 foreach($path in @('build/qa/current/portable-app','build/clean-tests')){$target=Join-Path $Root $path;if(Test-Path -LiteralPath $target){Remove-Artifact $target}}
}elseif($Stage -eq 'replace'){
 $staged=Join-Path $Root "release/.staging/SQL-Killer-$Version-Windows-x64.zip"
 if(-not(Test-Path -LiteralPath $staged)){throw 'Staged delivery missing'}
 $old=Join-Path $Root "release/SQL-Killer-$Version-Windows-x64"
 if(Test-Path -LiteralPath $old){Preserve-PortableData $old;Remove-Artifact $old}
}elseif($Stage -eq 'qa'){
 $qa=Join-Path $Root 'build/qa'
 if(Test-Path -LiteralPath $qa){Get-ChildItem -LiteralPath $qa -Force|Where-Object{$_.Name -notin @('current','mysql-data','mysql.pid')}|ForEach-Object{Remove-Artifact $_.FullName}}
}elseif($Stage -eq 'publish'){
 $release=Join-Path $Root 'release'
 if(-not(Test-Path -LiteralPath (Join-Path $release "SQL-Killer-$Version-Windows-x64.zip"))){throw 'Current delivery missing; keeping previous release'}
 $keep=@("SQL-Killer-$Version-Windows-x64","SQL-Killer-$Version-Windows-x64.zip","SQL-Killer_${Version}_x64-setup.exe","$Version-VERIFICATION.json",(($Version -replace '\.0$','')+'-VERIFICATION.json'),'SHA256SUMS.txt','.staging')
 Get-ChildItem -LiteralPath $release -Force|Where-Object{$_.Name -notin $keep}|ForEach-Object{if($_.PSIsContainer){Preserve-PortableData $_.FullName};Remove-Artifact $_.FullName}
 $nsis=Join-Path $Root 'target/release/bundle/nsis'
 if(Test-Path -LiteralPath $nsis){Get-ChildItem -LiteralPath $nsis -File|Where-Object{$_.Name -ne "SQL-Killer_${Version}_x64-setup.exe"}|ForEach-Object{Remove-Artifact $_.FullName}}
}else{
 foreach($directory in @('dist','release/.staging','build/package-current','build/clean-tests')){
  $path=Join-Path $Root $directory
  if(Test-Path -LiteralPath $path){Get-ChildItem -LiteralPath $path -Force|ForEach-Object{Remove-Artifact $_.FullName}}
 }
}
if($Stage -eq 'prepare'){Get-ChildItem -LiteralPath (Join-Path $Root 'build') -File -ErrorAction SilentlyContinue | Where-Object {$_.Extension -in @('.log','.txt') -and ($_.Name -ne 'cleanup-current.log')} | ForEach-Object {Remove-Artifact $_.FullName}}
if(-not $DryRun){$log=Join-Path $Root 'build/cleanup-current.log';New-Item -ItemType Directory -Force (Split-Path $log)|Out-Null;$records|Add-Content -LiteralPath $log}
$records
