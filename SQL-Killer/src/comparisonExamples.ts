import type {Schema} from './api';
export function comparisonExamples(schema:Schema):[string,string]{
 const names=schema.relations.map(r=>r.name);
 if(names.includes('Student')&&names.includes('SC'))return ['SELECT S.Sno,C.Cno FROM Student S LEFT JOIN SC C ON S.Sno=C.Sno;','SELECT S.Sno,C.Cno FROM Student S INNER JOIN SC C ON S.Sno=C.Sno;'];
 const table=schema.relations[0];if(!table||!table.columns.length)return ['',''];
 const quote=(v:string)=>'"'+v.replaceAll('"','""')+'"';const col=quote(table.columns[0]!.name),name=quote(table.name);
 return [`SELECT ${col} FROM ${name};`,`SELECT DISTINCT ${col} FROM ${name};`];
}
