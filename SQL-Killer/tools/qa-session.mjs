import {spawnSync} from 'node:child_process';
import path from 'node:path';
export function qaDirectory(label){
 if(!/^[\p{L}\p{N} ._-]+$/u.test(label)||label.includes('..'))throw Error('Invalid QA label');
 const cleaned=spawnSync('pwsh.exe',['-NoProfile','-File',path.join(process.cwd(),'tools/clean-build.ps1'),'-Stage','qa'],{encoding:'utf8',windowsHide:true});
 if(cleaned.error||cleaned.status)throw cleaned.error??Error(cleaned.stderr);
 return path.join(process.cwd(),'build/qa/current',label);
}
