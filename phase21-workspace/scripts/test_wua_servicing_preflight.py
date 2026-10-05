#!/usr/bin/env python3
"""Run the actual WUA servicing preflight with controlled, read-only COM responses.

Only the COM boundary is replaced. The production function is compiled unchanged
with rustc and std, so these controls require neither Windows nor a servicing call.
Native compilation still verifies the real Windows bindings separately.
"""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'crates/windows-update/src/execution_windows.rs'

SHIM = r'''
#![allow(non_snake_case, unused_unsafe, dead_code)]
use std::cell::RefCell;
#[derive(Debug, PartialEq)] pub enum UpdateError { Busy, RebootPending, Wua(String) }
type Result<T> = std::result::Result<T, UpdateError>;
#[derive(Debug)] struct Error(i32);
fn wua_err(e: Error) -> UpdateError { UpdateError::Wua(format!("HRESULT {}", e.0)) }
struct ComApartment;
impl ComApartment { fn mta() -> std::result::Result<Self, Error> { Ok(Self) } }
struct BSTR;
impl BSTR { fn from(_: &str) -> Self { Self } }
const CLSCTX_INPROC_SERVER: u32 = 1;
const UpdateSession: u32 = 1;
const SystemInformation: u32 = 2;
struct Bool(bool);
impl Bool { fn as_bool(self) -> bool { self.0 } }
#[derive(Clone)] struct State {
    busy: std::result::Result<bool, i32>,
    general: std::result::Result<bool, i32>,
    preinstall: std::result::Result<bool, i32>,
    calls: Vec<&'static str>,
}
impl Default for State {
    fn default() -> Self { Self { busy: Ok(false), general: Ok(false),
        preinstall: Ok(false), calls: vec![] } }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn read(name: &'static str) -> std::result::Result<Bool, Error> {
    STATE.with(|s| { let mut s = s.borrow_mut(); s.calls.push(name);
        match name { "busy" => s.busy, "general" => s.general,
            "preinstall" => s.preinstall, _ => unreachable!() }.map(Bool).map_err(Error)
    })
}
trait Create { fn create(class: &u32) -> std::result::Result<Self, Error> where Self: Sized; }
unsafe fn CoCreateInstance<T: Create>(class: &u32, _: Option<()>, _: u32)
    -> std::result::Result<T, Error> { T::create(class) }
struct IUpdateSession;
impl Create for IUpdateSession {
    fn create(class: &u32) -> std::result::Result<Self, Error> {
        assert_eq!(*class, UpdateSession); Ok(Self)
    }
}
struct Installer;
impl IUpdateSession {
    unsafe fn SetClientApplicationID(&self, _: &BSTR) -> std::result::Result<(), Error> { Ok(()) }
    unsafe fn CreateUpdateInstaller(&self) -> std::result::Result<Installer, Error> { Ok(Installer) }
}
impl Installer {
    unsafe fn SetClientApplicationID(&self, _: &BSTR) -> std::result::Result<(), Error> { Ok(()) }
    unsafe fn IsBusy(&self) -> std::result::Result<Bool, Error> { read("busy") }
    unsafe fn RebootRequiredBeforeInstallation(&self) -> std::result::Result<Bool, Error> { read("preinstall") }
}
struct ISystemInformation;
impl Create for ISystemInformation {
    fn create(class: &u32) -> std::result::Result<Self, Error> {
        assert_eq!(*class, SystemInformation); Ok(Self)
    }
}
impl ISystemInformation {
    unsafe fn RebootRequired(&self) -> std::result::Result<Bool, Error> { read("general") }
}
fn run(state: State) -> (Result<()>, Vec<&'static str>) {
    STATE.with(|s| *s.borrow_mut() = state);
    let result = ensure_servicing_available();
    (result, STATE.with(|s| s.borrow().calls.clone()))
}
#[test] fn general_reboot_without_installer_reboot_is_pending() {
    let (result, calls) = run(State { general: Ok(true), ..State::default() });
    assert_eq!(result, Err(UpdateError::RebootPending));
    assert_eq!(calls, ["busy", "general"]);
}
#[test] fn failed_general_reboot_query_is_not_available() {
    let (result, calls) = run(State { general: Err(-5), ..State::default() });
    assert_eq!(result, Err(UpdateError::Wua("HRESULT -5".into())));
    assert_eq!(calls, ["busy", "general"]);
}
#[test] fn available_requires_both_reboot_reads_to_be_false() {
    let (result, calls) = run(State::default());
    assert_eq!(result, Ok(()));
    assert_eq!(calls, ["busy", "general", "preinstall"]);
}
#[test] fn busy_prevents_subsequent_reboot_reads() {
    let (result, calls) = run(State { busy: Ok(true), ..State::default() });
    assert_eq!(result, Err(UpdateError::Busy)); assert_eq!(calls, ["busy"]);
}
#[test] fn failed_busy_query_remains_unknown() {
    let (result, calls) = run(State { busy: Err(-7), ..State::default() });
    assert_eq!(result, Err(UpdateError::Wua("HRESULT -7".into())));
    assert_eq!(calls, ["busy"]);
}
#[test] fn installer_reboot_remains_pending() {
    assert_eq!(run(State { preinstall: Ok(true), ..State::default() }).0,
        Err(UpdateError::RebootPending));
}
#[test] fn failed_installer_reboot_query_remains_unknown() {
    assert_eq!(run(State { preinstall: Err(-9), ..State::default() }).0,
        Err(UpdateError::Wua("HRESULT -9".into())));
}
'''


class ActualServicingPreflight(unittest.TestCase):
    def test_controlled_com_responses(self):
        text = SOURCE.read_text(encoding='utf-8')
        start = 'pub fn ensure_servicing_available() -> Result<()> {'
        end = '\npub fn execute_driver_updates'
        self.assertEqual(text.count(start), 1)
        function = text[text.index(start):text.index(end, text.index(start))]
        compiler = shutil.which('rustc')
        self.assertIsNotNone(compiler, 'rustc is required; this control must not silently skip')
        with tempfile.TemporaryDirectory(prefix='aethercore-wua-preflight-') as directory:
            source = Path(directory) / 'fixture.rs'
            binary = Path(directory) / ('fixture.exe' if os.name == 'nt' else 'fixture')
            source.write_text(SHIM + '\n' + function, encoding='utf-8')
            built = subprocess.run([compiler, '--test', '--edition=2024', str(source), '-o', str(binary)],
                                   capture_output=True, text=True, timeout=60)
            self.assertEqual(built.returncode, 0, built.stdout + built.stderr)
            result = subprocess.run([str(binary), '--nocapture'], capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            print(result.stdout, end='')


class ActualClientEventCollection(unittest.TestCase):
    def test_two_channel_coverage_and_shared_budget(self):
        native = (ROOT / 'crates/windows-update/src/client_events_windows.rs').read_text()
        start = 'pub fn query_client_errors(max_entries: usize) -> Result<ClientErrors> {'
        end = '\nfn query_channel('
        self.assertEqual(native.count(start), 1)
        function = native[native.index(start):native.index(end, native.index(start))]
        shim = r'''
use std::{cell::RefCell, time::{Duration, Instant}};
#[derive(Debug,Default)] struct ClientErrors {
    errors:Vec<u32>, reboot_notifications:Vec<i64>, unknown_events:u32,
    truncated:bool, unavailable_channels:u32,
}
type Result<T> = std::result::Result<T,()>;
#[derive(Default)] struct State { failed:[bool;2], calls:Vec<(String,usize,Instant)> }
thread_local! {static STATE:RefCell<State> = RefCell::new(State::default());}
fn query_channel(channel:&str,filter:&str,cap:usize,deadline:Instant)->Result<ClientErrors> {
    STATE.with(|s| {let mut s=s.borrow_mut();let index=s.calls.len();
        assert!(index<2, "unexpected extra channel query");
        assert!(deadline.saturating_duration_since(Instant::now()) <= Duration::from_secs(5));
        assert_eq!(filter, if index==0 {"Level=2"} else {"(Level=2 or EventID=21)"});
        s.calls.push((channel.into(),cap,deadline));
        if s.failed[index] {return Err(());}
        Ok(ClientErrors {errors:vec![index as u32], reboot_notifications:vec![10,10],
            unknown_events:index as u32, truncated:index==1, ..Default::default()})
    })
}
fn run(failed:[bool;2],cap:usize)->ClientErrors {
    STATE.with(|s|*s.borrow_mut()=State{failed,..Default::default()});
    query_client_errors(cap).unwrap()
}
#[test] fn both_channels_share_one_deadline_and_total_cap_never_exceeds_200() {
    for cap in [0,1,199,200,usize::MAX] {
        run([false,false],cap);
        STATE.with(|s|{let s=s.borrow();assert_eq!(s.calls.len(),2);
            assert_eq!(s.calls[0].0,"Microsoft-Windows-WindowsUpdateClient/Operational");
            assert_eq!(s.calls[1].0,"System");assert_eq!(s.calls[0].2,s.calls[1].2);
            assert_eq!(s.calls[0].1+s.calls[1].1,cap.min(200));});
    }
}
#[test] fn a_missing_channel_retains_other_evidence_but_never_complete_coverage() {
    for failed in [[true,false],[false,true],[true,true]] {
        let out=run(failed,200);assert_eq!(out.unavailable_channels, failed.into_iter().filter(|b|*b).count() as u32);
        assert_eq!(out.errors.len(),failed.into_iter().filter(|b|!*b).count());
    }
}
#[test] fn historical_notifications_do_not_enter_error_counts_or_duplicate() {
    let out=run([false,false],200);assert_eq!(out.errors,[0,1]);
    assert_eq!(out.reboot_notifications,[10]);assert_eq!(out.unknown_events,1);assert!(out.truncated);
}
'''
        compiler = shutil.which('rustc')
        self.assertIsNotNone(compiler, 'rustc is required; client evidence must not silently skip')
        system = '("System", cap / 2, "(Level=2 or EventID=21)"),'
        missing = 'Err(_) => output.unavailable_channels += 1,'
        self.assertEqual(function.count(system), 1)
        self.assertEqual(function.count(missing), 1)
        for label, controller, passing in [
            ('actual-client-events', function, True),
            ('missing-system-channel', function.replace(system, ''), False),
            ('lost-unavailable-coverage', function.replace(missing, 'Err(_) => {},'), False),
        ]:
            with self.subTest(control=label), tempfile.TemporaryDirectory(prefix='aethercore-wua-events-') as directory:
                source = Path(directory) / 'fixture.rs'
                binary = Path(directory) / ('fixture.exe' if os.name == 'nt' else 'fixture')
                source.write_text(shim + '\n' + controller)
                built = subprocess.run([compiler, '--test', '--edition=2024', str(source), '-o', str(binary)],
                                       capture_output=True, text=True, timeout=60)
                self.assertEqual(built.returncode, 0, built.stdout + built.stderr)
                result = subprocess.run([str(binary), '--nocapture'], capture_output=True, text=True, timeout=30)
                self.assertEqual(result.returncode == 0, passing, result.stdout + result.stderr)
                print(label + ':\n' + result.stdout, end='')


if __name__ == '__main__':
    unittest.main()
