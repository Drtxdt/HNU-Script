import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import { api,type Workspace,type Schema,type Question } from './api';
export const useLab=defineStore('lab',()=>{
 const workspace=ref<Workspace>();const schema=ref<Schema>({relations:[],routines:[],version:0,engine:'SQLite'});const questions=ref<Question[]>([]);const engine=ref<'sqlite'|'mysql'>('sqlite');const page=ref('ide');const theme=ref('system');const dirty=ref(false);const revision=ref(0);const status=ref('准备就绪');const error=ref('');const jobs=ref<Record<string,string>>({});const projectPath=ref('');const recovery=ref(false);const logs=ref<string[]>([]);const active= computed(()=>workspace.value?.documents.find(d=>d.id===workspace.value?.active_document));
 function log(message:string){logs.value.unshift(`${new Date().toLocaleTimeString()}  ${message}`);logs.value=logs.value.slice(0,100);status.value=message;}
 function fail(e:unknown){error.value=String(e);log(String(e));}
 async function bootstrap(){try{const r=await api<{workspace:Workspace,schema:Schema,questions:Question[],recovery:boolean}>('bootstrap');workspace.value=r.workspace;schema.value=r.schema;questions.value=r.questions;recovery.value=r.recovery;}catch(e){fail(e)}}
 function touch(){revision.value++;dirty.value=true;void api('invalidate').catch(fail);}
 async function cancel(channel?:string){const selected=channel?{[channel]:jobs.value[channel]}:{...jobs.value};for(const [key,id] of Object.entries(selected))if(jobs.value[key]===id)delete jobs.value[key];await Promise.all(Object.values(selected).filter(Boolean).map(job_id=>api('cancel',{job_id})));log('已请求取消任务');}
 async function run<T>(channel:string,command:string,args:Record<string,unknown>={}):Promise<T|undefined>{if(jobs.value[channel])await cancel(channel);const job_id=crypto.randomUUID();jobs.value[channel]=job_id;const rev=revision.value;error.value='';try{const result=await api<T>(command,{engine:engine.value,...args,job_id});if(rev!==revision.value||jobs.value[channel]!==job_id)return;return result;}catch(e){if(rev===revision.value)fail(e);}finally{if(jobs.value[channel]===job_id)delete jobs.value[channel];}}
 async function refresh(){try{schema.value=await api<Schema>('schema',{engine:engine.value});}catch(e){fail(e)}}
 function restore(w:Workspace,s?:Schema){workspace.value=w;if(s)schema.value=s;engine.value='sqlite';page.value=typeof w.view==='object'&&w.view&&'page'in w.view?String(w.view.page):'ide';theme.value=typeof w.view==='object'&&w.view&&'theme'in w.view?String(w.view.theme):'system';revision.value++;dirty.value=false;}
 function snapshot():Workspace{if(!workspace.value)throw new Error('工作区未加载');return JSON.parse(JSON.stringify({...workspace.value,view:{...(workspace.value.view&&typeof workspace.value.view==='object'&&!Array.isArray(workspace.value.view)?workspace.value.view:{}),theme:theme.value,page:page.value}})) as Workspace;}
 return{workspace,schema,questions,engine,page,theme,dirty,revision,status,error,jobs,projectPath,recovery,logs,active,log,fail,bootstrap,touch,cancel,run,refresh,restore,snapshot};
});
