use std::{process::Command, thread, time::{Duration, Instant}};
use aethercore_contracts::{PROTOCOL_VERSION, v1::{self, Request, RequestHeader, request, response}};
use anyhow::{Context, Result, bail};
#[cfg(windows)] use aethercore_update_engine::PlatformVerifier as _;
#[cfg(windows)] use aethercore_windows_foundation::MachineMutationGuard;
use uuid::Uuid;

fn main()->Result<()>{
    #[cfg(not(windows))] bail!("AetherCore update broker supports Windows only");
    #[cfg(windows)] return run_windows();
}

#[cfg(windows)]
fn run_windows()->Result<()>{
    let args:Vec<String>=std::env::args().collect();
    if args.len()!=5{bail!("unexpected update broker arguments")}
    let intent_id=arg(&args,"--intent-id")?;
    let locale=arg(&args,"--locale")?;
    if locale!="en"&&locale!="ar"{bail!("invalid display locale")}
    Uuid::parse_str(&intent_id).context("invalid update intent identifier")?;

    let intent=fetch_intent(&intent_id)?;
    if !confirm(&intent,&locale)?{let _=cancel_intent(&intent_id);std::process::exit(2)}
    // Defense in depth across service restart/process boundaries. MutationSupervisor::Update remains
    // the central logical authority; this is the same protected ProgramData lock used by the existing Windows mutators.
    let _machine_guard=UpdateMutationGuard::acquire().map_err(|error|{let _=cancel_intent(&intent_id);error})?;
    let ticket=claim(&intent_id)?;
    let release=ticket.release.as_ref().context("update ticket release missing")?;
    if release.release_id!=intent.release.as_ref().context("update intent release missing")?.release_id{bail!("update release identity changed")}
    let path=std::path::PathBuf::from(&ticket.staged_path);
    aethercore_update_engine::verify_file_hash_size(&path,&ticket.expected_sha256,ticket.expected_size)?;
    aethercore_update_engine::default_platform_verifier().verify_authenticode(&path)?;
    aethercore_update_engine::verify_file_hash_size(&path,&ticket.expected_sha256,ticket.expected_size)?;

    // The service, not the UI, supplied the exact staged Burn path. The broker passes no arbitrary
    // installer arguments and never downloads content itself.
    let exit_code=match Command::new(&path).status(){Ok(status)=>status.code().unwrap_or(-1),Err(_)=>-1};
    complete_with_retry(&ticket.ticket_id,exit_code)?;
    if !matches!(exit_code,0|3010){bail!("verified AetherCore installer failed with exit code {exit_code}")}
    Ok(())
}

#[cfg(windows)]
struct UpdateMutationGuard(MachineMutationGuard);
#[cfg(windows)]
impl UpdateMutationGuard{
    fn acquire()->Result<Self>{
        let guard=MachineMutationGuard::try_acquire()
            .context("acquire AetherCore machine mutation lock")?
            .ok_or_else(||anyhow::anyhow!("another Windows mutation is already active"))?;
        Ok(Self(guard))
    }
}

#[cfg(windows)]
fn fetch_intent(intent_id:&str)->Result<v1::UpdateInstallIntentResponse>{
    let response=call(request::Payload::GetUpdateInstallIntent(v1::GetUpdateInstallIntentRequest{intent_id:intent_id.into()}))?;
    match response.payload{Some(response::Payload::UpdateInstallIntent(v))if v.intent_id==intent_id=>Ok(v),_=>bail!("unexpected update intent response")}
}
#[cfg(windows)]
fn claim(intent_id:&str)->Result<v1::UpdateExecutionTicketResponse>{
    let response=call(request::Payload::ClaimUpdateInstall(v1::ClaimUpdateInstallRequest{intent_id:intent_id.into()}))?;
    match response.payload{Some(response::Payload::UpdateExecutionTicket(v))=>Ok(v),_=>bail!("unexpected update execution ticket response")}
}
#[cfg(windows)]
fn cancel_intent(intent_id:&str)->Result<()> {
    let _=call(request::Payload::CancelUpdateInstallIntent(v1::CancelUpdateInstallIntentRequest{intent_id:intent_id.into()}))?;
    Ok(())
}
#[cfg(windows)]
fn complete_with_retry(ticket_id:&str,exit_code:i32)->Result<()>{
    let deadline=Instant::now()+Duration::from_secs(90);
    loop{
        match call(request::Payload::CompleteUpdateInstall(v1::CompleteUpdateInstallRequest{ticket_id:ticket_id.into(),exit_code})){
            Ok(response)=>return match response.payload{Some(response::Payload::UpdateCompletion(v))if v.ticket_id==ticket_id=>Ok(()),_=>bail!("unexpected update completion response")},
            Err(error) if Instant::now()<deadline=>{let _=error;thread::sleep(Duration::from_millis(750));}
            Err(error)=>return Err(error.context("maintenance service did not return after update")),
        }
    }
}
#[cfg(windows)]
fn call(payload:request::Payload)->Result<v1::Response>{
    let req=Request{header:Some(RequestHeader{protocol_version:PROTOCOL_VERSION,request_id:Uuid::new_v4().to_string()}),payload:Some(payload)};
    let response=aethercore_ipc::connect(&req).context("connect to maintenance service")?;
    if response.status_code!=0{bail!("update request rejected: {}",response.error_message)}
    Ok(response)
}

#[cfg(windows)]
fn confirm(intent:&v1::UpdateInstallIntentResponse,locale:&str)->Result<bool>{
    use windows::{Win32::UI::WindowsAndMessaging::{IDYES,MB_DEFBUTTON2,MB_ICONWARNING,MB_RIGHT,MB_RTLREADING,MB_SETFOREGROUND,MB_YESNO,MessageBoxW},core::PCWSTR};
    let release=intent.release.as_ref().context("update release missing")?;
    let (title,body)=if locale=="ar"{("AetherCore — تثبيت تحديث موثوق".to_string(),format!("تم تنزيل تحديث AetherCore والتحقق منه محليًا.\n\nالإصدار: {}\nالحجم: {} ميغابايت\n\nسيشغّل وسيط التحديث حزمة التثبيت الموقعة التي أعدتها خدمة AetherCore فقط، وقد يعيد Windows تشغيل خدمة الصيانة أثناء الترقية. هل تريد المتابعة؟",release.version,release.size_bytes/1_048_576))}else{("AetherCore — Install Verified Update".to_string(),format!("AetherCore downloaded and locally verified this update.\n\nVersion: {}\nSize: {} MB\n\nThe update broker will run only the signed installer staged by the AetherCore service. Windows may restart the maintenance service during the upgrade. Continue?",release.version,release.size_bytes/1_048_576))};
    let t=wide(&title);let b=wide(&body);let base=MB_YESNO|MB_ICONWARNING|MB_DEFBUTTON2|MB_SETFOREGROUND;let flags=if locale=="ar"{base|MB_RIGHT|MB_RTLREADING}else{base};
    Ok(unsafe{MessageBoxW(None,PCWSTR(b.as_ptr()),PCWSTR(t.as_ptr()),flags)}==IDYES)
}
fn arg(args:&[String],name:&str)->Result<String>{let index=args.iter().position(|v|v==name).with_context(||format!("missing {name}"))?;args.get(index+1).cloned().with_context(||format!("missing value for {name}"))}
#[cfg(windows)] fn wide(value:&str)->Vec<u16>{value.encode_utf16().chain(std::iter::once(0)).collect()}
