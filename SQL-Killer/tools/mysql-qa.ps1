param([ValidateSet('start','stop')][string]$Action='start')
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$qa=Join-Path $projectRoot 'build/qa'
$data=Join-Path $qa 'mysql-data'
$mysql='C:\Program Files\MySQL\MySQL Server 9.5\bin'
New-Item -ItemType Directory -Force $qa | Out-Null
if($Action -eq 'stop'){
 & (Join-Path $mysql 'mysqladmin.exe') --no-defaults --host=127.0.0.1 --port=13367 --user=root shutdown
 exit $LASTEXITCODE
}
if(-not(Test-Path (Join-Path $data 'mysql.ibd'))){
 New-Item -ItemType Directory -Force $data | Out-Null
 & (Join-Path $mysql 'mysqld.exe') --no-defaults --initialize-insecure "--datadir=$data" --console
 if($LASTEXITCODE){throw '初始化失败'}
}
$server=Start-Process -FilePath (Join-Path $mysql 'mysqld.exe') -ArgumentList '--no-defaults',"--datadir=$($data.Replace('\','/'))",'--port=13367','--bind-address=127.0.0.1','--mysqlx=0','--console' -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $qa 'mysql-out.log') -RedirectStandardError (Join-Path $qa 'mysql-error.log')
$server.Id | Set-Content (Join-Path $qa 'mysql.pid')
Write-Output '独立 QA 实例：127.0.0.1:13367。仅用于本机测试，root 空密码；请测试完后执行 -Action stop。'
