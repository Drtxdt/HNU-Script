import {remote,Key} from 'webdriverio';
import {spawn} from 'node:child_process';
import {mkdir,writeFile,readFile} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
const root=process.cwd();
process.env.NO_PROXY='localhost,127.0.0.1';process.env.no_proxy=process.env.NO_PROXY;
const output=path.join(root,'build/qa',process.env.SQL_KILLER_QA_LABEL??'e2e');await mkdir(output,{recursive:true});
const app=process.env.SQL_KILLER_TEST_EXE??path.join(root,'target/debug/sql-killer.exe');
if(process.env.SQL_KILLER_QA_ISOLATED&&!path.resolve(app).startsWith(path.join(root,'build','qa')+path.sep))throw Error('Isolated QA must target a dedicated extracted copy under build/qa');
const environment={...process.env,SQL_KILLER_DATA_DIR:path.join(output,'data'),WEBVIEW2_BROWSER_EXECUTABLE_FOLDER:path.join(root,'build/runtime-expanded/Microsoft.WebView2.FixedVersionRuntime.154.0.4258.48.x64')};
if(process.env.SQL_KILLER_QA_ISOLATED){for(const key of Object.keys(environment))if(/^(PYTHON|NODE|QT|RUST|CARGO|VIRTUAL_ENV)/i.test(key))delete environment[key];environment.PATH=`${process.env.SystemRoot}\\System32;${process.env.SystemRoot}`;if(!process.env.SQL_KILLER_QA_INSTALLED)delete environment.SQL_KILLER_DATA_DIR;environment.WEBVIEW2_BROWSER_EXECUTABLE_FOLDER=path.join(output,'deliberately-unavailable-runtime');environment.HTTP_PROXY='http://127.0.0.1:1';environment.HTTPS_PROXY='http://127.0.0.1:1';}
const driver=spawn(path.join(root,'build/tools/bin/tauri-driver.exe'),['--native-driver',path.join(root,'build/tools/edge/msedgedriver.exe')],{windowsHide:true,env:environment});
let log='';driver.stdout.on('data',d=>log+=d);driver.stderr.on('data',d=>log+=d);
let browser,runtimeExecutable;
const captures=[];
async function cdp(cmd,params={}){const response=await fetch(`http://127.0.0.1:4444/session/${browser.sessionId}/ms/cdp/execute`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({cmd,params})});const result=await response.json();if(!response.ok)throw Error(JSON.stringify(result));return result.value;}
async function shot(file){if(process.env.SQL_KILLER_QA_SCALE)await cdp('Emulation.setDeviceMetricsOverride',{width:1500,height:960,deviceScaleFactor:Number(process.env.SQL_KILLER_QA_SCALE),mobile:false});const metrics=await browser.execute(()=>({devicePixelRatio,width:innerWidth,height:innerHeight}));const png=Buffer.from((await cdp('Page.captureScreenshot',{format:'png',fromSurface:true,captureBeyondViewport:false})).data,'base64');await writeFile(path.join(output,file),png);captures.push({file,...metrics,pixelWidth:png.readUInt32BE(16),pixelHeight:png.readUInt32BE(20)});}
async function beginScreencast(name){
 const address=browser.capabilities['ms:edgeOptions']?.debuggerAddress;
 assert.ok(address,'dedicated WebDriver browser exposes its own debugging endpoint');
 const targets=await (await fetch(`http://${address}/json/list`)).json();
 const page=targets.find(t=>t.type==='page'&&t.url.includes('tauri.localhost'))??targets.find(t=>t.type==='page');assert.ok(page);
 const directory=path.join(output,`${name}-video`);await mkdir(directory,{recursive:true});
 const socket=new WebSocket(page.webSocketDebuggerUrl);await new Promise((resolve,reject)=>{socket.onopen=resolve;socket.onerror=reject});
 let sequence=0;const pending=new Map(),frames=[],writes=[];
 const command=(method,params={})=>new Promise((resolve,reject)=>{const id=++sequence;pending.set(id,{resolve,reject});socket.send(JSON.stringify({id,method,params}))});
 socket.onmessage=event=>{const message=JSON.parse(event.data);if(message.id&&pending.has(message.id)){const p=pending.get(message.id);pending.delete(message.id);if(message.error)p.reject(Error(JSON.stringify(message.error)));else p.resolve(message.result)}
  if(message.method==='Page.screencastFrame'){const data=message.params;const file=`${String(frames.length).padStart(5,'0')}.jpg`;frames.push({file,time:data.metadata.timestamp});writes.push(writeFile(path.join(directory,file),Buffer.from(data.data,'base64')));socket.send(JSON.stringify({id:++sequence,method:'Page.screencastFrameAck',params:{sessionId:data.sessionId}}));}
 };
 await command('Page.startScreencast',{format:'jpeg',quality:88,maxWidth:1500,maxHeight:960,everyNthFrame:1});
 return async()=>{await command('Page.stopScreencast');socket.close();await Promise.all(writes);assert.ok(frames.length>10,'screencast contains real animation frames');
  await writeFile(path.join(directory,'frames.txt'),frames.map((f,i)=>`file '${f.file}'\nduration ${Math.max(.001,(frames[i+1]?.time??f.time+1/30)-f.time)}`).join('\n'));
  const encoder=spawn(process.env.SQL_KILLER_QA_FFMPEG,['-y','-f','concat','-safe','0','-i',path.join(directory,'frames.txt'),'-vf','fps=30,pad=ceil(iw/2)*2:ceil(ih/2)*2','-c:v','libx264','-preset','veryfast','-crf','21','-pix_fmt','yuv420p',path.join(output,`${name}.mp4`)],{windowsHide:true});let log='';encoder.stderr.on('data',d=>log+=d);const code=await new Promise(resolve=>encoder.on('close',resolve));await writeFile(path.join(output,`${name}-recording.log`),log);assert.equal(code,0);return{capturedFrames:frames.length,capturedSeconds:frames.at(-1).time-frames[0].time};
 };
}
const motionResults=[];
async function motionDemo(name){
 if(!process.env.SQL_KILLER_QA_MOTION)return;
 const view=await browser.$('.trace-view');await view.waitForDisplayed();
 const focus=await view.$('button*=专注演示');if(await focus.isExisting())await focus.click();
 const controls=await view.$('.motion-controls');
 await controls.$('button[title="重置"]').click();
 const layout=await view.$$('svg [data-node]');const positions=await Promise.all(Array.from(layout).map(n=>n.getAttribute('transform')));
 await controls.$('[data-testid="motion-play"]').click();await browser.pause(270);
 await controls.$('[data-testid="motion-play"]').click();
 const frozen=await controls.$('.motion-phase-bar i').getAttribute('style');await browser.pause(350);
 assert.equal(await controls.$('.motion-phase-bar i').getAttribute('style'),frozen,'pause freezes the exact fractional phase');
 assert.deepEqual(await Promise.all(Array.from(layout).map(n=>n.getAttribute('transform'))),positions,'play never re-lays out graph');
 const row=await view.$('.animation-tables tr[data-row]');
 if(await row.isExisting()){await controls.$('[data-testid="motion-play"]').click();await row.click();assert.ok((await controls.$('[data-testid="motion-play"]').getText()).includes('播放'),'record inspection pauses playback');}

 await controls.$('select').selectByAttribute('value','2');
 await controls.$('[data-testid="motion-play"]').click();
 const timing=await browser.executeAsync(done=>{const deltas=[];let last=performance.now(),start=last;function frame(now){deltas.push(now-last);last=now;if(now-start<1800)requestAnimationFrame(frame);else done(deltas)}requestAnimationFrame(frame)});
 await controls.$('[data-testid="motion-play"]').click();
 // Jump via real range keyboard, then replay from its new cursor.
 const range=await controls.$('input[type="range"]');await range.click();await browser.keys(Key.Home);
 await controls.$('select').selectByAttribute('value','1');
 if(name==='motion-join'||name==='motion-filter-aggregate'){for(const node of await view.$$('svg [data-node]'))if((await node.getText()).includes(name==='motion-join'?'连接':'WHERE')){await node.click();break;}}
 await browser.execute(()=>{const view=document.querySelector('.focus-view');if(view)view.scrollTop=0});
 const directory=path.join(output,name);await mkdir(directory,{recursive:true});const frames=[];
 await controls.$('[data-testid="motion-play"]').click();
 const stopRecording=process.env.SQL_KILLER_QA_FFMPEG?await beginScreencast(name):null;
 const started=Date.now();
 while(Date.now()-started<(name==='motion-filter-aggregate'?9500:6500)){const png=Buffer.from((await cdp('Page.captureScreenshot',{format:'png',fromSurface:true,captureBeyondViewport:false})).data,'base64');const file=`${String(frames.length).padStart(4,'0')}.png`;await writeFile(path.join(directory,file),png);frames.push({file,time:Date.now()-started});await browser.pause(70);}
 const recording=stopRecording?await stopRecording():null;
 if((await controls.$('[data-testid="motion-play"]').getText()).includes('暂停'))await controls.$('[data-testid="motion-play"]').click();
 await writeFile(path.join(directory,'frames.txt'),frames.map((f,i)=>`file '${f.file}'\nduration ${Math.max(.01,((frames[i+1]?.time??f.time+100)-f.time)/1000)}`).join('\n'));
 const sorted=timing.slice(2).sort((a,b)=>a-b);motionResults.push({name,recording,pauseFrozen:true,layoutStable:true,rafMedianMs:sorted[Math.floor(sorted.length*.5)],rafP95Ms:sorted[Math.floor(sorted.length*.95)],frames:frames.length,durationMs:Date.now()-started,renderedRows:(await view.$$('tbody tr')).length});
 await shot(`${name}.png`);
 const reduced=await view.$('input[type="checkbox"]');await reduced.click();assert.equal((await view.$$('.motion-packet')).length,0,'reduced motion removes travelling edge packets');await reduced.click();
 await view.$('button=退出专注').click();
}
try{
 await new Promise(r=>setTimeout(r,1500));
 browser=await remote({hostname:'127.0.0.1',port:4444,logLevel:'warn',connectionRetryCount:0,capabilities:{browserName:'wry','tauri:options':{application:app}}});
 if(process.env.SQL_KILLER_QA_CAPS)console.log(JSON.stringify(browser.capabilities));
 await browser.$('[data-page="ide"]').waitForDisplayed({timeout:30000});await browser.$('[data-page="ide"]').click();await browser.$('[data-testid="run"]').waitForDisplayed({timeout:30000});
 const browserPid=Number(browser.capabilities['goog:processID']);assert.ok(Number.isSafeInteger(browserPid)&&browserPid>0);
 runtimeExecutable=await new Promise((resolve,reject)=>{const probe=spawn(process.env.SQL_KILLER_QA_POWERSHELL??'pwsh.exe',['-NoProfile','-NonInteractive','-Command',`[Console]::OutputEncoding = [Text.UTF8Encoding]::new(); $qaBrowser = Get-Process msedgewebview2 -ErrorAction Stop | Where-Object { $_.Parent.Id -eq ${browserPid} } | Select-Object -First 1; if (-not $qaBrowser) { throw 'WebView2 child not found' }; $qaBrowser.Path`],{windowsHide:true});let value='';probe.stdout.on('data',d=>value+=d);probe.on('error',reject);probe.on('close',code=>code?reject(Error('runtime path probe failed')):resolve(value.trim()));});
 if(process.env.SQL_KILLER_QA_ISOLATED)assert.ok(String(runtimeExecutable).toLowerCase().startsWith(path.join(path.dirname(app),'WebView2').toLowerCase()),`isolated process must use its bundled WebView2: ${runtimeExecutable}`);

 if(process.env.SQL_KILLER_QA_SCALE){const response=await fetch(`http://127.0.0.1:4444/session/${browser.sessionId}/ms/cdp/execute`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({cmd:'Emulation.setDeviceMetricsOverride',params:{width:1500,height:960,deviceScaleFactor:Number(process.env.SQL_KILLER_QA_SCALE),mobile:false}})});if(!response.ok)throw Error(`WebView2 scale emulation failed: ${await response.text()}`);console.log('Requested scale',process.env.SQL_KILLER_QA_SCALE,'observed DPR',await browser.execute(()=>devicePixelRatio));}
 await browser.$('[data-testid="run"]').click();
 await browser.waitUntil(async()=>!(await browser.$('[data-testid="run"]').getAttribute('disabled')),{timeout:15000});
 await shot('01-workbench.png');
 console.log('Native SQL execution completed');
 assert.ok((await browser.$('body').getText()).includes('SQL'));
 await browser.$('[data-testid="trace"]').click();
 await browser.waitUntil(async()=>(await browser.$('body').getText()).includes('同一快照'),{timeout:15000});
 await shot('02-trace.png');
 await motionDemo('motion-join');
 if(process.env.SQL_KILLER_QA_EXPORTS){
   await browser.$('.global-flow summary').click();
   // Keep native export commands intact; only replace the OS file chooser
   // with a deterministic path inside this run's dedicated QA directory.
   await browser.execute(directory=>{const native=window.fetch.bind(window);window.fetch=(url,options)=>decodeURIComponent(String(url)).endsWith('/plugin:dialog|save')?Promise.resolve(new Response(JSON.stringify(directory+'\\'+JSON.parse(options.body).options.defaultPath),{headers:{'Content-Type':'application/json','Tauri-Response':'ok'}})):native(url,options)},output);
   for(const [button,file] of [['SVG ↗','数据库过程图.svg'],['PNG ↗','数据库过程图.png'],['报告 ↗','SQL 实验报告.md']]){
     await browser.$(`button=${button}`).click();
     await browser.waitUntil(async()=>readFile(path.join(output,file)).then(data=>data.length>30,()=>false),{timeout:10000});
   }
   assert.ok((await readFile(path.join(output,'数据库过程图.svg'),'utf8')).includes('Student'));
   assert.equal((await readFile(path.join(output,'数据库过程图.png'))).subarray(0,8).toString('hex'),'89504e470d0a1a0a');
   assert.ok((await readFile(path.join(output,'SQL 实验报告.md'),'utf8')).includes('SELECT'));
 }
 if(process.env.SQL_KILLER_QA_MOTION){
   await browser.$('.monaco-editor .view-lines').click();await browser.keys([Key.Control,'a']);await browser.keys('SELECT Sdept,COUNT(*) AS total,COUNT(Sage) AS present FROM Student WHERE Sage>=18 GROUP BY Sdept HAVING COUNT(*)>0;');
   await browser.$('[data-testid="trace"]').click();await browser.$('.animation-stage').waitForDisplayed({timeout:15000});await motionDemo('motion-filter-aggregate');
   await browser.$('[aria-label="主题"]').selectByAttribute('value','dark');await shot('motion-dark.png');await browser.$('[aria-label="主题"]').selectByAttribute('value','light');
 }
 await browser.$('[data-page="algebra"]').click();
 if(process.env.SQL_KILLER_QA_MOTION)await browser.$('[aria-label="代数示例"]').selectByVisibleText('自然连接与下推');
 await browser.$('[data-testid="algebra-run"]').click();
 await browser.waitUntil(async()=>(await browser.$('body').getText()).includes('核对'),{timeout:15000});
 await shot('03-algebra.png');
 if(process.env.SQL_KILLER_QA_MOTION)await browser.$('button=优化后').click();
 await motionDemo('motion-algebra');
 await browser.$('[data-page="transactions"]').click();
 if(process.env.SQL_KILLER_QA_MOTION){await browser.$('.schedule-input select').selectByVisibleText('死锁（可重复读锁模型）');await browser.$('.schedule-input select option[value="locks"]').parentElement().selectByAttribute('value','locks');await browser.$('.schedule-input select option[value="rr"]').parentElement().selectByAttribute('value','rr');}
 await browser.$('[data-testid="simulate"]').click();
 await browser.waitUntil(async()=>(await browser.$('body').getText()).includes('可串行化'),{timeout:15000});
 await shot('04-transactions.png');
 await motionDemo('motion-transactions');
 await browser.$('[data-page="learn"]').click();await browser.$('[data-testid="start-learning"]').click();await browser.$('button=按年龄筛选').click();
 await browser.$('button=查看解析').click();
 await browser.$('button=填入参考答案').click();
 await browser.$('[data-testid="grade"]').click();
 await browser.$('.grade-card.passed').waitForDisplayed({timeout:15000});
 await shot('05-exercises.png');
 if(process.env.SQL_KILLER_QA_RACES){
   await browser.execute(()=>{const original=window.fetch.bind(window);window.fetch=async(url,options)=>{const result=await original(url,options);let command;try{command=JSON.parse(options?.body??'{}').command}catch{command=undefined}if(command==='exercise_trace'||command==='trace')await new Promise(resolve=>setTimeout(resolve,700));return result;}});
   await browser.$('[data-testid="exercise-trace"]').click();await browser.$('.course-tree button:not(.active)').click();await browser.pause(1000);assert.equal(await browser.$('.trace-view').isExisting(),false,'switching question rejects an already completed delayed result');
   await browser.$('button=按年龄筛选').click();
 }

 if(process.env.SQL_KILLER_QA_MOTION){await browser.$('[data-testid="exercise-trace"]').click();await browser.$('.animation-stage').waitForDisplayed({timeout:15000});await motionDemo('motion-exercise');}
 await browser.$('[data-page="ide"]').click();

 if(process.env.SQL_KILLER_QA_RACES){await browser.$('[data-testid="trace"]').click();await browser.$('.editor-toolbar .subtle').click();await browser.pause(1000);assert.equal(await browser.$('.trace-view').isExisting(),false,'cancel rejects a delayed result even when native calculation has finished');}
 const editor=await browser.$('.monaco-editor .view-lines');await editor.click();
 await browser.keys([Key.Control,'a']);await browser.keys("SELECT '中文;字符串' AS 文本, 9007199254740993 AS 整数;");
 await browser.keys([Key.Control,Key.Enter]);
 await browser.waitUntil(async()=>(await browser.$('.result-table').getText()).includes('9007199254740993'),{timeout:15000});
 const invoke=(command,args={})=>browser.executeAsync((command,args,done)=>{window.__TAURI_INTERNALS__.invoke('rpc',{command,args}).then(value=>done({value}),error=>done({error:String(error)}))},command,args).then(r=>{if(r.error)throw Error(r.error);return r.value});
 if(process.env.SQL_KILLER_QA_MOTION){
   await invoke('execute',{sql:'CREATE TABLE IF NOT EXISTS MotionLarge(n); DELETE FROM MotionLarge; INSERT INTO MotionLarge WITH RECURSIVE c(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM c WHERE n<10000) SELECT n FROM c;'});
   await browser.$('.monaco-editor .view-lines').click();await browser.keys([Key.Control,'a']);await browser.keys('SELECT * FROM MotionLarge;');
   await browser.$('[data-testid="trace"]').click();await browser.$('.animation-stage').waitForDisplayed({timeout:20000});
   const eventCount=await browser.$('.motion-event-count').getText();assert.ok(eventCount.includes('42'),'10,000 rows are represented by 20 detailed events plus one batch per operation');
   assert.ok((await browser.$$('.animation-stage tbody tr')).length<80,'bounded virtual rendering');assert.ok((await browser.$$('.flying-record')).length<=2,'bounded overlays');
   await motionDemo('motion-10000');
   // Editing invalidates the frozen playback immediately.
   await browser.$('.monaco-editor .view-lines').click();await browser.keys(' ');await browser.waitUntil(async()=>!(await browser.$('.animation-stage').isExisting()),{timeout:3000});
 }
 await invoke('execute',{sql:'DROP TABLE IF EXISTS RoundTrip;'});
 const initial=await invoke('bootstrap');initial.workspace.documents[0].text="SELECT '工程恢复😀';";
 const project=path.join(output,'中文 空格工程.sqlkproj');await invoke('save_project',{path:project,workspace:initial.workspace});
 await invoke('execute',{sql:'CREATE TABLE RoundTrip(n INTEGER); INSERT INTO RoundTrip VALUES(1);'});
 await invoke('load_project',{path:project});const restored=await invoke('bootstrap');assert.equal(restored.workspace.documents[0].text,"SELECT '工程恢复😀';");assert.ok(!restored.schema.relations.some(r=>r.name==='RoundTrip'));
 await invoke('export',{path:path.join(output,'中文.csv'),table:{columns:['值'],rows:[[{kind:'text',value:'中文,😀'}]],truncated:false},format:'csv'});
 await shot('06-unicode.png');
 if(process.env.SQL_KILLER_QA_MYSQL){
   await browser.$('button[title="MySQL 连接设置"]').click();
   const dialog=await browser.$('[aria-label="MySQL 连接"]');await dialog.$('input[type="number"]').setValue('13367');
   await dialog.$('[data-testid="mysql-connect"]').click();
   await browser.waitUntil(async()=>(await dialog.getText()).includes('三个独立会话'),{timeout:15000});
   await dialog.$('.toolbar select').selectByAttribute('value','bank');await dialog.$('button=创建并切换').click();
   await browser.waitUntil(async()=>(await dialog.getText()).includes('已创建专用沙箱'),{timeout:15000});await dialog.$('[aria-label="关闭连接设置"]').click();
   await browser.$('[data-page="transactions"]').click();await browser.$('button=MySQL 双会话').click();
   const sessions=await browser.$$('.session');await sessions[0].$('.primary').click();
   await browser.waitUntil(async()=>(await browser.$$('.history .file-tabs button')).length===1,{timeout:15000});
   assert.equal((await browser.$$('.history .notice.warning')).length,0,'MySQL session A must succeed');
   await sessions[1].$('.primary').click();await browser.waitUntil(async()=>(await browser.$$('.history .file-tabs button')).length===2,{timeout:15000});
   assert.equal((await browser.$$('.history .notice.warning')).length,0,'MySQL session B must succeed');
   await sessions[0].$('button=回滚').click();await shot('07-mysql-sessions.png');
 }
 const metrics=await browser.execute(()=>({devicePixelRatio:devicePixelRatio,width:innerWidth,height:innerHeight,userAgent:navigator.userAgent}));
 await browser.executeAsync(done=>{window.__TAURI_INTERNALS__.invoke('plugin:window|close',{label:'main'}).then(()=>done(true),e=>done(String(e)))});
 await browser.$('[aria-label="未保存修改"]').waitForDisplayed({timeout:5000});await browser.$('[aria-label="未保存修改"]').$('button=取消').click();
 await writeFile(path.join(output,'result.json'),JSON.stringify({status:'passed',date:new Date().toISOString(),app,isolated:!!process.env.SQL_KILLER_QA_ISOLATED,raceChecks:!!process.env.SQL_KILLER_QA_RACES,runtimeExecutable,motionResults,scaleMode:process.env.SQL_KILLER_QA_SCALE?'WebView2 CDP device-metrics emulation':'native system DPI',metrics,captures},null,2));
}catch(error){if(browser){try{await writeFile(path.join(output,'failure-state.json'),JSON.stringify(await browser.execute(()=>({url:location.href,ready:document.readyState,body:document.body.innerText.slice(0,8000),app:document.getElementById('app')?.innerHTML.slice(0,8000)})),null,2));await writeFile(path.join(output,'browser-errors.json'),JSON.stringify(await browser.getLogs('browser'),null,2))}catch{/* diagnostic APIs may be unavailable */}}if(browser)await browser.saveScreenshot(path.join(output,'failure.png')).catch(()=>{});throw error;}finally{if(browser)await browser.deleteSession().catch(()=>{});driver.kill();await writeFile(path.join(output,'webdriver.log'),log);}

