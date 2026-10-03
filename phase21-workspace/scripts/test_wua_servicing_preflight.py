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


if __name__ == '__main__':
    unittest.main()
