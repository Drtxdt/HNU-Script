<script setup lang="ts">
import {computed,onBeforeUnmount,ref,watch} from 'vue';
import {useLab} from '../store';
import type {Table} from '../api';
import ResultTable from './ResultTable.vue';
const props=defineProps<{id:string}>();const emit=defineEmits<{skip:[]}>();const lab=useLab();
const p=computed(()=>lab.predictions.find(p=>p.id===props.id));const choice=ref(-1);const result=ref<{correct:boolean,table:Table,explanation:string}>();
const channel='inline-prediction';
watch(()=>props.id,()=>{lab.invalidateJobs([channel]);choice.value=-1;result.value=undefined});
async function verify(){const id=props.id;const r=await lab.run<{correct:boolean,table:Table,explanation:string}>(channel,'predict',{id,choice:choice.value});if(r&&id===props.id){result.value=r;lab.workspace!.learning.predictions[id]=r.correct;lab.markDirty();}}
function skip(){lab.invalidateJobs([channel]);emit('skip');}
onBeforeUnmount(()=>lab.invalidateJobs([channel]));
</script>
<template><section v-if="p" class="prediction-panel"><h3>先预测 · {{p.title}}</h3><p>{{p.prompt}}</p><label v-for="(c,i) in p.choices" :key="c"><input v-model="choice" type="radio" :value="i" :name="'inline-'+id">{{c}}</label><div class="toolbar"><button class="primary" :disabled="choice<0||!!lab.jobs[channel]" @click="verify">执行并验证预测</button><button @click="skip">跳过预测，直接实验</button></div><div v-if="result" role="status"><strong>{{result.correct?'预测与引擎结果一致':'观察真实结果，再调整理解'}}</strong><p>{{result.explanation}}</p><ResultTable :table="result.table" compact/></div><details><summary>查看验证 SQL</summary><pre>{{p.sql}}</pre></details></section></template>
