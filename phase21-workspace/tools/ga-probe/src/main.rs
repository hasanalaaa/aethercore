#![forbid(unsafe_code)]

#[cfg(windows)]
mod windows_main {
    use std::{
        env,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    use aethercore_contracts::{
        PROTOCOL_VERSION,
        v1::{HydrateSessionRequest, PingRequest, Request, RequestHeader, request},
    };
    use aethercore_ipc::SessionClient;

    #[derive(Clone, Copy)]
    struct Config {
        sessions: usize,
        requests_per_session: usize,
        timeout_ms: u64,
        reconnect_every: usize,
    }

    fn parse_usize(flag: &str, default: usize, min: usize, max: usize) -> Result<usize, String> {
        let args: Vec<String> = env::args().collect();
        let Some(index) = args.iter().position(|value| value == flag) else {
            return Ok(default);
        };
        let raw = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value after {flag}"))?;
        let value = raw
            .parse::<usize>()
            .map_err(|_| format!("invalid integer for {flag}: {raw}"))?;
        if !(min..=max).contains(&value) {
            return Err(format!("{flag} must be between {min} and {max}"));
        }
        Ok(value)
    }

    fn config() -> Result<Config, String> {
        Ok(Config {
            sessions: parse_usize("--sessions", 4, 1, 4)?,
            requests_per_session: parse_usize("--requests-per-session", 2_000, 1, 1_000_000)?,
            timeout_ms: parse_usize("--timeout-ms", 5_000, 100, 120_000)? as u64,
            reconnect_every: parse_usize("--reconnect-every", 250, 1, 1_000_000)?,
        })
    }

    fn request(id: String, hydrate: bool) -> Request {
        let payload = if hydrate {
            request::Payload::HydrateSession(HydrateSessionRequest {})
        } else {
            request::Payload::Ping(PingRequest {})
        };
        Request {
            header: Some(RequestHeader {
                protocol_version: PROTOCOL_VERSION,
                request_id: id,
            }),
            payload: Some(payload),
        }
    }

    fn connect(
        replay_after: u64,
        failed: Arc<AtomicBool>,
        reset_count: Arc<AtomicU64>,
    ) -> Result<(Arc<SessionClient>, Arc<AtomicU64>), String> {
        let last_sequence = Arc::new(AtomicU64::new(replay_after));
        let sequence_for_event = last_sequence.clone();
        let failed_for_event = failed.clone();
        let client = SessionClient::connect(
            "aethercore-ga-probe",
            env!("CARGO_PKG_VERSION"),
            replay_after,
            Arc::new(move |event| {
                let prior = sequence_for_event.load(Ordering::Acquire);
                if event.sequence <= prior {
                    failed_for_event.store(true, Ordering::Release);
                    return;
                }
                sequence_for_event.store(event.sequence, Ordering::Release);
            }),
            Arc::new(move |_| {
                reset_count.fetch_add(1, Ordering::AcqRel);
            }),
            Arc::new(|| {}),
        )
        .map_err(|error| error.to_string())?;
        Ok((client, last_sequence))
    }

    pub fn run() -> Result<(), String> {
        let cfg = config()?;
        let started = Instant::now();
        let failed = Arc::new(AtomicBool::new(false));
        let total_requests = Arc::new(AtomicU64::new(0));
        let reconnects = Arc::new(AtomicU64::new(0));
        let stream_resets = Arc::new(AtomicU64::new(0));
        let request_failures = Arc::new(AtomicU64::new(0));
        let mut workers = Vec::with_capacity(cfg.sessions);

        for worker_id in 0..cfg.sessions {
            let failed = failed.clone();
            let total_requests = total_requests.clone();
            let reconnects = reconnects.clone();
            let stream_resets = stream_resets.clone();
            let request_failures = request_failures.clone();
            workers.push(thread::spawn(move || -> Result<(), String> {
                let mut replay_after = 0u64;
                let (mut client, mut last_sequence) =
                    connect(replay_after, failed.clone(), stream_resets.clone())?;
                for index in 0..cfg.requests_per_session {
                    if index > 0 && index % cfg.reconnect_every == 0 {
                        replay_after = last_sequence.load(Ordering::Acquire);
                        drop(client);
                        reconnects.fetch_add(1, Ordering::AcqRel);
                        let connected =
                            connect(replay_after, failed.clone(), stream_resets.clone())?;
                        client = connected.0;
                        last_sequence = connected.1;
                    }
                    let id = format!(
                        "ga-{worker_id}-{index}-{}",
                        total_requests.fetch_add(1, Ordering::AcqRel)
                    );
                    let hydrate = index % 16 == 15;
                    match client
                        .request(request(id, hydrate), Duration::from_millis(cfg.timeout_ms))
                    {
                        Ok(response) if response.status_code == 200 => {}
                        Ok(response) => {
                            request_failures.fetch_add(1, Ordering::AcqRel);
                            return Err(format!(
                                "service returned status {}",
                                response.status_code
                            ));
                        }
                        Err(error) => {
                            request_failures.fetch_add(1, Ordering::AcqRel);
                            return Err(format!("request failed: {error}"));
                        }
                    }
                    if failed.load(Ordering::Acquire) {
                        return Err("non-monotonic event sequence observed".into());
                    }
                }
                Ok(())
            }));
        }

        for worker in workers {
            match worker.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => return Err(error),
                Err(_) => return Err("GA probe worker panicked".into()),
            }
        }

        let elapsed_ms = started.elapsed().as_millis();
        println!(
            "{{\"schema\":\"aethercore.ga-probe.v1\",\"ok\":true,\"sessions\":{},\"requests\":{},\"reconnects\":{},\"stream_resets\":{},\"request_failures\":{},\"elapsed_ms\":{}}}",
            cfg.sessions,
            total_requests.load(Ordering::Acquire),
            reconnects.load(Ordering::Acquire),
            stream_resets.load(Ordering::Acquire),
            request_failures.load(Ordering::Acquire),
            elapsed_ms,
        );
        Ok(())
    }
}

fn main() {
    #[cfg(windows)]
    {
        if let Err(error) = windows_main::run() {
            eprintln!(
                "{{\"schema\":\"aethercore.ga-probe.v1\",\"ok\":false,\"error\":{:?}}}",
                error
            );
            std::process::exit(1);
        }
        return;
    }
    #[cfg(not(windows))]
    {
        eprintln!("aethercore-ga-probe requires Windows");
        std::process::exit(2);
    }
}
