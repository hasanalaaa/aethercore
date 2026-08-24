fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }

    let mut config = prost_build::Config::new();
    config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    config.type_attribute(".", "#[serde(rename_all = \"camelCase\")]");
    // events.proto is the Phase 10 graph root. It imports operations plus every domain schema,
    // producing one package-compatible aethercore.v1 module while keeping contracts modular.
    config
        .compile_protos(&["proto/events.proto"], &["proto"])
        .expect("compile protobuf contracts");

    for file in [
        "common.proto",
        "operations.proto",
        "drivers.proto",
        "repair.proto",
        "cleanup.proto",
        "startup.proto",
        "diagnostics.proto",
        "intelligence.proto",
        "scheduler.proto",
        "update.proto",
        "support_bundle.proto",
        "performance.proto",
        "timeline.proto",
        "care.proto",
        "events.proto",
        "aethercore.proto",
    ] {
        println!("cargo:rerun-if-changed=proto/{file}");
    }
}
