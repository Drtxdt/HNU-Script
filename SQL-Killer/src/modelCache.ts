/** Keep active editors and a bounded LRU of inactive undo histories. */
export class ModelCache<T extends {dispose():void}> {
 private entries=new Map<string,{model:T,users:number,retired:boolean}>();
 constructor(private limit=16){}
 acquire(id:string,create:()=>T){let entry=this.entries.get(id);if(!entry)entry={model:create(),users:0,retired:false};this.entries.delete(id);entry.users++;this.entries.set(id,entry);return entry.model;}
 release(id:string){const entry=this.entries.get(id);if(entry){entry.users=Math.max(0,entry.users-1);if(!entry.users&&entry.retired){entry.model.dispose();this.entries.delete(id);}else if(!entry.users){this.entries.delete(id);this.entries.set(id,entry);}}this.trim();}
 private trim(){const inactive=[...this.entries].filter(([,e])=>!e.users);for(const [id,e] of inactive.slice(0,Math.max(0,inactive.length-this.limit))){e.model.dispose();this.entries.delete(id);}}
 clear(){for(const [id,e] of this.entries)if(!e.users){e.model.dispose();this.entries.delete(id);}else e.retired=true;}
 get size(){return this.entries.size;}
}
