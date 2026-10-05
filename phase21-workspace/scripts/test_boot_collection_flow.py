#!/usr/bin/env python3
"""Compile the actual boot controller/parser with a read-only Event Log boundary shim.

No Windows API, owner history, servicing call or extra dependency is used. The
production collect_boots body and boot parser remain unchanged in the fixture;
only the three bounded event reads and clock/cancellation boundary are controlled.
"""
from pathlib import Path
import os
import re
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SHIM = r'''
#![allow(dead_code)]
use std::cell::RefCell;
use boot::{BootEvidence, KernelBootClass, PERFORMANCE_PROVIDER, boot_from_event, delay_from_event, bind_boot_evidence};
#[derive(Debug,PartialEq)] enum CrashError { Unavailable, Cancelled }
type Result<T> = std::result::Result<T,CrashError>;
struct Utc;
impl Utc {fn now()->Self{Self} fn timestamp_millis(&self)->i64{10_000}}
struct CollectorControl;
#[derive(Clone,Copy)] enum BootFieldType {U32,FileTime,Text}
struct RenderedEventSystem {provider:String,event_id:u32,version:Option<u8>,recorded_filetime:u64,recorded_unix_ms:i64}
struct NamedBootEvent {system:RenderedEventSystem,fields:Vec<(&'static str,Option<String>)>}
impl NamedBootEvent {fn field(&self,name:&str)->Option<String>{self.fields.iter().find(|(key,_)|*key==name).and_then(|(_,v)|v.clone())}}
#[derive(Default)] struct State {calls:Vec<String>,incomplete:Option<usize>,failed:Option<usize>,cancelled:bool,unsupported:bool}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
fn checkpoint(_:&CollectorControl,_:&str)->Result<()> {
    STATE.with(|s|if s.borrow().cancelled{Err(CrashError::Cancelled)}else{Ok(())})
}
const BASE:u64=116_444_736_001_000_000;
fn read_boot_events(_:&CollectorControl,channel:&str,provider:&str,ids:&str,fields:&[(&'static str,BootFieldType)],cap:usize)
    ->Result<(Vec<NamedBootEvent>,bool)> {
    STATE.with(|s| {let mut s=s.borrow_mut();let call=s.calls.len();s.calls.push(format!("{channel}|{ids}|{cap}"));
        if s.failed==Some(call){return Err(CrashError::Unavailable);}
        let (id,version,time,data)=match call {
          0=>(100,2,BASE+110,vec![("BootTsVersion","2".into()),("SystemBootInstance","1".into()),
            ("BootTime","25414".into()),("BootStartTime",BASE.to_string()),("BootEndTime",(BASE+100).to_string())]),
          1=>(27,if s.unsupported{2}else{1},BASE+22,vec![("BootType","0".into())]),
          2=>(101,1,BASE+120,vec![("StartTime",BASE.to_string()),("Path",r"C:\Example\app.exe".into()),
            ("TotalTime","250".into()),("DegradationTime","50".into())]),
          _=>panic!("unexpected extra Event Log query"),
        };
        for (name,_) in &data {assert!(fields.iter().any(|(requested,_)|requested==name));}
        Ok((vec![NamedBootEvent {system:RenderedEventSystem {provider:provider.into(),event_id:id,version:Some(version),
            recorded_filetime:time,recorded_unix_ms:100},fields:data.into_iter().map(|(k,v)|(k,Some(v))).collect()}],s.incomplete!=Some(call)))
    })
}
fn run(state:State)->Result<Vec<BootEvidence>> {STATE.with(|s|*s.borrow_mut()=state);collect_boots(&CollectorControl)}
#[test] fn actual_controller_uses_three_bounded_reads_then_publishes_supported_exact_evidence() {
    let boots=run(State::default()).unwrap();assert_eq!(boots.len(),1);
    assert_eq!(boots[0].raw_class.as_ref().unwrap().value,0);assert_eq!(boots[0].delays[0].degradation_time_ms,50);
    STATE.with(|s|assert_eq!(s.borrow().calls,[
        "Microsoft-Windows-Diagnostics-Performance/Operational|EventID=100|64",
        "System|EventID=27|64",
        "Microsoft-Windows-Diagnostics-Performance/Operational|EventID=101 or EventID=103|128"]));
}
#[test] fn incomplete_boot_read_cannot_vouch_for_any_unique_association() {
    let boots=run(State{incomplete:Some(0),..State::default()}).unwrap();
    assert_eq!(boots[0].boot_time_ms,Some(25414));assert!(boots[0].raw_class.is_none());assert!(boots[0].delays.is_empty());
}
#[test] fn incomplete_or_failed_kernel_query_retains_history_without_class() {
    for state in [State{incomplete:Some(1),..State::default()},State{failed:Some(1),..State::default()},
        State{unsupported:true,..State::default()}] {
        let boots=run(state).unwrap();assert_eq!(boots[0].boot_time_ms,Some(25414));assert!(boots[0].raw_class.is_none());
    }
}
#[test] fn incomplete_or_failed_delay_query_never_turns_into_attribution() {
    for state in [State{incomplete:Some(2),..State::default()},State{failed:Some(2),..State::default()}] {
        assert!(run(state).unwrap()[0].delays.is_empty());
    }
}
#[test] fn cancelled_controller_does_not_publish_even_when_boundary_returns_records() {
    assert_eq!(run(State{cancelled:true,..State::default()}),Err(CrashError::Cancelled));
}
#[test] fn primary_boot_query_failure_remains_unavailable() {
    assert_eq!(run(State{failed:Some(0),..State::default()}),Err(CrashError::Unavailable));
}
'''

class ActualBootCollection(unittest.TestCase):
    def test_actual_controller_and_parser(self):
        native=(ROOT/'crates/crash-diagnostics/src/windows_impl.rs').read_text()
        start='fn collect_boots(control: &CollectorControl) -> Result<Vec<BootEvidence>> {'
        end='\n#[derive(Clone, Copy)]\nenum BootFieldType'
        self.assertEqual(native.count(start),1)
        self.assertIn(end,native)
        body=native[native.index(start):native.index(end,native.index(start))]
        parser=(ROOT/'crates/crash-diagnostics/src/boot.rs').read_text().split('#[cfg(test)]\nmod tests')[0]
        parser=parser.replace('use serde::{Deserialize, Serialize};','').replace(', Serialize','').replace(', Deserialize','')
        parser=re.sub(r'^#\[serde\([^\n]*\)\]\n','',parser,flags=re.M)
        parser=re.sub(r'^    #\[serde\([^\n]*\)\]\n','',parser,flags=re.M)
        compiler=shutil.which('rustc')
        self.assertIsNotNone(compiler,'rustc is required; boot flow must not silently skip')
        fence='checkpoint(control, "boot.complete")?;'
        self.assertEqual(body.count(fence),1)
        controls=[('actual',body,True),('missing-completion-fence',body.replace(fence,''),False),
            ('ignored-incomplete-boot-read',body.replace('boots_complete && kernel_complete','kernel_complete')
             .replace('boots_complete && delays_complete','delays_complete'),False)]
        for label,controller,should_pass in controls:
            with self.subTest(control=label), tempfile.TemporaryDirectory(prefix='aethercore-boot-flow-') as directory:
                source=Path(directory)/'fixture.rs';binary=Path(directory)/('fixture.exe' if os.name=='nt' else 'fixture')
                source.write_text(SHIM+'\nmod boot {\n'+parser+'\n}\n'+controller)
                built=subprocess.run([compiler,'--test','--edition=2024',str(source),'-o',str(binary)],capture_output=True,text=True,timeout=60)
                self.assertEqual(built.returncode,0,built.stdout+built.stderr)
                result=subprocess.run([str(binary),'--nocapture'],capture_output=True,text=True,timeout=30)
                if should_pass:self.assertEqual(result.returncode,0,result.stdout+result.stderr)
                else:self.assertNotEqual(result.returncode,0,'negative control passed: '+label)
                print(label+':\n'+result.stdout,end='')

if __name__=='__main__':
    unittest.main()
