<script setup lang="ts">
import {computed,ref,onBeforeUnmount} from 'vue';
import {useLab} from '../store';
import {type Table,exportText} from '../api';
import ResultTable from '../components/ResultTable.vue';
import {orderedCourse,resumeQuestion} from '../learningFlow';
const lab=useLab();const section=ref('home');const selected=ref('');const choice=ref(-1);const feedback=ref<{correct:boolean,table:Table,explanation:string}>();
const checkpoint=computed(()=>lab.predictions.find(p=>p.id===selected.value));
const offline=computed(()=>orderedCourse(lab.questions));const passed=computed(()=>offline.value.filter(q=>lab.workspace?.progress[`${q.id}@${lab.workspace.exercise_seed}`]==='passed').length);
const resume=computed(()=>resumeQuestion(lab.questions,lab.workspace?.progress??{},lab.workspace?.exercise_seed??0,lab.workspace?.learning.last_question));
const lesson=computed(()=>lab.lessons.find(l=>l.question===resume.value?.id));
onBeforeUnmount(()=>lab.invalidateJobs(['prediction']));
function mode(value:string){lab.workspace!.learning.mode=value;lab.markDirty();}
async function predict(){if(!checkpoint.value||choice.value<0)return;const id=checkpoint.value.id;const r=await lab.run<{correct:boolean,table:Table,explanation:string}>('prediction','predict',{id,choice:choice.value});if(r&&checkpoint.value?.id===id){feedback.value=r;lab.workspace!.learning.predictions[id]=r.correct;lab.markDirty();}}
function choose(id:string){lab.invalidateJobs(['prediction']);selected.value=id;choice.value=-1;feedback.value=undefined;}
async function report(){await exportText(`# 学习记录

离线主线：${passed.value}/${offline.value.length}

`+Object.entries(lab.workspace!.learning.attempts).map(([k,p])=>`- ${k}：${p?.status}，${p?.attempts} 次，${p?.assisted?'辅助':'独立'}作答`).join(String.fromCharCode(10)),'学习记录.md');}
</script>
<template><div class="lab-page learning-page"><header class="learning-heading"><div><span class="eyebrow">一步一个发现 · DATABASE LAB</span><h1>今天，从这里继续</h1></div><button @click="report().catch(lab.fail)">导出学习记录</button></header>
 <section v-if="section==='home'" class="learning-hero"><div><span class="eyebrow">{{passed===0?'第一站':'你的下一步'}} · {{resume?`第 ${resume.chapter} 章`:'离线主线已完成'}}</span><h2>{{resume?.title??'带着问题，开始自由实验'}}</h2><p>{{lesson?.explanation??resume?.knowledge??'你已完成离线主线，可以复习错题或进入选课调查。'}}</p><button class="primary" data-testid="start-learning" @click="resume?lab.study(resume.id):(lab.page='cases')">{{resume?`继续学习：${resume.title}`:'进入案件调查'}} →</button><small>可以随时从课程地图选择其他章节。</small></div><aside><strong>{{passed}} / {{offline.length}}</strong><span>离线主线已通过</span><progress :value="passed" :max="offline.length||31"/><p>先预测，再运行；观察结果后解释原因。MySQL 拓展另行统计。</p><span>{{lab.workspace?.learning.mode==='free'?'自主学习 · 直接实验':'教学辅助 · 目标与提示'}}</span></aside></section>
 <nav class="learning-sections" aria-label="学习入口"><button v-for="[id,title] in [['home','继续学习'],['map','课程地图'],['exercises','全部练习与错题'],['predictions','预测练习'],['settings','学习设置']]" :key="id" :class="{active:section===id}" @click="section=id">{{title}}</button></nav>
 <template v-if="section==='home'"><div class="learning-destinations"><article><span class="eyebrow">探索</span><h3>自由实验</h3><p>SQL、关系代数与事务工作台，使用你自己的问题。</p><button @click="lab.page='ide'">进入实验室 →</button></article><article><span class="eyebrow">调查档案 01</span><h3>选课异常调查</h3><p>教务处收到一份投诉。用数据核验，再用证据解释。</p><button @click="lab.page='cases'">打开调查档案 →</button></article></div></template>
 <section v-else-if="section==='map'"><h2>六段入门引导</h2><div class="lesson-grid"><article v-for="(l,i) in lab.lessons" :key="l.id"><span class="eyebrow">0{{i+1}}</span><h3>{{l.title}}</h3><p>{{l.explanation}}</p><button @click="lab.study(l.question)">开始这个实验</button></article></div><h2>完整学习路线</h2><div class="course-map"><button v-for="q in offline" :key="q.id" @click="lab.study(q.id)"><span>{{lab.workspace?.progress[`${q.id}@${lab.workspace.exercise_seed}`]==='passed'?'✓':'○'}}</span><small>第 {{q.chapter}} 章</small>{{q.title}}</button></div></section>
 <section v-else-if="section==='exercises'" class="learning-summary"><h2>课程练习与错题</h2><p>保留原课件章号，支持直接进入任意章节。MySQL 题目在完整题库中单独标识。</p><button class="primary" @click="lab.study(lab.selectedQuestion||resume?.id||'relation')">打开完整题库</button><button @click="lab.study('relation')">从第一节复习</button></section>
 <section v-else-if="section==='predictions'"><h2>八个预测检查点</h2><p>先选择，再用真实 SQLite 验证。预测错误不会阻止继续学习。</p><div class="checkpoint-list"><button v-for="c in lab.predictions" :key="c.id" :class="{active:selected===c.id}" @click="choose(c.id)">{{lab.workspace?.learning.predictions[c.id]?'✓ ':''}}{{c.title}}</button></div><section v-if="checkpoint" class="prediction-panel"><h3>{{checkpoint.prompt}}</h3><label v-for="(c,i) in checkpoint.choices" :key="c"><input v-model="choice" type="radio" :value="i" name="prediction">{{c}}</label><div class="toolbar"><button class="primary" :disabled="choice<0||!!lab.jobs.prediction" @click="predict">执行并验证预测</button><button @click="choose('')">跳过</button></div><div v-if="feedback" role="status"><strong>{{feedback.correct?'预测与引擎结果一致':'观察结果，再调整理解'}}</strong><p>{{feedback.explanation}}</p><ResultTable :table="feedback.table" compact/></div><details><summary>查看验证 SQL</summary><pre>{{checkpoint.sql}}</pre></details></section></section>
 <section v-else class="learning-summary"><h2>选择适合自己的学习方式</h2><label>学习方式<select :value="lab.workspace?.learning.mode||'guided'" @change="mode(($event.target as HTMLSelectElement).value)"><option value="guided">教学辅助：目标、预测、分级提示</option><option value="free">自主学习：直接进入编辑与结果</option></select></label><p>可以随时切换。自主学习仍可主动打开提示与解析，已有进度保留。</p></section>
</div></template>
