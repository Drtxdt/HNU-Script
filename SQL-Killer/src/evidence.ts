import type {Table,Wire} from './api';
import type {Cell} from './generated/Cell';
import type {EvidenceBoard} from './generated/EvidenceBoard';
export function cellText(cell:Cell|undefined){return !cell||cell.kind==='null'?'NULL':String(cell.value);}
export function numericCell(cell:Cell|undefined){if(!cell||!['integer','unsigned','decimal','real'].includes(cell.kind))return;const n=Number(cellText(cell));return Number.isFinite(n)?n:undefined;}
export function barRows(table:Table,category:number,value:number){return table.rows.slice(0,20).flatMap((r,i)=>{const n=numericCell(r[value]);return n===undefined?[]:[{key:i,label:cellText(r[category]),value:n,text:cellText(r[value])}]});}
export function timelineRows(table:Table){const i=table.columns.indexOf('event_seq');if(i<0)return[];return table.rows.map((row,index)=>({row,index,sequence:numericCell(row[i])})).filter((r):r is typeof r&{sequence:number}=>r.sequence!==undefined).sort((a,b)=>a.sequence-b.sequence||a.index-b.index);}
export function addRelation(board:Wire<EvidenceBoard>,from:string,to:string,kind:string){if(!from||!to||from===to||!['explains','related'].includes(kind)||board.relations.some(r=>r.from===from&&r.to===to&&r.kind===kind)||board.relations.length>=400)return false;board.relations.push({from,to,kind});return true;}

export const evidenceTitles:Record<string,string>={capacity:'人数异常',requests:'请求身份',timeline:'事件日志'};
