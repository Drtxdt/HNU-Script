"""Collect distributed dependency license texts with original attribution."""
import pathlib,json,subprocess,shutil,os,urllib.request,concurrent.futures
root=pathlib.Path(__file__).resolve().parents[1]
out=root/'licenses';out.mkdir(exist_ok=True)
env={**os.environ,'CARGO_HOME':str(root/'build/cargo')}
metadata=json.loads(subprocess.check_output(['cargo','metadata','--format-version','1','--offline'],cwd=root,env=env))
index=[]
def copy_texts(name,version,folder,license_id):
    target=out/f'{name.replace("/","_").replace("@","")}-{version}'
    files=[p for p in folder.iterdir() if p.is_file() and p.name.lower().startswith(('license','licence','copying','notice','copyright'))]
    if files:
        target.mkdir(exist_ok=True)
        for file in files: shutil.copyfile(file,target/file.name)
    index.append({'package':name,'version':version,'license':license_id,'texts':[str((target/p.name).relative_to(out)) for p in files]})
for package in metadata['packages']:
    if package.get('source'):copy_texts(package['name'],package['version'],pathlib.Path(package['manifest_path']).parent,package.get('license'))
# Some crate archives omit the workspace-root license. Retrieve that exact
# revision's upstream license instead of silently omitting its attribution.
def upstream_license(package):
    entry=next((x for x in index if x['package']==package['name'] and x['version']==package['version']),None)
    if not entry:return
    entry['source']=f"https://crates.io/crates/{package['name']}/{package['version']}"
    if entry['texts']:return
    folder=pathlib.Path(package['manifest_path']).parent
    vcs=folder/'.cargo_vcs_info.json'
    repo=(package.get('repository') or '').removesuffix('.git')
    if not vcs.exists() or not repo.startswith('https://github.com/'):return
    revision=json.loads(vcs.read_text())['git']['sha1']
    base=repo.replace('https://github.com/','https://raw.githubusercontent.com/')+'/'+revision+'/'
    target=out/f"{package['name']}-{package['version']}"
    for name in ['LICENSE','LICENSE-MIT','LICENSE.md','LICENSE.txt','COPYING']:
        try:
            data=urllib.request.urlopen(base+name,timeout=10).read()
            target.mkdir(exist_ok=True);(target/name).write_bytes(data)
            entry['texts']=[str((target/name).relative_to(out))];entry['license_source']=base+name
            return
        except (OSError,ValueError):pass
    # Preserve source-file copyright notices when the repository also has no
    # standalone license; the original SPDX expression remains in INDEX.json.
    for name in ['README.md','Readme.md','src/lib.rs','lib.rs']:
        candidate=folder/name
        if candidate.is_file() and 'license' in candidate.read_text(encoding='utf8',errors='replace').lower():
            target.mkdir(exist_ok=True);shutil.copyfile(candidate,target/'UPSTREAM-NOTICE.txt')
            entry['texts']=[str((target/'UPSTREAM-NOTICE.txt').relative_to(out))];return
with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:
    list(pool.map(upstream_license,(p for p in metadata['packages'] if p.get('source'))))
seen=set()
for manifest in (root/'node_modules/.pnpm').glob('*/node_modules/*/package.json'):
    p=json.loads(manifest.read_text(encoding='utf8'));key=(p.get('name'),p.get('version'))
    if None in key or key in seen:continue
    seen.add(key);copy_texts(*key,manifest.parent,p.get('license'))
    index[-1]['source']=f"https://www.npmjs.com/package/{p['name']}/v/{p['version']}"
for manifest in (root/'node_modules/.pnpm').glob('*/node_modules/@*/*/package.json'):
    p=json.loads(manifest.read_text(encoding='utf8'));key=(p.get('name'),p.get('version'))
    if None in key or key in seen:continue
    seen.add(key);copy_texts(*key,manifest.parent,p.get('license'))
    index[-1]['source']=f"https://www.npmjs.com/package/{p['name']}/v/{p['version']}"
(out/'INDEX.json').write_text(json.dumps(index,ensure_ascii=False,indent=2),encoding='utf8')
(out/'README.txt').write_text('SQL-Killer uses MIT-licensed project code. Dependency license texts retain their original attribution. INDEX.json records resolved packages including build/test dependencies. SQLite is public domain. Microsoft WebView2 is a separately licensed redistributed runtime; its embedded credits and Microsoft terms apply. No course PDFs are redistributed.\n',encoding='utf8')
print(f'Collected {len(index)} package attributions')
