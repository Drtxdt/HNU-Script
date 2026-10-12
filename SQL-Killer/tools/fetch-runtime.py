"""Download the exact official runtime and QA driver, without changing system Edge."""
import hashlib
import json
import pathlib
import urllib.request
import zipfile
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
VERSION = '154.0.4258.48'
URL = 'https://msedge.sf.dl.delivery.mp.microsoft.com/filestreamingservice/files/621dd012-b3d6-4b8b-a6b7-fc3938bfe9d4/Microsoft.WebView2.FixedVersionRuntime.154.0.4258.48.x64.cab'
downloads = ROOT / 'build/downloads'
downloads.mkdir(parents=True, exist_ok=True)
cab = downloads / f'webview2-{VERSION}.cab'
if not cab.exists():
    urllib.request.urlretrieve(URL, cab)
digest=hashlib.file_digest(cab.open('rb'),'sha256').hexdigest()
lock=ROOT/'runtime.lock.json'
if lock.exists() and json.loads(lock.read_text(encoding='utf8'))['sha256']!=digest:
    raise RuntimeError('Fixed WebView2 SHA256 does not match runtime.lock.json')
expanded = ROOT / 'build/runtime-expanded'
expanded.mkdir(exist_ok=True)
if not (expanded / f'Microsoft.WebView2.FixedVersionRuntime.{VERSION}.x64/msedgewebview2.exe').exists():
    subprocess.run(['expand.exe', str(cab), '-F:*', str(expanded)], check=True)
driver = downloads / 'edgedriver.zip'
if not driver.exists():
    urllib.request.urlretrieve(f'https://msedgedriver.microsoft.com/{VERSION}/edgedriver_win64.zip', driver)
with zipfile.ZipFile(driver) as archive:
    archive.extractall(ROOT / 'build/tools/edge')
(ROOT / 'runtime.lock.json').write_text(json.dumps({'version': VERSION, 'architecture': 'x64', 'url': URL, 'sha256': hashlib.file_digest(cab.open('rb'), 'sha256').hexdigest()}, indent=2), encoding='utf-8')
