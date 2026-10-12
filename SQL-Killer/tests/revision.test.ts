import {beforeEach,expect,it,vi} from 'vitest';
import {createPinia,setActivePinia} from 'pinia';
import {useLab} from '../src/store';
import {api,byteOffset} from '../src/api';
vi.mock('../src/api',async original=>({...await original<typeof import('../src/api')>(),api:vi.fn()}));
beforeEach(()=>{setActivePinia(createPinia());vi.mocked(api).mockReset();vi.mocked(api).mockResolvedValue(true)});
it('UTF-16 Monaco positions convert into UTF-8 byte offsets',()=>{expect(byteOffset('中😀SELECT',3)).toBe(7)});
it('edited documents reject stale asynchronous results',async()=>{const lab=useLab();let resolve!:(value:unknown)=>void;vi.mocked(api).mockImplementationOnce(()=>new Promise(r=>{resolve=r}));const task=lab.run('query','execute');lab.touch();resolve({rows:[]});expect(await task).toBeUndefined();expect(lab.jobs).toEqual({});});
it('a newer channel job does not receive an older result',async()=>{const lab=useLab();let resolve!:(value:unknown)=>void;vi.mocked(api).mockImplementationOnce(()=>new Promise(r=>{resolve=r}));const old=lab.run('query','execute');const newer=lab.run('query','execute');await newer;resolve({old:true});expect(await old).toBeUndefined();});
