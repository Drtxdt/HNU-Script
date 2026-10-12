import {beforeEach,expect,it,vi} from 'vitest';
import {createPinia,setActivePinia} from 'pinia';
const {api}=vi.hoisted(()=>({api:vi.fn()}));
vi.mock('../src/api',()=>({api}));
import {useLab} from '../src/store';
beforeEach(()=>{setActivePinia(createPinia());api.mockReset()});
it('cancel rejects an already computed but not yet delivered result',async()=>{
 let deliver:(value:string)=>void=()=>{};
 api.mockImplementation((command:string)=>command==='trace'?new Promise<string>(resolve=>deliver=resolve):Promise.resolve());
 const lab=useLab();const pending=lab.run<string>('trace','trace');expect(lab.jobs.trace).toBeTruthy();
 await lab.cancel('trace');deliver('obsolete');expect(await pending).toBeUndefined();expect(lab.jobs.trace).toBeUndefined();
});
it('a new document revision rejects a previous calculation',async()=>{
 let deliver:(value:string)=>void=()=>{};
 api.mockImplementation((command:string)=>command==='trace'?new Promise<string>(resolve=>deliver=resolve):Promise.resolve());
 const lab=useLab();const pending=lab.run<string>('trace','trace');lab.touch();deliver('obsolete');expect(await pending).toBeUndefined();
});
