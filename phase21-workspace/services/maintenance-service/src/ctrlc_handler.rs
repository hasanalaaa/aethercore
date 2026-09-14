//! The console Ctrl+C handler, the console analogue of the SCM Stop control.

use std::sync::OnceLock;

use anyhow::Result;
use windows::Win32::System::Console::SetConsoleCtrlHandler;
use windows::core::BOOL;

static HANDLER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

unsafe extern "system" fn callback(_: u32) -> BOOL {
    if let Some(f) = HANDLER.get() {
        f();
    }
    true.into()
}

pub fn install(f: impl Fn() + Send + Sync + 'static) -> Result<()> {
    let _ = HANDLER.set(Box::new(f));
    unsafe {
        SetConsoleCtrlHandler(Some(callback), true).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    Ok(())
}
