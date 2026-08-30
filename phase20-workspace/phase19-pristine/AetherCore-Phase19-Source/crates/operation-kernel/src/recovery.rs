use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Clone, Default)] pub struct RecoverySupervisor { completed:Arc<Mutex<Vec<String>>> }
#[derive(Debug,Error)] pub enum RecoveryError { #[error("recovery task {task} failed: {detail}")] Task{task:String,detail:String} }
impl RecoverySupervisor {
    pub fn new()->Self{Self::default()}
    pub fn run_task<E:std::fmt::Display>(&self,name:&str,task:impl FnOnce()->Result<(),E>)->Result<(),RecoveryError>{task().map_err(|e|RecoveryError::Task{task:name.into(),detail:e.to_string()})?;self.completed.lock().unwrap_or_else(|p|p.into_inner()).push(name.into());Ok(())}
    pub fn completed(&self)->Vec<String>{self.completed.lock().unwrap_or_else(|p|p.into_inner()).clone()}
}
