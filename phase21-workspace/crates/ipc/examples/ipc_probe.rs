fn main() {
    #[cfg(windows)]
    {
        let noop_event: std::sync::Arc<dyn Fn(aethercore_contracts::v1::EventEnvelope) + Send + Sync> =
            std::sync::Arc::new(|_| {});
        let noop_reset: std::sync::Arc<dyn Fn(aethercore_contracts::v1::StreamReset) + Send + Sync> =
            std::sync::Arc::new(|_| {});
        let noop_disconnect: std::sync::Arc<dyn Fn() + Send + Sync> = std::sync::Arc::new(|| {});
        match aethercore_ipc::SessionClient::connect(
            "p36-probe",
            env!("CARGO_PKG_VERSION"),
            0,
            noop_event,
            noop_reset,
            noop_disconnect,
        ) {
            Ok(client) => {
                println!("CONNECT=OK");
                println!("server_hello: {:?}", client.hello);
            }
            Err(e) => {
                println!("CONNECT=FAIL");
                println!("error_debug: {e:?}");
                println!("error_display: {e}");
            }
        }
    }
    #[cfg(not(windows))]
    {
        println!("windows-only probe");
    }
}
