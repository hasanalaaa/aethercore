use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    io::Read,
    os::windows::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use aethercore_operation_engine::StartupChangeAction;
use aethercore_windows_foundation::{ComApartment, MachineMutationGuard, OwnedServiceHandle};
use sha2::{Digest, Sha256};
use windows::{
    core::{BSTR, BOOL, GUID, PCWSTR, PWSTR},
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS},
        Storage::FileSystem::{
            MoveFileExW, FILE_ATTRIBUTE_REPARSE_POINT, MOVEFILE_COPY_ALLOWED,
            MOVEFILE_WRITE_THROUGH,
        },
        System::{
            Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
            Registry::{
                RegCloseKey, RegDeleteValueW, RegEnumKeyExW, RegEnumValueW, RegOpenKeyExW,
                RegQueryValueExW, RegSetValueExW, HKEY, HKEY_LOCAL_MACHINE, HKEY_USERS,
                KEY_QUERY_VALUE, KEY_READ, KEY_SET_VALUE, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
                REG_EXPAND_SZ, REG_MULTI_SZ, REG_SAM_FLAGS, REG_SZ, REG_VALUE_TYPE,
            },
            Services::{
                ChangeServiceConfig2W, ChangeServiceConfigW, OpenSCManagerW,
                OpenServiceW, SERVICE_CHANGE_CONFIG, SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
                SERVICE_DELAYED_AUTO_START_INFO, SERVICE_DEMAND_START, SERVICE_ERROR,
                SERVICE_NO_CHANGE, SERVICE_QUERY_CONFIG, SERVICE_START_TYPE,
            },
            TaskScheduler::{IRegisteredTask, ITaskFolder, ITaskService},
            Variant::VARIANT,
        },
    },
};

use super::{NativeState, Result, StartupError, StartupItem, StartupPlatform};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUNONCE_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\RunOnce";
const SERVICES_KEY: &str = r"SYSTEM\CurrentControlSet\Services";
const MAX_STARTUP_FILE_EVIDENCE_BYTES: u64 = 64 * 1024 * 1024;
const CLSID_TASK_SCHEDULER: GUID = GUID::from_u128(0x0f87369f_a4e5_4cfc_bd3e_73e6154572dd);

pub struct WindowsStartupPlatform;
pub(crate) struct StartupMutationGuard(MachineMutationGuard);
pub(crate) fn acquire_mutation_guard()->Result<StartupMutationGuard>{let guard=MachineMutationGuard::try_acquire().map_err(|e|StartupError::Platform(e.to_string()))?.ok_or(StartupError::MutationBusy)?;Ok(StartupMutationGuard(guard))}

impl StartupPlatform for WindowsStartupPlatform {
    fn scan(&self)->Result<(Vec<StartupItem>,Vec<String>)>{
        let mut items=Vec::new();let mut warnings=Vec::new();
        scan_registry_startup(&mut items,&mut warnings)?;
        scan_startup_folders(&mut items,&mut warnings)?;
        if let Err(e)=scan_scheduled_tasks(&mut items){warnings.push(format!("Task Scheduler inventory unavailable: {e}"));}
        if let Err(e)=scan_services(&mut items){warnings.push(format!("Service inventory unavailable: {e}"));}
        items.sort_by(|a,b|a.kind.cmp(&b.kind).then_with(||a.display_name.to_ascii_lowercase().cmp(&b.display_name.to_ascii_lowercase())));
        Ok((items,warnings))
    }

    fn current_state(&self,action:&StartupChangeAction)->Result<String>{
        let template:NativeState=serde_json::from_str(&action.original_state_json)?;
        let state=match template {
            NativeState::RegistryValue{hive,key,value_name,view,value_type,data_hex,..}=>query_registry_state(&hive,&key,&value_name,&view,value_type,&data_hex)?,
            NativeState::StartupFile{path,size_bytes,modified_unix_ms,sha256,backup_path,..}=>query_startup_file_state(&path,size_bytes,modified_unix_ms,&sha256,&backup_path)?,
            NativeState::ScheduledTask{task_path,xml_sha256,..}=>query_task_state(&task_path,&xml_sha256)?,
            NativeState::Service{service_name,..}=>query_service_state(&service_name)?,
        };
        serde_json::to_string(&state).map_err(Into::into)
    }

    fn apply(&self,action:&StartupChangeAction)->Result<()> {
        let original:NativeState=serde_json::from_str(&action.original_state_json)?;
        let applied:NativeState=serde_json::from_str(&action.applied_state_json)?;
        let restoring=action.direction=="Restore";
        match (original,applied) {
            (NativeState::RegistryValue{hive,key,value_name,view,value_type,data_hex,..},NativeState::RegistryValue{..})=>{
                if restoring{set_registry_value(&hive,&key,&value_name,&view,value_type,&data_hex)}else{delete_registry_value(&hive,&key,&value_name,&view)}
            }
            (NativeState::StartupFile{path,..},NativeState::StartupFile{backup_path,..})=>{
                if restoring{move_file(&backup_path,&path)}else{move_file(&path,&backup_path)}
            }
            (NativeState::ScheduledTask{task_path,..},NativeState::ScheduledTask{enabled,..})=>set_task_enabled(&task_path,if restoring{true}else{enabled}),
            (NativeState::Service{service_name,start_type,delayed_auto,..},NativeState::Service{start_type:disabled_type,delayed_auto:disabled_delayed,..})=>{
                if restoring{set_service_start(&service_name,start_type,delayed_auto)}else{set_service_start(&service_name,disabled_type,disabled_delayed)}
            }
            _=>Err(StartupError::Platform("startup state type mismatch".into())),
        }
    }
}

fn scan_registry_startup(out:&mut Vec<StartupItem>,warnings:&mut Vec<String>)->Result<()> {
    for (key,kind) in [(RUN_KEY,"RegistryRun"),(RUNONCE_KEY,"RegistryRunOnce")] {
        for (view,label) in [(KEY_WOW64_64KEY,"64"),(KEY_WOW64_32KEY,"32")] {
            if let Err(e)=enum_registry_values(HKEY_LOCAL_MACHINE,"HKLM",key,view,label,"Machine",kind,out){warnings.push(format!("HKLM {key} ({label}-bit) unavailable: {e}"));}
        }
    }
    // LocalSystem has a different HKCU. Enumerate only already-loaded real user hives under HKEY_USERS.
    for sid in enum_subkeys(HKEY_USERS,"")? {
        if !sid.starts_with("S-1-5-21-"){continue}
        for (key,kind) in [(RUN_KEY,"RegistryRun"),(RUNONCE_KEY,"RegistryRunOnce")] {
            let sub=format!(r"{}\{}",sid,key);
            for (view,label) in [(KEY_WOW64_64KEY,"64"),(KEY_WOW64_32KEY,"32")] {
                if let Err(e)=enum_registry_values(HKEY_USERS,&format!("HKU\\{sid}"),&sub,view,label,"User",kind,out){warnings.push(format!("Loaded user startup hive {sid} ({label}-bit) unavailable: {e}"));}
            }
        }
    }
    Ok(())
}

fn enum_registry_values(root:HKEY,hive_label:&str,key:&str,view:REG_SAM_FLAGS,view_label:&str,scope:&str,kind:&str,out:&mut Vec<StartupItem>)->Result<()> {
    let logical_key=if let Some(sid)=hive_label.strip_prefix("HKU\\"){key.strip_prefix(&format!("{}\\",sid)).unwrap_or(key)}else{key};
    let h=open_key(root,key,KEY_QUERY_VALUE|view)?;let mut index=0u32;
    loop {
        let mut name=vec![0u16;16_384];let mut name_len=(name.len()-1) as u32;let mut ty=0u32;let mut data=vec![0u8;64*1024];let mut data_len=data.len() as u32;
        let rc=unsafe{RegEnumValueW(h.0,index,Some(PWSTR(name.as_mut_ptr())),&mut name_len,None,Some(&mut ty),Some(data.as_mut_ptr()),Some(&mut data_len))};
        if rc==ERROR_NO_MORE_ITEMS{break} if rc!=ERROR_SUCCESS{return Err(StartupError::Platform(format!("RegEnumValueW error {}",rc.0)))}
        index+=1;name.truncate(name_len as usize);data.truncate(data_len as usize);let value_name=String::from_utf16_lossy(&name);
        let supported=ty==REG_SZ.0||ty==REG_EXPAND_SZ.0;let command=if supported{decode_utf16_bytes(&data)}else{format!("Registry value type {ty}")};
        let systemish=is_security_or_system_command(&command);let protected=systemish||!supported;
        let reason=if !supported{"Only REG_SZ/REG_EXPAND_SZ startup values are reversible in Phase 5."}else if systemish{"Windows/security startup targets are protected."}else{""};
        let state=NativeState::RegistryValue{hive:hive_label.into(),key:logical_key.into(),value_name:value_name.clone(),view:view_label.into(),exists:true,value_type:ty,data_hex:hex::encode(&data)};
        out.push(StartupItem{item_id:stable_id(&format!("reg|{hive_label}|{logical_key}|{view_label}|{value_name}")),kind:kind.into(),scope:scope.into(),display_name:value_name.clone(),publisher:if systemish{"Windows / security".into()}else{"Third-party / unknown".into()},command:command.clone(),source:format!("{hive_label}\\{logical_key} [{view_label}] · {value_name}"),enabled:true,manageable:!protected,protected,protection_reason:reason.into(),impact:"Unknown".into(),confidence:"InsufficientEvidence".into(),evidence_detail:"Runs at user logon; no directly correlated boot-duration evidence was found by this inventory provider.".into(),recommendation:if protected{"Keep enabled".into()}else{"Review".into()},service_change:false,original_state:Some(state)});
    } Ok(())
}

fn scan_startup_folders(out:&mut Vec<StartupItem>,warnings:&mut Vec<String>)->Result<()> {
    let program_data=PathBuf::from(std::env::var_os("ProgramData").unwrap_or_else(||OsStr::new(r"C:\ProgramData").to_os_string()));
    scan_folder(&program_data.join(r"Microsoft\Windows\Start Menu\Programs\Startup"),"Machine",out,warnings)?;
    let system_drive=std::env::var("SystemDrive").unwrap_or_else(|_|"C:".into());let users=PathBuf::from(format!(r"{}\Users",system_drive));
    if let Ok(entries)=std::fs::read_dir(users){for e in entries.flatten(){let name=e.file_name().to_string_lossy().to_string();if matches!(name.to_ascii_lowercase().as_str(),"public"|"default"|"default user"|"all users"){continue}scan_folder(&e.path().join(r"AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup"),&format!("User: {name}"),out,warnings)?;}}
    Ok(())
}
fn scan_folder(path:&Path,scope:&str,out:&mut Vec<StartupItem>,warnings:&mut Vec<String>)->Result<()> {
    let Ok(entries)=std::fs::read_dir(path) else{return Ok(())};
    for e in entries.flatten(){let p=e.path();let Ok(meta)=std::fs::symlink_metadata(&p) else{continue};if !meta.is_file(){continue}if meta.file_type().is_symlink() || (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0) != 0 {warnings.push(format!("Skipped reparse/symlink startup entry: {}",p.display()));continue}
        let modified=meta.modified().ok().and_then(|t|t.duration_since(UNIX_EPOCH).ok()).map(|d|d.as_millis() as i64).unwrap_or(0);let name=e.file_name().to_string_lossy().to_string();let systemish=is_security_or_system_command(&p.to_string_lossy());let oversized=meta.len()>MAX_STARTUP_FILE_EVIDENCE_BYTES;let protected=systemish||oversized;
        let digest=if oversized{String::new()}else{file_sha256(&p)?};
        let state=NativeState::StartupFile{path:p.to_string_lossy().into_owned(),exists:true,size_bytes:meta.len(),modified_unix_ms:modified,sha256:digest,backup_path:String::new(),backup_exists:false};
        let reason=if systemish{"Windows/security startup targets are protected."}else if oversized{"Startup entry exceeds the Phase 5 evidence-size budget and is therefore observation-only."}else{""};
        out.push(StartupItem{item_id:stable_id(&format!("folder|{}",p.to_string_lossy().to_ascii_lowercase())),kind:"StartupFolder".into(),scope:scope.into(),display_name:name,publisher:if systemish{"Windows / security".into()}else{"Third-party / unknown".into()},command:p.to_string_lossy().into_owned(),source:p.to_string_lossy().into_owned(),enabled:true,manageable:!protected,protected,protection_reason:reason.into(),impact:"Unknown".into(),confidence:"InsufficientEvidence".into(),evidence_detail:"Present in a Startup folder; no directly correlated duration evidence is asserted.".into(),recommendation:if protected{"Keep enabled".into()}else{"Review".into()},service_change:false,original_state:Some(state)});
    } Ok(())
}

fn scan_scheduled_tasks(out:&mut Vec<StartupItem>)->Result<()> {
    with_task_service(|service|unsafe{let root=service.GetFolder(&BSTR::from("\\")).map_err(win)?;scan_task_folder(&root,out)})
}
unsafe fn scan_task_folder(folder:&ITaskFolder,out:&mut Vec<StartupItem>)->Result<()> {
    let tasks=unsafe{folder.GetTasks(1)}.map_err(win)?;let count=unsafe{tasks.Count()}.map_err(win)?;
    for i in 1..=count {let task=unsafe{tasks.get_Item(&VARIANT::from(i))}.map_err(win)?;let enabled=unsafe{task.Enabled()}.map_err(win)?.0!=0;if !enabled{continue}let xml=unsafe{task.Xml()}.map_err(win)?.to_string();if !xml.contains("<LogonTrigger")&&!xml.contains("<BootTrigger"){continue}let path=unsafe{task.Path()}.map_err(win)?.to_string();let name=unsafe{task.Name()}.map_err(win)?.to_string();let protected=path.to_ascii_lowercase().starts_with(r"\microsoft\windows\")||is_security_or_system_command(&xml);let digest=task_definition_hash(&xml);let command=extract_xml_tag(&xml,"Command").unwrap_or_else(||"Task Scheduler action".into());let state=NativeState::ScheduledTask{task_path:path.clone(),enabled:true,xml_sha256:digest};out.push(StartupItem{item_id:stable_id(&format!("task|{}",path.to_ascii_lowercase())),kind:"ScheduledTask".into(),scope:"Scheduled task".into(),display_name:name,publisher:if protected{"Windows / security".into()}else{"Third-party / unknown".into()},command,source:path.clone(),enabled:true,manageable:!protected,protected,protection_reason:if protected{"Microsoft Windows or security-related scheduled tasks are protected.".into()}else{String::new()},impact:"Unknown".into(),confidence:"InsufficientEvidence".into(),evidence_detail:"Task has a boot/logon trigger; no undocumented duration fields are converted into a performance claim.".into(),recommendation:if protected{"Keep enabled".into()}else{"Review".into()},service_change:false,original_state:Some(state)});}
    let folders=unsafe{folder.GetFolders(0)}.map_err(win)?;let fcount=unsafe{folders.Count()}.map_err(win)?;for i in 1..=fcount{let child=unsafe{folders.get_Item(&VARIANT::from(i))}.map_err(win)?;unsafe{scan_task_folder(&child,out)?;}} Ok(())
}

fn scan_services(out:&mut Vec<StartupItem>)->Result<()> {
    let services=enum_subkeys(HKEY_LOCAL_MACHINE,SERVICES_KEY)?;let mut configs=HashMap::<String,(u32,u32,String,bool,u32,Vec<String>)>::new();
    for name in services {let sub=format!(r"{}\{}",SERVICES_KEY,name);let start=query_dword(HKEY_LOCAL_MACHINE,&sub,"Start").unwrap_or(4);let ty=query_dword(HKEY_LOCAL_MACHINE,&sub,"Type").unwrap_or(0);let image=query_string(HKEY_LOCAL_MACHINE,&sub,"ImagePath").unwrap_or_default();let delayed=query_dword(HKEY_LOCAL_MACHINE,&sub,"DelayedAutoStart").unwrap_or(0)!=0;let launch=query_dword(HKEY_LOCAL_MACHINE,&sub,"LaunchProtected").unwrap_or(0);let deps=query_multi_string(HKEY_LOCAL_MACHINE,&sub,"DependOnService").unwrap_or_default();configs.insert(name,(start,ty,image,delayed,launch,deps));}
    let mut depended=HashSet::new();for(_,(_,_,_,_,_,deps))in &configs{for d in deps{depended.insert(d.to_ascii_lowercase());}}
    let system_root=std::env::var("SystemRoot").unwrap_or_else(|_|r"C:\Windows".into()).to_ascii_lowercase();
    for(name,(start,ty,image,delayed,launch,_)) in configs {if start!=2{continue}let lname=name.to_ascii_lowercase();let image_lower=image.to_ascii_lowercase();let own_process=(ty&0x10)!=0&&(ty&0x20)==0;let essential=is_essential_service(&lname);let windows_binary=image_lower.contains(&system_root)||image_lower.contains("\\system32\\")||image_lower.contains("svchost.exe");let security=is_security_or_system_command(&format!("{name} {image}"));let protected_role=is_protected_service_role(&format!("{name} {image}"));let is_depended=depended.contains(&lname);let protected=launch!=0||essential||windows_binary||security||protected_role||is_depended||!own_process;let reason=if launch!=0{"Windows launch-protected service."}else if essential{"Essential Windows service policy."}else if windows_binary{"Windows-hosted service."}else if security{"Security-related service policy."}else if protected_role{"Networking, storage, input, accessibility, or other protected service role."}else if is_depended{"Another service declares a dependency on this service."}else if !own_process{"Shared/driver service types are not managed."}else{""};let state=NativeState::Service{service_name:name.clone(),start_type:start,delayed_auto:delayed,service_type:ty,binary_path:image.clone(),launch_protected:launch};out.push(StartupItem{item_id:stable_id(&format!("service|{lname}")),kind:"Service".into(),scope:"Machine service".into(),display_name:name.clone(),publisher:if protected&&windows_binary{"Windows".into()}else{"Third-party / unknown".into()},command:image.clone(),source:format!("Service: {name}"),enabled:true,manageable:!protected,protected,protection_reason:reason.into(),impact:"Unknown".into(),confidence:"InsufficientEvidence".into(),evidence_detail:if delayed{"Automatic (delayed) service; exact boot impact is not inferred without direct telemetry.".into()}else{"Automatic service; exact boot impact is not inferred without direct telemetry.".into()},recommendation:if protected{"Keep enabled".into()}else{"Review".into()},service_change:true,original_state:Some(state)});}
    Ok(())
}

fn query_registry_state(hive:&str,key:&str,value_name:&str,view:&str,expected_type:u32,expected_hex:&str)->Result<NativeState>{let(root,key_path)=parse_hive(hive,key);let view_flag=match view{"64"=>KEY_WOW64_64KEY,"32"=>KEY_WOW64_32KEY,_=>REG_SAM_FLAGS(0)};match query_raw_value(root,&key_path,value_name,view_flag){Ok((ty,data))=>Ok(NativeState::RegistryValue{hive:hive.into(),key:key.into(),value_name:value_name.into(),view:view.into(),exists:true,value_type:ty,data_hex:hex::encode(data)}),Err(StartupError::Platform(e))if e=="not-found"=>Ok(NativeState::RegistryValue{hive:hive.into(),key:key.into(),value_name:value_name.into(),view:view.into(),exists:false,value_type:expected_type,data_hex:expected_hex.into()}),Err(e)=>Err(e)}}
fn query_startup_file_state(path:&str,size:u64,modified:i64,expected_sha256:&str,backup:&str)->Result<NativeState>{
    let p=Path::new(path);let b=Path::new(backup);
    if let Ok(m)=std::fs::symlink_metadata(p){
        if m.file_type().is_symlink() || (m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0) != 0 {return Err(StartupError::Platform("startup target became a reparse/symlink".into()))}
        if m.len()>MAX_STARTUP_FILE_EVIDENCE_BYTES{return Err(StartupError::Platform("startup target exceeds evidence-size budget".into()))}
        return Ok(NativeState::StartupFile{path:path.into(),exists:true,size_bytes:m.len(),modified_unix_ms:modified_ms(&m),sha256:file_sha256(p)?,backup_path:backup.into(),backup_exists:!backup.is_empty()&&b.is_file()});
    }
    if !backup.is_empty(){
        if let Ok(m)=std::fs::symlink_metadata(b){
            if m.file_type().is_symlink() || (m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0) != 0 {return Err(StartupError::Platform("startup backup became a reparse/symlink".into()))}
            if m.len()>MAX_STARTUP_FILE_EVIDENCE_BYTES{return Err(StartupError::Platform("startup backup exceeds evidence-size budget".into()))}
            return Ok(NativeState::StartupFile{path:path.into(),exists:false,size_bytes:m.len(),modified_unix_ms:modified_ms(&m),sha256:file_sha256(b)?,backup_path:backup.into(),backup_exists:true});
        }
    }
    Ok(NativeState::StartupFile{path:path.into(),exists:false,size_bytes:size,modified_unix_ms:modified,sha256:expected_sha256.into(),backup_path:backup.into(),backup_exists:false})
}
fn query_task_state(path:&str,_digest:&str)->Result<NativeState>{with_task_service(|service|unsafe{let root=service.GetFolder(&BSTR::from("\\")).map_err(win)?;let task=root.GetTask(&BSTR::from(path)).map_err(win)?;let xml=task.Xml().map_err(win)?.to_string();let enabled=task.Enabled().map_err(win)?.0!=0;Ok(NativeState::ScheduledTask{task_path:path.into(),enabled,xml_sha256:task_definition_hash(&xml)})})}
fn query_service_state(name:&str)->Result<NativeState>{let sub=format!(r"{}\{}",SERVICES_KEY,name);Ok(NativeState::Service{service_name:name.into(),start_type:query_dword(HKEY_LOCAL_MACHINE,&sub,"Start")?,delayed_auto:query_dword(HKEY_LOCAL_MACHINE,&sub,"DelayedAutoStart").unwrap_or(0)!=0,service_type:query_dword(HKEY_LOCAL_MACHINE,&sub,"Type")?,binary_path:query_string(HKEY_LOCAL_MACHINE,&sub,"ImagePath").unwrap_or_default(),launch_protected:query_dword(HKEY_LOCAL_MACHINE,&sub,"LaunchProtected").unwrap_or(0)})}

fn delete_registry_value(hive:&str,key:&str,name:&str,view:&str)->Result<()>{let(root,path)=parse_hive(hive,key);let vf=match view{"64"=>KEY_WOW64_64KEY,"32"=>KEY_WOW64_32KEY,_=>REG_SAM_FLAGS(0)};let h=open_key(root,&path,KEY_SET_VALUE|vf)?;let name_w=wide(name);let rc=unsafe{RegDeleteValueW(h.0,PCWSTR(name_w.as_ptr()))};if rc!=ERROR_SUCCESS{return Err(StartupError::Platform(format!("RegDeleteValueW error {}",rc.0)))}Ok(())}
fn set_registry_value(hive:&str,key:&str,name:&str,view:&str,ty:u32,data_hex:&str)->Result<()>{let(root,path)=parse_hive(hive,key);let vf=match view{"64"=>KEY_WOW64_64KEY,"32"=>KEY_WOW64_32KEY,_=>REG_SAM_FLAGS(0)};let h=open_key(root,&path,KEY_SET_VALUE|vf)?;let data=hex::decode(data_hex).map_err(|e|StartupError::Platform(e.to_string()))?;let name_w=wide(name);let rc=unsafe{RegSetValueExW(h.0,PCWSTR(name_w.as_ptr()),None,REG_VALUE_TYPE(ty),Some(&data))};if rc!=ERROR_SUCCESS{return Err(StartupError::Platform(format!("RegSetValueExW error {}",rc.0)))}Ok(())}
fn move_file(from:&str,to:&str)->Result<()>{let dst=Path::new(to);if let Some(parent)=dst.parent(){std::fs::create_dir_all(parent).map_err(|e|StartupError::Platform(e.to_string()))?;}let from_w=wide(from);let to_w=wide(to);unsafe{MoveFileExW(PCWSTR(from_w.as_ptr()),PCWSTR(to_w.as_ptr()),MOVEFILE_COPY_ALLOWED|MOVEFILE_WRITE_THROUGH).map_err(win)} }
fn set_task_enabled(path:&str,enabled:bool)->Result<()>{with_task_service(|service|unsafe{let root=service.GetFolder(&BSTR::from("\\")).map_err(win)?;let task=root.GetTask(&BSTR::from(path)).map_err(win)?;task.SetEnabled(windows::Win32::Foundation::VARIANT_BOOL(if enabled{-1}else{0})).map_err(win)})}
fn set_service_start(name:&str,start:u32,delayed:bool)->Result<()> {unsafe{let scm=OwnedServiceHandle::new(OpenSCManagerW(PCWSTR::null(),PCWSTR::null(),0x0001).map_err(win)?);let name_w=wide(name);let svc=OwnedServiceHandle::new(OpenServiceW(scm.get(),PCWSTR(name_w.as_ptr()),SERVICE_CHANGE_CONFIG|SERVICE_QUERY_CONFIG).map_err(win)?);ChangeServiceConfigW(svc.get(),windows::Win32::System::Services::ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),SERVICE_START_TYPE(start),SERVICE_ERROR(SERVICE_NO_CHANGE),PCWSTR::null(),PCWSTR::null(),None,PCWSTR::null(),PCWSTR::null(),PCWSTR::null(),PCWSTR::null()).map_err(win)?;let info=SERVICE_DELAYED_AUTO_START_INFO{fDelayedAutostart:BOOL::from(delayed)};ChangeServiceConfig2W(svc.get(),SERVICE_CONFIG_DELAYED_AUTO_START_INFO,Some((&info as *const SERVICE_DELAYED_AUTO_START_INFO).cast())).map_err(win)?;Ok(())}}

fn with_task_service<T>(f:impl FnOnce(&ITaskService)->Result<T>)->Result<T>{let _com=ComApartment::mta().map_err(|hr|StartupError::Platform(format!("CoInitializeEx failed: 0x{:08X}",hr.0 as u32)))?;unsafe{let service:ITaskService=CoCreateInstance(&CLSID_TASK_SCHEDULER,None,CLSCTX_INPROC_SERVER).map_err(win)?;let v=VARIANT::default();service.Connect(&v,&v,&v,&v).map_err(win)?;f(&service)}}

struct Key(HKEY);impl Drop for Key{fn drop(&mut self){unsafe{let _=RegCloseKey(self.0);}}}
fn open_key(root:HKEY,key:&str,access:REG_SAM_FLAGS)->Result<Key>{let mut h=HKEY::default();let key_w=wide(key);let rc=unsafe{RegOpenKeyExW(root,PCWSTR(key_w.as_ptr()),None,access,&mut h)};if rc==ERROR_FILE_NOT_FOUND{return Err(StartupError::Platform("not-found".into()))}if rc!=ERROR_SUCCESS{return Err(StartupError::Platform(format!("RegOpenKeyExW error {}",rc.0)))}Ok(Key(h))}
fn enum_subkeys(root:HKEY,key:&str)->Result<Vec<String>>{let h=if key.is_empty(){None}else{Some(open_key(root,key,KEY_READ)?)};let handle=h.as_ref().map(|k|k.0).unwrap_or(root);let mut out=Vec::new();for i in 0..u32::MAX{let mut buf=vec![0u16;1024];let mut len=(buf.len()-1)as u32;let rc=unsafe{RegEnumKeyExW(handle,i,Some(PWSTR(buf.as_mut_ptr())),&mut len,None,None,None,None)};if rc==ERROR_NO_MORE_ITEMS{break}if rc!=ERROR_SUCCESS{return Err(StartupError::Platform(format!("RegEnumKeyExW error {}",rc.0)))}buf.truncate(len as usize);out.push(String::from_utf16_lossy(&buf));}Ok(out)}
fn query_raw_value(root:HKEY,key:&str,name:&str,view:REG_SAM_FLAGS)->Result<(u32,Vec<u8>)>{let h=open_key(root,key,KEY_QUERY_VALUE|view)?;let mut ty=REG_VALUE_TYPE(0);let mut len=0u32;let name_w=wide(name);let name_p=PCWSTR(name_w.as_ptr());let rc=unsafe{RegQueryValueExW(h.0,name_p,None,Some(&mut ty),None,Some(&mut len))};if rc==ERROR_FILE_NOT_FOUND{return Err(StartupError::Platform("not-found".into()))}if rc!=ERROR_SUCCESS{return Err(StartupError::Platform(format!("RegQueryValueExW size error {}",rc.0)))}let mut data=vec![0u8;len as usize];let rc=unsafe{RegQueryValueExW(h.0,name_p,None,Some(&mut ty),Some(data.as_mut_ptr()),Some(&mut len))};if rc!=ERROR_SUCCESS{return Err(StartupError::Platform(format!("RegQueryValueExW error {}",rc.0)))}data.truncate(len as usize);Ok((ty.0,data))}
fn query_dword(root:HKEY,key:&str,name:&str)->Result<u32>{let(_,data)=query_raw_value(root,key,name,REG_SAM_FLAGS(0))?;if data.len()<4{return Err(StartupError::Platform(format!("{name} is not a DWORD")))}Ok(u32::from_le_bytes([data[0],data[1],data[2],data[3]]))}
fn query_string(root:HKEY,key:&str,name:&str)->Result<String>{let(ty,data)=query_raw_value(root,key,name,REG_SAM_FLAGS(0))?;if ty!=REG_SZ.0&&ty!=REG_EXPAND_SZ.0{return Err(StartupError::Platform(format!("{name} is not a string")))}Ok(decode_utf16_bytes(&data))}
fn query_multi_string(root:HKEY,key:&str,name:&str)->Result<Vec<String>>{let(ty,data)=query_raw_value(root,key,name,REG_SAM_FLAGS(0))?;if ty!=REG_MULTI_SZ.0{return Ok(vec![])}let u=bytes_to_u16(&data);Ok(String::from_utf16_lossy(&u).split('\0').filter(|s|!s.is_empty()).map(str::to_owned).collect())}
fn parse_hive(hive:&str,key:&str)->(HKEY,String){if let Some(sid)=hive.strip_prefix("HKU\\"){if key.starts_with(&format!("{}\\",sid)){(HKEY_USERS,key.into())}else{(HKEY_USERS,format!(r"{}\{}",sid,key))}}else{(HKEY_LOCAL_MACHINE,key.into())}}

fn modified_ms(m:&std::fs::Metadata)->i64{m.modified().ok().and_then(|t|t.duration_since(UNIX_EPOCH).ok()).map(|d|d.as_millis()as i64).unwrap_or(0)}
fn bytes_to_u16(data:&[u8])->Vec<u16>{data.chunks_exact(2).map(|c|u16::from_le_bytes([c[0],c[1]])).collect()}
fn decode_utf16_bytes(data:&[u8])->String{let mut v=bytes_to_u16(data);while v.last()==Some(&0){v.pop();}String::from_utf16_lossy(&v)}
fn wide(v:&str)->Vec<u16>{OsStr::new(v).encode_wide().chain(Some(0)).collect()}
fn file_sha256(path:&Path)->Result<String>{let mut f=std::fs::File::open(path).map_err(|e|StartupError::Platform(e.to_string()))?;let mut h=Sha256::new();let mut buf=[0u8;64*1024];loop{let n=f.read(&mut buf).map_err(|e|StartupError::Platform(e.to_string()))?;if n==0{break}h.update(&buf[..n]);}Ok(hex::encode(h.finalize()))}
fn task_definition_hash(xml:&str)->String{
    // IRegisteredTask::SetEnabled changes the task-level Settings/Enabled property.
    // Remove only that element from identity material. Trigger-level Enabled fields
    // remain hashed so unrelated scheduling drift is never hidden.
    let normalized=normalize_task_settings_enabled(xml);
    hex::encode(Sha256::digest(normalized.as_bytes()))
}
fn normalize_task_settings_enabled(xml:&str)->String {
    let Some(settings_start)=xml.find("<Settings") else{return xml.to_owned()};
    let Some(open_rel)=xml[settings_start..].find('>') else{return xml.to_owned()};
    let body_start=settings_start+open_rel+1;
    let Some(close_rel)=xml[body_start..].find("</Settings>") else{return xml.to_owned()};
    let body_end=body_start+close_rel;
    let body=&xml[body_start..body_end];
    let Some(enabled_start_rel)=body.find("<Enabled>") else{return xml.to_owned()};
    let enabled_start=body_start+enabled_start_rel;
    let value_start=enabled_start+"<Enabled>".len();
    let Some(enabled_close_rel)=xml[value_start..body_end].find("</Enabled>") else{return xml.to_owned()};
    let enabled_end=value_start+enabled_close_rel+"</Enabled>".len();
    let mut out=String::with_capacity(xml.len());
    out.push_str(&xml[..enabled_start]);
    out.push_str(&xml[enabled_end..]);
    out
}
fn stable_id(v:&str)->String{hex::encode(Sha256::digest(v.as_bytes()))[..32].to_string()}
fn win(e:windows::core::Error)->StartupError{StartupError::Platform(e.to_string())}
fn extract_xml_tag(xml:&str,tag:&str)->Option<String>{let open=format!("<{tag}>");let close=format!("</{tag}>");let start=xml.find(&open)?+open.len();let end=xml[start..].find(&close)?+start;Some(xml[start..end].replace("&amp;","&").replace("&quot;","\""))}
fn is_security_or_system_command(v:&str)->bool{let x=v.to_ascii_lowercase();["\\windows\\system32\\","%systemroot%\\","%windir%\\","microsoft\\windows","windows defender","windefend","securityhealth","antivirus","antimalware","endpoint protection","edr","sense.exe"].iter().any(|k|x.contains(k))}
fn is_protected_service_role(v:&str)->bool{let x=v.to_ascii_lowercase();["network","ethernet","wireless","wi-fi","wifi","wlan","vpn","storage","disk","nvme","raid","keyboard","mouse","touchpad","input","accessibility","screen reader","braille"].iter().any(|k|x.contains(k))}
fn is_essential_service(n:&str)->bool{matches!(n,"rpcss"|"dcomlaunch"|"plugplay"|"power"|"profsvc"|"usermanager"|"gpsvc"|"schedule"|"eventlog"|"bfe"|"mpssvc"|"winmgmt"|"cryptsvc"|"nsi"|"dhcp"|"dnscache"|"wlansvc"|"audiosrv"|"themes"|"appinfo"|"trustedinstaller"|"wuauserv"|"bits"|"windefend"|"securityhealthservice"|"sense"|"wdnissvc")}

#[cfg(test)]
mod windows_policy_tests {
    use super::*;

    #[test]
    fn essential_service_names_are_protected() {
        assert!(is_essential_service("rpcss"));
        assert!(is_essential_service("windefend"));
        assert!(!is_essential_service("vendoragent"));
    }

    #[test]
    fn networking_storage_input_and_accessibility_roles_are_protected() {
        assert!(is_protected_service_role("Vendor VPN Tunnel Service"));
        assert!(is_protected_service_role("OEM NVMe Storage Agent"));
        assert!(is_protected_service_role("Touchpad Input Service"));
        assert!(is_protected_service_role("Braille Accessibility Helper"));
        assert!(!is_protected_service_role("Vendor Update Agent"));
    }

    #[test]
    fn security_paths_fail_closed() {
        assert!(is_security_or_system_command(r"C:\Windows\System32\foo.exe"));
        assert!(is_security_or_system_command(r"C:\Vendor\Endpoint Protection\agent.exe"));
    }

    #[test]
    fn task_hash_ignores_only_task_level_enabled_state() {
        let a = "<Task><Triggers><LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers><Settings><Enabled>true</Enabled></Settings><Actions>A</Actions></Task>";
        let b = "<Task><Triggers><LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers><Settings><Enabled>false</Enabled></Settings><Actions>A</Actions></Task>";
        let c = "<Task><Triggers><LogonTrigger><Enabled>false</Enabled></LogonTrigger></Triggers><Settings><Enabled>false</Enabled></Settings><Actions>A</Actions></Task>";
        let d = "<Task><Triggers><LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers><Settings></Settings><Actions>A</Actions></Task>";
        assert_eq!(task_definition_hash(a), task_definition_hash(b));
        assert_eq!(task_definition_hash(a), task_definition_hash(d));
        assert_ne!(task_definition_hash(a), task_definition_hash(c));
    }

    #[test]
    #[ignore = "read-only native Windows Phase 5 probe"]
    fn live_inventory_is_read_only() {
        let (items, _warnings) = WindowsStartupPlatform.scan().expect("startup inventory");
        assert!(items.iter().all(|item| item.enabled));
        assert!(items.iter().all(|item| item.recommendation == "Review" || item.recommendation == "Keep enabled"));
    }
}
