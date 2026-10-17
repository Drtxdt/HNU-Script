// Real Tauri/WebView2 QA, isolated from personal application data.
import {remote,Key} from 'webdriverio';
import {spawn} from 'node:child_process';
import {mkdir,writeFile,readFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
import path from 'node:path';
const root=process.cwd(),output=path.join(root,'build/qa',process.env.SQL_KILLER_QA_LABEL??'self-study');
await mkdir(output,{recursive:true});
process.env.NO_PROXY='localhost,127.0.0.1';process.env.no_proxy=process.env.NO_PROXY;
const env={...process.env,SQL_KILLER_DATA_DIR:path.join(output,'data'),WEBVIEW2_BROWSER_EXECUTABLE_FOLDER:path.join(root,'build/runtime-expanded/Microsoft.WebView2.FixedVersionRuntime.154.0.4258.48.x64')};
const driver=spawn(path.join(root,'build/tools/bin/tauri-driver.exe'),['--native-driver',path.join(root,'build/tools/edge/msedgedriver.exe')],{windowsHide:true,env});
let browser,log='';driver.stdout.on('data',d=>log+=d);driver.stderr.on('data',d=>log+=d);
const captures=[];let nativeMetrics;
async function shot(name){const png=Buffer.from((await cdp('Page.captureScreenshot',{format:'png',fromSurface:true,captureBeyondViewport:false})).data,'base64');await writeFile(path.join(output,name),png);const metrics=await browser.execute(()=>({width:innerWidth,height:innerHeight,devicePixelRatio}));captures.push({name,pixelWidth:png.readUInt32BE(16),pixelHeight:png.readUInt32BE(20),...metrics});}
async function click(selector){await browser.$(selector).waitForDisplayed();await browser.$(selector).click();}
async function edit(selector,text){await click(selector);await browser.keys([Key.Control,'a']);await browser.keys(text);}
async function assertClean(){assert.ok(!(await browser.$('.app-menu').getText()).includes('保存 ●'),'browsing must not mark a saved project dirty');}
async function cdp(cmd,params){const r=await fetch(`http://127.0.0.1:4444/session/${browser.sessionId}/ms/cdp/execute`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({cmd,params})});assert.ok(r.ok);return(await r.json()).value;}
try{
 await new Promise(resolve=>setTimeout(resolve,1000));
 browser=await remote({hostname:'127.0.0.1',port:4444,logLevel:'warn',connectionRetryCount:0,capabilities:{browserName:'wry','tauri:options':{application:process.env.SQL_KILLER_TEST_EXE??path.join(root,'target/debug/sql-killer.exe')}}});
 await browser.$('[data-testid="start-learning"]').waitForDisplayed({timeout:30000});nativeMetrics=await browser.execute(()=>({width:innerWidth,height:innerHeight,devicePixelRatio}));await assertClean();await shot('01-learning.png');
 await click('[data-page="cases"]');await browser.$('.case-work').waitForDisplayed();await assertClean();
 const queries={capacity:"SELECT C.Cno,C.capacity,COUNT(*) actual FROM CaseCourse C JOIN Enrollment E ON E.Cno=C.Cno WHERE E.status='success' GROUP BY C.Cno,C.capacity HAVING COUNT(*)>C.capacity",requests:"SELECT DISTINCT E.request_id,S.Sno,S.Sname FROM Enrollment E JOIN CaseStudent S ON S.Sno=E.Sno WHERE E.request_id IN (SELECT request_id FROM RequestLog WHERE phase='read' AND observed_count=39)",timeline:"SELECT event_seq,request_id,phase,observed_count FROM RequestLog ORDER BY event_seq"};
 for(const [task,sql] of Object.entries(queries)){
  await click(`[data-testid="case-task-${task}"]`);
  await edit('.case-work .monaco-editor .view-lines',sql);
  await click('button=提交证据查询');await browser.$('.case-work .grade-card.passed').waitForDisplayed({timeout:15000});
  await click('button=只读试运行');await browser.waitUntil(async()=>!(await browser.$('button=固定查询证据').getAttribute('disabled')),{timeout:15000});await click('button=固定查询证据');
  const cards=await browser.$$('.evidence-card');const card=cards[cards.length-1];
  await card.$('select:not([multiple])').selectByAttribute('value','stale_read');await card.$('input').setValue(`${task}：支持旧值覆盖的证据`);
 }
 await click('[data-testid="case-task-reproduce"]');await click('button=回放两笔请求的读写与提交');await browser.$('.case-work .trace-view').waitForDisplayed({timeout:15000});
 const numbers=await browser.$$('.case-work .structured-answer input');await numbers[0].setValue('41');await numbers[1].setValue('40');await browser.$('.case-work .structured-answer select').selectByAttribute('value','stale_read');await click('button=验证复现结论');await browser.$('.case-work .grade-card.passed').waitForDisplayed({timeout:15000});
 await click('[data-testid="case-task-repair"]');await browser.$('.case-work select').selectByAttribute('value','row_lock');await click('button=枚举离线模型');await browser.$('.case-work .grade-card.passed').waitForDisplayed({timeout:15000});
 await browser.$('.case-conclusion select').selectByAttribute('value','stale_read');await click('button=重新核验证据并结案');await browser.$('.case-conclusion strong').waitForDisplayed({timeout:15000});assert.equal((await browser.$$('.evidence-card')).length,3);await shot('02-case-closed.png');
 // Replace only native file pickers in this QA window. The real save/export RPCs run.
 await browser.execute(directory=>{const original=window.fetch.bind(window);window.fetch=(url,options)=>decodeURIComponent(String(url)).endsWith('/plugin:dialog|save')?Promise.resolve(new Response(JSON.stringify(directory+'\\'+JSON.parse(options.body).options.defaultPath),{headers:{'Content-Type':'application/json','Tauri-Response':'ok'}})):original(url,options)},output);
 await click('.app-menu > button:nth-of-type(3)');const project=path.join(output,'数据库学习.sqlkproj');await browser.waitUntil(async()=>readFile(project).then(b=>b.length>100,()=>false),{timeout:15000});await browser.waitUntil(async()=>!(await browser.$('.app-menu').getText()).includes('保存 ●'));
 await click('button=导出案件报告');await browser.waitUntil(async()=>readFile(path.join(output,'选课事故调查报告.md')).then(b=>b.length>100,()=>false),{timeout:15000});
 await click('[data-page="learn"]');await click('[data-testid="start-learning"]');await browser.$('.structured-answer').waitForDisplayed();await assertClean();
 for(const title of ['按年龄筛选','选择输出列','尚无成绩']){await click(`button=${title}`);await browser.$('.answer-editor').waitForDisplayed();await assertClean();}
 await click('button=下一层提示');assert.ok((await browser.$('.app-menu').getText()).includes('保存 ●'),'using a hint must persist immediately');
 await click('[data-page="learn"]');await click('[data-testid="start-learning"]');await click('button=尚无成绩');await browser.$('strong=观察提示').waitForDisplayed();
 await click('[data-page="ide"]');await browser.$('[data-testid="run"]').waitForDisplayed();
 await edit('.monaco-editor .view-lines','SELECT \nFROM Student S;');await browser.keys([Key.Control,Key.Home]);await browser.keys(Key.End);
 await browser.$('[data-relation="Student"] [data-column="Sno"]').doubleClick();await click('[data-testid="run"]');await browser.waitUntil(async()=>(await browser.$('.execution-meta').getText()).includes('执行完成'),{timeout:15000});assert.ok((await browser.$('.result-table').getText()).includes('Sno'),'schema insertion must use S alias');
 await click('button=SQL 对照');await click('button=生成同快照对照');await browser.$('.compare-outputs').waitForDisplayed({timeout:15000});await shot('03-comparison.png');
 await click('[data-page="learn"]');
 for(const [width,height] of [[960,640],[1366,768],[1500,960]])for(const scale of [1,1.25,1.5]){
  await cdp('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:scale,mobile:false});await shot(`layout-${width}-${height}-${scale}.png`);
  const clips=await browser.execute(()=>[...document.querySelectorAll('.learning-entry button,.lesson-grid button')].some(el=>el.getBoundingClientRect().right>innerWidth));assert.equal(clips,false,'learning actions must fit the viewport');
 }
 await browser.$('[aria-label="主题"]').selectByAttribute('value','dark');await shot('04-learning-dark.png');await browser.$('[aria-label="主题"]').selectByAttribute('value','light');
 // Reopen the saved case through the UI, then load a legacy example through the UI.
 async function openViaPicker(file){
  await browser.execute(file=>{const previous=window.fetch.bind(window);window.fetch=(url,options)=>decodeURIComponent(String(url)).endsWith('/plugin:dialog|open')?Promise.resolve(new Response(JSON.stringify(file),{headers:{'Content-Type':'application/json','Tauri-Response':'ok'}})):previous(url,options)},file);
  await click('button=打开工程');const pending=await browser.$('[aria-label="未保存修改"]');if(await pending.isExisting())await pending.$('button=不保存').click();
 }
 await openViaPicker(project);await browser.$('.case-conclusion strong').waitForDisplayed({timeout:15000});await assertClean();assert.equal((await browser.$$('.evidence-card')).length,3,'saved case evidence is restored');
 const legacy=path.join(root,'examples/student.sqlkproj');const original=await readFile(legacy);
 await openViaPicker(legacy);await browser.$('[data-testid="run"]').waitForDisplayed({timeout:15000});await assertClean();assert.deepEqual(await readFile(legacy),original,'opening a legacy project must not change it');await shot('05-legacy-restored.png');
 await writeFile(path.join(output,'result.json'),JSON.stringify({status:'passed',nativeMetrics,captures,caseClosed:true,savedCaseRestored:true,legacyRestoredClean:true,aliasInsertion:true,browsingClean:true,caseReport:true,scaleMode:'WebView2 device-metrics emulation; not physical Windows DPI',humanUsabilityStudy:false},null,2));
}catch(error){if(browser)await browser.saveScreenshot(path.join(output,'failure.png')).catch(()=>{});throw error;}finally{if(browser)await browser.deleteSession().catch(()=>{});driver.kill();await writeFile(path.join(output,'webdriver.log'),log);}
