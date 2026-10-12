import type {Trace,Wire} from '../api';
import type {AnimationEvent} from '../generated/AnimationEvent';
export type Frame=Wire<AnimationEvent>&{stepIndex:number,batch?:number};
export type DetailMode='sample'|'full';
export function framesFor(trace:Trace,mode:DetailMode):Frame[]{
 const frames:Frame[]=[];
 trace.steps.forEach((step,stepIndex)=>{
  const events=step.events??[];
  if(mode==='full'||events.length<=20){frames.push(...events.map(e=>({...e,stepIndex})));return;}
  const chosen=new Set<number>([0,events.length-1]);
  for(const predicate of [(e:Wire<AnimationEvent>)=>e.truth==='UNKNOWN',(e:Wire<AnimationEvent>)=>e.kind==='drop',(e:Wire<AnimationEvent>)=>e.message.includes('NULL'),(e:Wire<AnimationEvent>)=>e.sources.length>1]){const i=events.findIndex(predicate);if(i>=0)chosen.add(i);}
  for(let i=0;chosen.size<20&&i<events.length;i++)chosen.add(i);
  let i=0;
  while(i<events.length){if(chosen.has(i)){frames.push({...events[i++],stepIndex});continue;}const first=i;while(i<events.length&&!chosen.has(i))i++;const group=events.slice(first,i);frames.push({...group[0],id:group[0].id+':batch',message:`合批演示 ${group.length} 条记录 · 完整冻结数据仍可查看`,sources:group.flatMap(e=>e.sources).slice(0,24),targets:group.flatMap(e=>e.targets).slice(0,24),stepIndex,batch:group.length});}
 });
 return frames;
}
/** Single logical clock. Seeking is pure and never runs SQL or queues animations. */
export class PlaybackClock {
 index=0;progress=0;playing=false;speed=1;private last:number|undefined;
 constructor(public count:number){}
 play(){if(this.index===this.count-1&&this.progress===1)this.seek(0);this.playing=this.count>0;this.last=undefined;}
 pause(){this.playing=false;this.last=undefined;}
 seek(index:number,progress=0){this.index=Math.max(0,Math.min(Math.max(0,this.count-1),index));this.progress=Math.max(0,Math.min(1,progress));this.last=undefined;}
 tick(now:number){if(!this.playing)return;if(this.last===undefined){this.last=now;return;}let progress=this.progress+Math.min(100,Math.max(0,now-this.last))*this.speed/1000;this.last=now;while(progress>=1&&this.index<this.count-1){progress--;this.index++;}this.progress=Math.min(1,progress);if(this.index===this.count-1&&this.progress===1)this.pause();}
}
export const ease=(p:number)=>p*p*(3-2*p);
export const phase=(p:number)=>p<.15?'定位输入':p<.55?'展示处理':p<.9?'结果落位':'核对结果';

/** Only edges belonging to an observed directed cycle are marked as deadlock edges. */
export function cycleEdges(edges:{source:string,target:string}[]):string[]{
 return edges.filter(edge=>{const queue=[edge.target],seen=new Set<string>();while(queue.length){const node=queue.shift()!;if(node===edge.source)return true;if(seen.has(node))continue;seen.add(node);queue.push(...edges.filter(e=>e.source===node).map(e=>e.target))}return false;}).map(e=>`${e.source}:${e.target}`);
}
