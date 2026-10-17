import {readFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const json=async path=>JSON.parse(await readFile(new URL('../'+path,import.meta.url),'utf8'));
const expected=(await json('package.json')).version;
assert.equal((await json('src-tauri/tauri.conf.json')).version,expected,'Tauri version differs');
for(const path of ['core/Cargo.toml','src-tauri/Cargo.toml']){
 const text=await readFile(new URL('../'+path,import.meta.url),'utf8');
 assert.equal(text.match(/^version\s*=\s*"([^"]+)"/m)?.[1],expected,path+' version differs');
}
console.log('Version manifests agree:',expected);
