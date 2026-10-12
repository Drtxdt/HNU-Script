import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import type { Schema as S } from './generated/Schema';
import type { Workspace as W } from './generated/Workspace';
import type { Question as Q } from './generated/Question';
import type { Execution as E } from './generated/Execution';
import type { Trace as T } from './generated/Trace';
import type { Simulation as M } from './generated/Simulation';
import type { AlgebraResult as A } from './generated/AlgebraResult';
import type { Grade as G } from './generated/Grade';
import type { Analysis as D } from './generated/Analysis';
import type { Table } from './generated/Table';
export type Wire<T> = T extends bigint ? number : T extends Array<infer U> ? Array<Wire<U>> : T extends object ? { [K in keyof T]: Wire<T[K]> } : T;
export type Schema=Wire<S>; export type Workspace=Wire<W>; export type Question=Wire<Q>; export type Execution=Wire<E>; export type Trace=Wire<T>;export type Simulation=Wire<M>;export type AlgebraResult=Wire<A>;export type Grade=Wire<G>;export type Analysis=Wire<D>;
export type {Table};
export const api=<T>(command:string,args:Record<string,unknown>={}):Promise<T> => invoke<T>('rpc',{command,args});
export const byteOffset=(text:string,offset:number):number=>new TextEncoder().encode(text.slice(0,offset)).length;
export async function exportTable(table:Table,format:'csv'|'json'){const path=await save({defaultPath:`查询结果.${format}`,filters:[{name:format.toUpperCase(),extensions:[format]}]});if(path)await api('export',{path,table,format});}
export async function exportText(content:string,name:string){const ext=name.split('.').pop()??'txt';const path=await save({defaultPath:name,filters:[{name:ext.toUpperCase(),extensions:[ext]}]});if(path)await api('export',{path,content});}
