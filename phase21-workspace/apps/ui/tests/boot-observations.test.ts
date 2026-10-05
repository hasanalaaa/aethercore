import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { registerHooks } from 'node:module';
import { compile } from 'svelte/compiler';
import { render } from 'svelte/server';
import type { BootMeasurement, StartupItem } from '../src/lib/contracts.ts';
registerHooks({ resolve(specifier, context, next) {
  try { return next(specifier, context); } catch (error) {
    if ((error as { code?: string }).code === 'ERR_UNSUPPORTED_DIR_IMPORT') return next(`${specifier}/index.ts`, context);
    throw error;
  }
}, load(url, context, next) {
  if (url.endsWith('.svelte')) return { format: 'module', shortCircuit: true,
    source: compile(readFileSync(new URL(url), 'utf8'), { filename: new URL(url).pathname, generate: 'server' }).js.code };
  if (url.endsWith('/platform/transport.ts')) return { format: 'module-typescript', shortCircuit: true,
    source: readFileSync(new URL(url), 'utf8').replace("import.meta.env.VITE_AETHERCORE_TEST_TRANSPORT", "'1'") };
  return next(url, context);
} });
Object.assign(globalThis,{ document:{documentElement:{dataset:{},style:{}}}, window:{matchMedia:()=>({matches:false})},
  __AETHERCORE_TEST_TRANSPORT__:{invoke:async()=>null,listen:async()=>()=>{}} });
Object.defineProperty(globalThis,'localStorage',{configurable:true,value:{getItem:()=>null,setItem:()=>{},removeItem:()=>{}}});
const { shellState } = await import('../src/app/shell-state.ts');
const { streamState,createInitialStreamState } = await import('../src/platform/stream-state.ts');
const { default: StartupPage } = await import('../src/features/startup/StartupPage.svelte');
const recorded = 1_790_669_773_000;
const boot = { recordedUnixMs:recorded,hasDuration:true,durationMs:90000,hasSystemBootInstance:true,systemBootInstance:9,
  completedMeasurement:true,hasRawClass:true,rawClassVersion:1,rawClassValue:0,matchesOsRestart:true,
  hasBootStart:true,bootStartUnixMs:recorded-120000,hasBootEnd:true,bootEndUnixMs:recorded-10000,
  coverage:{source:'Microsoft-Windows-Diagnostics-Performance event100',availability:1,hasObservedUnixMs:true,observedUnixMs:recorded},
  comparison:{medianMs:40000,sampleCount:3,windowStartUnixMs:recorded-3*86400000,windowEndUnixMs:recorded,
    hasBaseline:true,baselineMs:30000,baselineCount:5,baselineWindowStartUnixMs:recorded-6*86400000,baselineWindowEndUnixMs:recorded-86400000},
  delays:[{eventId:101,fullPath:'C:\\Apps\\agent.exe',totalTimeMs:1700,degradationTimeMs:300,recordedUnixMs:recorded}]
} as BootMeasurement;
const item = {itemId:'a',kind:'RegistryRun',displayName:'Agent',command:'"C:\\Apps\\agent.exe" --start',enabled:true,
  impact:'Unknown',confidence:'Unknown',manageable:true,protected:false,scope:'User',publisher:'',source:'',protectionReason:'',
  evidenceDetail:'',recommendation:'',serviceChange:false} as StartupItem;
function show(locale:'en'|'ar', value:BootMeasurement=boot, items=[item]) {
  shellState.update(s=>({...s,locale}));
  const state=createInitialStreamState();state.diagnostics.boots=[value];state.diagnostics.completedUnixMs=recorded+10000;
  state.startupSnapshot={...state.startupSnapshot,state:'Ready',items};streamState.set(state);
  return render(StartupPage).body;
}
for(const locale of ['en','ar'] as const) test(`${locale}: actual StartupPage displays measured class, 3-sample median, 5-prior baseline and exact-path historical delay`,()=>{
  const html=show(locale);
  assert.ok(html.includes('0x0'),'recorded class zero needs an explicit presence flag');
  assert.match(html,/data-boot-comparison/,'the actual page lacks a boot comparison');
  assert.match(html,/data-boot-attribution/,'the actual startup item lacks the historical exact-path delay');
  assert.match(html,/Microsoft-Windows-Kernel-Boot/,'raw class needs its source/version');
  assert.doesNotMatch(html,/cold boot|fast startup|saved seconds|إقلاع بارد|وقت موفَّر/i);
});
test('actual page keeps old schema historical and never displays a comparison or matched delay without class proof',()=>{
  const html=show('en',{...boot,hasRawClass:false,comparison:null});
  assert.doesNotMatch(html,/data-boot-comparison|data-boot-attribution/);
});
const { classifiedBoot, bootComparison, historicalDelay }=await import('../src/features/startup/boot-observations.ts');
test('typed presence, unsupported class/version, invalid baseline and clock ordering remain unknown',()=>{
  assert.ok(classifiedBoot(boot));
  for(const patch of [{hasRawClass:false},{rawClassVersion:2},{rawClassValue:-1},{systemBootInstance:0},{completedMeasurement:false},{durationMs:0},{bootEndUnixMs:recorded+1}])
    assert.equal(classifiedBoot({...boot,...patch}),false);
  for(const patch of [{sampleCount:2},{medianMs:0},{windowEndUnixMs:recorded+1},{windowStartUnixMs:recorded}])
    assert.equal(bootComparison({...boot,comparison:{...boot.comparison!,...patch}}),null);
  for(const patch of [{baselineCount:4},{baselineMs:0},{baselineWindowEndUnixMs:recorded}])
    assert.equal(bootComparison({...boot,comparison:{...boot.comparison!,...patch}})?.hasBaseline,false);
});
test('attribution is unique full executable identity, never basename, Name or a causal impact change',()=>{
  const original=structuredClone(item);
  assert.equal(historicalDelay(item,[item],[boot])?.delay.totalTimeMs,1700);
  assert.deepEqual(item,original);
  for(const command of ['agent.exe','C:\\Other\\agent.exe','C:\\Program Files\\agent.exe --start','%APPDATA%\\agent.exe','C:\\Apps\\..\\Apps\\agent.exe','\\\\host\\Apps\\agent.exe'])
    assert.equal(historicalDelay({...item,command},[{...item,command}],[boot]),null,command);
  assert.equal(historicalDelay(item,[item,{...item,itemId:'b'}],[boot]),null,'two entries for one executable');
  assert.equal(historicalDelay(item,[item],[{...boot,delays:[boot.delays![0],boot.delays![0]]}]),null,'two measurements cannot be guessed into one');
  assert.equal(historicalDelay({...item,kind:'Service'},[{...item,kind:'Service'}],[boot]),null,'101 is not service attribution');
  assert.equal(historicalDelay(item,[item],[{...boot,delays:[{...boot.delays![0],eventId:103}]}]),null);
  const service={...item,kind:'Service'};
  assert.equal(historicalDelay(service,[service],[{...boot,delays:[{...boot.delays![0],eventId:103}]}])?.delay.eventId,103);
});
for(const locale of ['en','ar'] as const) test(`${locale}: historical/insufficient baseline and truncated evidence retain limitations`,()=>{
  const html=show(locale,{...boot,matchesOsRestart:false,delaysTruncated:true,comparison:{...boot.comparison!,hasBaseline:false}});
  assert.match(html,/data-boot-comparison/);
  assert.ok(!html.includes(locale==='en'?'Five prior same-class boots:':'خمس عمليات إقلاع سابقة من الفئة نفسها:'));
  const duplicate=show(locale,boot,[item,{...item,itemId:'b'}]);
  assert.doesNotMatch(duplicate,/data-boot-attribution/);
});
