#![forbid(unsafe_code)]

use std::{fs::File, io::{Read, Write}, path::Path, time::Duration};
use sha2::{Digest,Sha256};
use thiserror::Error;

#[derive(Debug,Error)]
pub enum DownloadError{
    #[error("update URL is not HTTPS")] InvalidHttpsUrl,
    #[error("network error: {0}")] Network(String),
    #[error("update server returned HTTP {0}")] HttpStatus(u16),
    #[error("update response is larger than the allowed limit")] ResponseTooLarge,
    #[error("update artifact size does not match signed metadata")] SizeMismatch,
    #[error("update download was cancelled")] Cancelled,
    #[error("filesystem error: {0}")] Io(#[from]std::io::Error),
}

#[derive(Clone)]
pub struct HttpsTransport{client:reqwest::blocking::Client}

impl HttpsTransport{
    pub fn new()->Result<Self,DownloadError>{
        let client=reqwest::blocking::Client::builder()
            .user_agent("AetherCore-Update/1")
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build().map_err(|e|DownloadError::Network(e.to_string()))?;
        Ok(Self{client})
    }
    pub fn get_bytes(&self,url:&str,max_bytes:usize)->Result<Vec<u8>,DownloadError>{
        ensure_https(url)?;if max_bytes==0{return Err(DownloadError::ResponseTooLarge)}
        let mut response=self.client.get(url).send().map_err(|e|DownloadError::Network(e.to_string()))?;
        if response.status()!=reqwest::StatusCode::OK{return Err(DownloadError::HttpStatus(response.status().as_u16()))}
        if response.content_length().is_some_and(|n|n>max_bytes as u64){return Err(DownloadError::ResponseTooLarge)}
        let mut bytes=Vec::with_capacity(response.content_length().unwrap_or(0).min(max_bytes as u64) as usize);
        response.by_ref().take(max_bytes as u64+1).read_to_end(&mut bytes).map_err(|e|DownloadError::Network(e.to_string()))?;
        if bytes.len()>max_bytes{return Err(DownloadError::ResponseTooLarge)}Ok(bytes)
    }
    pub fn download_to_file(&self,url:&str,destination:&Path,max_bytes:u64,expected_size:u64,cancelled:&dyn Fn()->bool,progress:&mut dyn FnMut(u64,u64))->Result<String,DownloadError>{
        ensure_https(url)?;if expected_size==0||expected_size>max_bytes{return Err(DownloadError::ResponseTooLarge)}
        let mut response=self.client.get(url).send().map_err(|e|DownloadError::Network(e.to_string()))?;
        if response.status()!=reqwest::StatusCode::OK{return Err(DownloadError::HttpStatus(response.status().as_u16()))}
        if response.content_length().is_some_and(|n|n!=expected_size||n>max_bytes){return Err(DownloadError::SizeMismatch)}
        if let Some(parent)=destination.parent(){std::fs::create_dir_all(parent)?}
        let temporary=destination.with_extension("download-part");let _=std::fs::remove_file(&temporary);let mut cleanup=TempDownload::new(temporary.clone());
        let mut file=File::create(&temporary)?;let mut hash=Sha256::new();let mut total=0u64;let mut buffer=[0u8;128*1024];
        loop{if cancelled(){return Err(DownloadError::Cancelled)}let read=response.read(&mut buffer).map_err(|e|DownloadError::Network(e.to_string()))?;if read==0{break}total=total.checked_add(read as u64).ok_or(DownloadError::ResponseTooLarge)?;if total>max_bytes||total>expected_size{return Err(DownloadError::SizeMismatch)}file.write_all(&buffer[..read])?;hash.update(&buffer[..read]);progress(total,expected_size);}
        file.sync_all()?;drop(file);if total!=expected_size{return Err(DownloadError::SizeMismatch)}
        match std::fs::remove_file(destination){Ok(())=>{},Err(e)if e.kind()==std::io::ErrorKind::NotFound=>{},Err(e)=>return Err(DownloadError::Io(e))}
        std::fs::rename(&temporary,destination)?;cleanup.commit();Ok(hex::encode(hash.finalize()))
    }
}

fn ensure_https(url:&str)->Result<(),DownloadError>{
    if url.len()>2048||!url.starts_with("https://")||url.bytes().any(|b|b.is_ascii_control()||b==b' '||b==b'\\'){return Err(DownloadError::InvalidHttpsUrl)}
    let rest=&url[8..];let authority=rest.split('/').next().unwrap_or("");if authority.is_empty()||authority.contains('@')||url.contains('#'){return Err(DownloadError::InvalidHttpsUrl)}Ok(())
}

struct TempDownload{path:std::path::PathBuf,committed:bool}
impl TempDownload{fn new(path:std::path::PathBuf)->Self{Self{path,committed:false}}fn commit(&mut self){self.committed=true;}}
impl Drop for TempDownload{fn drop(&mut self){if !self.committed{let _=std::fs::remove_file(&self.path);}}}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn downloader_rejects_non_https_and_ambiguous_authorities(){
        for value in ["http://example.invalid/a","https://u:p@example.invalid/a","https://example.invalid/a#x","https://example.invalid/a path"]{assert!(ensure_https(value).is_err(),"{value}");}
        assert!(ensure_https("https://updates.example.invalid/a").is_ok());
    }
}
