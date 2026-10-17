<script setup lang="ts">
import {ref,watch,onBeforeUnmount} from 'vue';
import {useLab} from '../store';
import type {Wire} from '../api';
import type {QueryComparison} from '../generated/QueryComparison';
import ResultTable from './ResultTable.vue';
import TraceView from './TraceView.vue';
const lab=useLab();
const left=ref('SELECT S.Sno,C.Cno FROM Student S LEFT JOIN SC C ON S.Sno=C.Sno;');
const right=ref('SELECT S.Sno,C.Cno FROM Student S INNER JOIN SC C ON S.Sno=C.Sno;');
const result=ref<Wire<QueryComparison>>();
watch([left,right,()=>lab.engine],()=>{lab.invalidateJobs(['compare']);result.value=undefined});
watch(()=>lab.epoch,()=>{result.value=undefined});
async function compare(){const r=await lab.run<Wire<QueryComparison>>('compare','compare_queries',{engine:'sqlite',left:left.value,right:right.value});if(r)result.value=r;}
onBeforeUnmount(()=>lab.invalidateJobs(['compare']));
</script>
<template><section class="query-comparison"><h2>两条 SQL，同一份冻结数据</h2><p>对照使用自由实验的 SQLite 数据库。按行多重集比较结果，列按位置对应；顺序可在输出中观察。结果相同只代表当前数据。</p><div class="compare-inputs"><label>查询 A<textarea v-model="left" aria-label="对照查询 A"/></label><label>查询 B<textarea v-model="right" aria-label="对照查询 B"/></label></div><button class="primary" :disabled="lab.engine!=='sqlite'||!!lab.jobs.compare" @click="compare">生成同快照对照</button><p v-if="lab.engine!=='sqlite'" class="notice">请切换到 SQLite；MySQL 使用单查询过程及真实会话实验。</p><template v-if="result"><p class="notice">{{result.diff.equal?'当前数据的结果多重集相同':'当前数据的结果不同'}} · A {{result.left.result.rows.length}} 行 / B {{result.right.result.rows.length}} 行</p><div class="compare-outputs"><div><ResultTable :table="result.left.result" :diff-rows="result.diff.expected" title="查询 A"/><TraceView :trace="result.left"/></div><div><ResultTable :table="result.right.result" :diff-rows="result.diff.actual" title="查询 B"/><TraceView :trace="result.right"/></div></div></template></section></template>
