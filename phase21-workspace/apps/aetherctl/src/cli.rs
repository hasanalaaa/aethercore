//! Hand-rolled argument parser (repo style — no clap). Global flags are accepted BEFORE
//! the subcommand only; anything after the command belongs to that command.
//! Unknown flags, missing values, and malformed values are typed Usage failures (exit 2).

use crate::error::CliError;
use std::path::PathBuf;
use std::time::Duration;

pub const USAGE: &str = "\
aetherctl -- AetherCore headless command surface

USAGE: aetherctl [global flags] <command> [subcommand] [command flags]

GLOBAL FLAGS (before the command):
  --socket-dir <path>    unix IPC rendezvous directory (default: platform default)
  --timeout-ms <n>       per-request deadline in milliseconds (default: 10000)
  --output <json|text>   output mode (default: text)
  --no-color             disable ANSI emphasis in text mode
  --version              print version and exit
  help                   print this usage block

OFFLINE COMMANDS (no service required, strictly read-only):
  about | version | capabilities | engine-source
  telemetry-once [--interval-ms <n>]
  self-check [--load-model]
  service detect | service units --print

SERVICE COMMANDS (require the maintenance-service endpoint):
  doctor
  perf      start [--interval-ms <n>] | stop | snapshot | report
  optimize  plan [--findings <id,id,...>] | start [--plan-id <id>] | status [--plan-id <id>]
  timeline  page [--size <n>] [--before <seq>] | patterns
  care      status | start [--non-interactive] | cancel | consent-grant
  insights  list | explain [--question <key>] | dismiss --insight-id <id>
  scan      start | cancel --scan-id <id> | status | history [--limit <n>]

SECURITY COMMANDS (Phase 32; offline unless noted):
  sec       audit [--ssh <cfg>] [--sudoers <f>] [--fs <dir>] [--authlog <f>]
              [--secrets <dir>] [--firewall]   (offline direct, read-only)
  sec       audit --profile <cis-l1|cis-l2> --out <file>
              [--format <json|html|both>] [--sign --key <seedfile>]
  sec       report --in <report.json>        (re-render a saved report)
  compliance summary --profile cis-l1 --report <file> --map <cis_map.json>
  compliance verify <report.json>            (offline integrity/signature verification)
  vulndb    update --from <db.json> --dest <dir>   (EXPLICIT owner action)
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Text,
    Json,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// None => platform default socket dir (crates/ipc resolution).
    pub socket_dir: Option<PathBuf>,
    pub timeout: Duration,
    pub output: OutputMode,
    pub no_color: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            socket_dir: None,
            timeout: Duration::from_millis(10_000),
            output: OutputMode::Text,
            no_color: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Invocation {
    pub config: Config,
    pub command: Command,
    pub version_requested: bool,
    /// Phase 31 (W6): resolved UI language (--lang > AETHERCORE_LANG > en).
    pub lang: crate::i18n::Lang,
}

#[derive(Debug, Clone)]
pub enum Command {
    Offline(OfflineJob),
    Service(ServiceJob),
}

#[derive(Debug, Clone)]
pub enum OfflineJob {
    About,
    Version,
    Capabilities,
    EngineSource,
    TelemetryOnce {
        interval_ms: u32,
    },
    SelfCheck {
        load_model: bool,
    },
    ServiceDetect,
    ServiceUnits,
    /// Phase 29 (T3): offline EXPORT_V1 chain verification — zero service dependency.
    ExportVerify {
        file: String,
    },
    /// Phase 29 (T3): explicit owner key generation (NEVER implicit).
    KeysGenerate {
        out: String,
    },
    /// Phase 29 (T3): print an Ed25519 public-key fingerprint for a seed file.
    KeysFingerprint {
        input: String,
    },
    /// Phase 30 (T6): offline read-only SQLite diagnostics.
    DbCheckSqlite {
        path: String,
    },
    /// Phase 32 (T7): OFFLINE security-audit lanes over explicit targets.
    SecAudit {
        /// (kind, path) pairs parsed from flag form; kind in
        /// {ssh,sudoers,fs,authlog,secrets}; firewall = ("firewall","").
        targets: Vec<(String, String)>,
    },
    /// Phase 33: offline CIS compliance report generation.
    ComplianceAudit {
        profile: String,
        out: String,
        format: String,
        sign: bool,
        key: Option<String>,
        targets: Vec<(String, String)>,
    },
    /// Phase 33: offline compliance report verification.
    ComplianceVerify {
        file: String,
    },
    /// Phase 32 (T7): re-render a saved SecurityAuditReport.
    SecReport {
        file: String,
    },
    /// Phase 32 (S5): CIS L1 control-family pass/fail table over a saved report.
    SecComplianceSummary {
        profile: String,
        report_file: String,
        map_file: String,
    },
    /// Phase 32 (S3): explicit owner action — install DB + write fresh manifest pin.
    VulndbUpdate {
        from: String,
        dest: String,
    },
    Help,
}

#[derive(Debug, Clone)]
pub enum ServiceJob {
    Doctor,
    PerfStart {
        interval_ms: u32,
    },
    PerfStop,
    PerfSnapshot,
    PerfReport,
    OptimizePlan {
        findings: Option<Vec<String>>,
    },
    OptimizeStart {
        plan_id: Option<String>,
    },
    OptimizeStatus {
        plan_id: Option<String>,
    },
    TimelinePage {
        page_size: u32,
        before_sequence: u64,
    },
    TimelinePatterns,
    CareStatus,
    CareStart {
        non_interactive: bool,
    },
    CareCancel,
    CareConsentGrant,
    InsightsList,
    InsightsExplain {
        question_key: String,
    },
    InsightsDismiss {
        insight_id: String,
    },
    ScanStart,
    ScanCancel {
        scan_id: String,
    },
    ScanStatus,
    ScanHistory {
        limit: u32,
    },
    /// Phase 29 (T3): daemon-backed EXPORT_V1 journal export written to `out`.
    ExportJournal {
        from_unix_ms: i64,
        to_unix_ms: i64,
        out: String,
    },
}

struct Cursor<'a> {
    args: &'a [String],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(args: &'a [String]) -> Self {
        Cursor { args, pos: 1 }
    }

    fn peek(&self) -> Option<&'a String> {
        self.args.get(self.pos)
    }

    fn next(&mut self) -> Option<&'a String> {
        let value = self.args.get(self.pos);
        if value.is_some() {
            self.pos += 1;
        }
        value
    }

    fn value_after(&mut self, flag: &str) -> Result<String, CliError> {
        let raw = match self.next() {
            Some(raw) => raw.clone(),
            None => return Err(value_missing(flag)),
        };
        if raw.starts_with("--") || raw.is_empty() {
            return Err(value_missing(flag));
        }
        Ok(raw)
    }
}

fn value_missing(flag: &str) -> CliError {
    CliError::Usage {
        message_key: format!(
            "cli.usage.flag{}RequiresValue",
            flag.trim_start_matches('-')
        ),
        detail: Some(format!("flag {flag} requires a value")),
    }
}

fn usage(message_key: &str, detail: String) -> CliError {
    CliError::Usage {
        message_key: message_key.to_string(),
        detail: Some(detail),
    }
}

/// Parses `args` (including argv[0]).
pub fn parse(args: &[String]) -> Result<Invocation, CliError> {
    let mut cursor = Cursor::new(args);
    let mut config = Config::default();
    let mut version_requested = false;
    let mut lang_flag: Option<crate::i18n::Lang> = None;

    // Global flags BEFORE the command only.
    while let Some(arg) = cursor.peek() {
        match arg.as_str() {
            "--socket-dir" => {
                cursor.next();
                config.socket_dir = Some(PathBuf::from(cursor.value_after("--socket-dir")?));
            }
            "--timeout-ms" => {
                cursor.next();
                let value = cursor.value_after("--timeout-ms")?;
                let millis: u64 = value.parse().map_err(|_| {
                    usage(
                        "cli.usage.timeoutMsInvalid",
                        format!("--timeout-ms expects an integer, got '{value}'"),
                    )
                })?;
                if millis == 0 {
                    return Err(usage(
                        "cli.usage.timeoutMsInvalid",
                        "--timeout-ms must be > 0".to_string(),
                    ));
                }
                config.timeout = Duration::from_millis(millis);
            }
            "--output" => {
                cursor.next();
                let value = cursor.value_after("--output")?;
                config.output = match value.as_str() {
                    "json" => OutputMode::Json,
                    "text" => OutputMode::Text,
                    other => {
                        return Err(usage(
                            "cli.usage.outputModeInvalid",
                            format!("--output expects json|text, got '{other}'"),
                        ));
                    }
                };
            }
            "--no-color" => {
                cursor.next();
                config.no_color = true;
            }
            "--lang" => {
                cursor.next();
                let value = cursor.value_after("--lang")?;
                lang_flag = crate::i18n::Lang::parse(&value);
                if lang_flag.is_none() {
                    return Err(usage(
                        "cli.usage.langInvalid",
                        format!("--lang expects en|ar, got '{value}'"),
                    ));
                }
            }
            "--version" => {
                cursor.next();
                version_requested = true;
            }
            _ => break,
        }
    }

    if version_requested && cursor.peek().is_none() {
        return Ok(Invocation {
            config,
            command: Command::Offline(OfflineJob::About),
            version_requested,
            lang: crate::i18n::resolve(lang_flag.map(|l| match l {
                crate::i18n::Lang::Ar => "ar",
                _ => "en",
            })),
        });
    }

    let name = match cursor.next() {
        Some(name) => name.clone(),
        None => return Err(CliError::usage("cli.usage.commandRequired")),
    };

    let command = match name.as_str() {
        "help" => Command::Offline(OfflineJob::Help),
        "about" => Command::Offline(OfflineJob::About),
        "version" => Command::Offline(OfflineJob::Version),
        "capabilities" => Command::Offline(OfflineJob::Capabilities),
        "engine-source" => Command::Offline(OfflineJob::EngineSource),
        "telemetry-once" => {
            let mut interval_ms = 250u32;
            while let Some(flag) = cursor.next() {
                match flag.as_str() {
                    "--interval-ms" => {
                        interval_ms = parse_u32(&mut cursor, "--interval-ms", 250, 60_000)?;
                    }
                    other => return Err(unknown_flag(other)),
                }
            }
            Command::Offline(OfflineJob::TelemetryOnce { interval_ms })
        }
        "self-check" => {
            let mut load_model = false;
            while let Some(flag) = cursor.next() {
                match flag.as_str() {
                    "--load-model" => load_model = true,
                    other => return Err(unknown_flag(other)),
                }
            }
            Command::Offline(OfflineJob::SelfCheck { load_model })
        }
        // Phase 29 (T3): export journal (daemon-backed) | export verify <file> (offline).
        "export" => {
            let sub = cursor.next().cloned().ok_or_else(|| {
                usage(
                    "cli.usage.exportSubcommandRequired",
                    "export requires journal|verify".to_string(),
                )
            })?;
            match sub.as_str() {
                "journal" => {
                    let mut from_ms = 0i64;
                    let mut to_ms = 0i64;
                    let mut out: Option<String> = None;
                    while let Some(flag) = cursor.next() {
                        match flag.as_str() {
                            "--from-ms" => {
                                from_ms = parse_i64(&mut cursor, "--from-ms")?;
                            }
                            "--to-ms" => {
                                to_ms = parse_i64(&mut cursor, "--to-ms")?;
                            }
                            "--out" => out = Some(cursor.value_after("--out")?),
                            other => return Err(unknown_flag(other)),
                        }
                    }
                    let out = out.ok_or_else(|| {
                        usage(
                            "cli.usage.exportRequiresOut",
                            "export journal requires --out <file>".to_string(),
                        )
                    })?;
                    Command::Service(ServiceJob::ExportJournal {
                        from_unix_ms: from_ms,
                        to_unix_ms: to_ms,
                        out,
                    })
                }
                "verify" => {
                    let file = cursor.next().cloned().ok_or_else(|| {
                        usage(
                            "cli.usage.exportVerifyRequiresFile",
                            "export verify requires a file path".to_string(),
                        )
                    })?;
                    if cursor.peek().is_some() {
                        return Err(unknown_flag(cursor.peek().unwrap_or(&String::new())));
                    }
                    Command::Offline(OfflineJob::ExportVerify { file })
                }
                other => {
                    return Err(usage(
                        "cli.usage.unknownSubcommand",
                        format!("unknown export subcommand '{other}' (journal|verify)"),
                    ));
                }
            }
        }
        // Phase 32 (T7): sec audit/report + compliance summary + vulndb update.
        "sec" => {
            let sub = cursor.next().cloned().ok_or_else(|| {
                usage(
                    "cli.usage.subcommandRequired",
                    "sec requires audit|report".to_string(),
                )
            })?;
            match sub.as_str() {
                "audit" => {
                    let mut targets: Vec<(String, String)> = Vec::new();
                    let mut profile = String::from("cis-l1");
                    let mut out = None;
                    let mut format = String::from("json");
                    let mut sign = false;
                    let mut key = None;
                    let mut compliance_requested = false;
                    while let Some(flag) = cursor.peek() {
                        let flag = flag.clone();
                        match flag.as_str() {
                            "--ssh" => {
                                cursor.next();
                                targets.push(("ssh".into(), cursor.value_after("--ssh")?));
                            }
                            "--sudoers" => {
                                cursor.next();
                                targets.push(("sudoers".into(), cursor.value_after("--sudoers")?));
                            }
                            "--fs" => {
                                cursor.next();
                                targets.push(("fs".into(), cursor.value_after("--fs")?));
                            }
                            "--authlog" => {
                                cursor.next();
                                targets.push(("authlog".into(), cursor.value_after("--authlog")?));
                            }
                            "--secrets" => {
                                cursor.next();
                                targets.push(("secrets".into(), cursor.value_after("--secrets")?));
                            }
                            "--firewall" => {
                                cursor.next();
                                targets.push(("firewall".into(), String::new()));
                            }
                            "--profile" => {
                                cursor.next();
                                profile = cursor.value_after("--profile")?;
                                compliance_requested = true;
                            }
                            "--out" => {
                                cursor.next();
                                out = Some(cursor.value_after("--out")?);
                                compliance_requested = true;
                            }
                            "--format" => {
                                cursor.next();
                                format = cursor.value_after("--format")?;
                                compliance_requested = true;
                            }
                            "--sign" => {
                                cursor.next();
                                sign = true;
                                compliance_requested = true;
                            }
                            "--key" => {
                                cursor.next();
                                key = Some(cursor.value_after("--key")?);
                                compliance_requested = true;
                            }
                            other => {
                                if other.starts_with("--") {
                                    return Err(unknown_flag(other));
                                }
                                break;
                            }
                        }
                    }
                    if compliance_requested {
                        if !matches!(profile.as_str(), "cis-l1" | "cis-l2") {
                            return Err(usage(
                                "cli.usage.complianceProfile",
                                format!("unsupported profile '{profile}'"),
                            ));
                        }
                        if !matches!(format.as_str(), "json" | "html" | "both") {
                            return Err(usage(
                                "cli.usage.complianceFormat",
                                format!("unsupported report format '{format}'"),
                            ));
                        }
                        if sign != key.is_some() {
                            return Err(usage(
                                "cli.usage.complianceSigning",
                                "--sign and --key <seedfile> must be supplied together".to_string(),
                            ));
                        }
                        Command::Offline(OfflineJob::ComplianceAudit {
                            profile,
                            out: out.ok_or_else(|| value_missing("--out"))?,
                            format,
                            sign,
                            key,
                            targets,
                        })
                    } else if targets.is_empty() {
                        return Err(usage(
                            "cli.usage.secAuditRequiresTarget",
                            "sec audit requires at least one target flag".to_string(),
                        ));
                    } else {
                        Command::Offline(OfflineJob::SecAudit { targets })
                    }
                }
                "report" => {
                    cursor.next();
                    let file = cursor.value_after("--in").or_else(|_| {
                        // tolerate positional form: `sec report <file>`
                        cursor.peek().cloned().ok_or_else(|| value_missing("--in"))
                    })?;
                    Command::Offline(OfflineJob::SecReport { file })
                }
                other => {
                    return Err(usage(
                        "cli.usage.unknownSubcommand",
                        format!("unknown sec subcommand '{other}' (audit|report)"),
                    ));
                }
            }
        }
        // Phase 32 (S5): cis-l1 summary over a saved report (offline).
        "compliance" => {
            let sub = cursor.next().cloned().ok_or_else(|| {
                usage(
                    "cli.usage.subcommandRequired",
                    "compliance requires summary".to_string(),
                )
            })?;
            if sub == "verify" {
                let file = cursor
                    .next()
                    .cloned()
                    .ok_or_else(|| value_missing("<report.json>"))?;
                Command::Offline(OfflineJob::ComplianceVerify { file })
            } else {
                if sub != "summary" {
                    return Err(usage(
                        "cli.usage.unknownSubcommand",
                        format!("unknown compliance subcommand '{sub}'"),
                    ));
                }
                let mut profile = String::from("cis-l1");
                let mut report_file = String::new();
                let mut map_file = String::new();
                while let Some(flag) = cursor.peek().cloned() {
                    match flag.as_str() {
                        "--profile" => {
                            cursor.next();
                            profile = cursor.value_after("--profile")?;
                        }
                        "--report" => {
                            cursor.next();
                            report_file = cursor.value_after("--report")?;
                        }
                        "--map" => {
                            cursor.next();
                            map_file = cursor.value_after("--map")?;
                        }
                        other => {
                            if other.starts_with("--") {
                                return Err(unknown_flag(other));
                            }
                            break;
                        }
                    }
                }
                if report_file.is_empty() || map_file.is_empty() {
                    return Err(usage(
                        "cli.usage.complianceRequiresInputs",
                        "compliance summary requires --report <file> --map <file>".to_string(),
                    ));
                }
                Command::Offline(OfflineJob::SecComplianceSummary {
                    profile,
                    report_file,
                    map_file,
                })
            }
        }
        // Phase 32 (S3): EXPLICIT owner action — install candidate vulndb + pin.
        "vulndb" => {
            let sub = cursor.next().cloned().ok_or_else(|| {
                usage(
                    "cli.usage.subcommandRequired",
                    "vulndb requires update".to_string(),
                )
            })?;
            if sub != "update" {
                return Err(usage(
                    "cli.usage.unknownSubcommand",
                    format!("unknown vulndb subcommand '{sub}'"),
                ));
            }
            let mut from = String::new();
            let mut dest = String::new();
            let mut url: Option<String> = None;
            while let Some(flag) = cursor.peek().cloned() {
                match flag.as_str() {
                    "--from" => {
                        cursor.next();
                        from = cursor.value_after("--from")?;
                    }
                    "--url" => {
                        cursor.next();
                        url = Some(cursor.value_after("--url")?);
                    }
                    "--dest" => {
                        cursor.next();
                        dest = cursor.value_after("--dest")?;
                    }
                    other => {
                        if other.starts_with("--") {
                            return Err(unknown_flag(other));
                        }
                        break;
                    }
                }
            }
            if url.is_some() {
                return Err(usage(
                    "cli.usage.vulndbUrlUnsupported",
                    "vulndb update accepts only --from <file>; network fetch stays banned"
                        .to_string(),
                ));
            }
            if from.is_empty() || dest.is_empty() {
                return Err(usage(
                    "cli.usage.vulndbUpdateRequiresInputs",
                    "vulndb update requires --from <file> --dest <dir>".to_string(),
                ));
            }
            Command::Offline(OfflineJob::VulndbUpdate { from, dest })
        }
        // Phase 29 (T3): keys generate --out <path> | keys fingerprint --in <path>.
        // Explicit owner actions — key material is NEVER created anywhere else.
        "keys" => {
            let sub = cursor.next().cloned().ok_or_else(|| {
                usage(
                    "cli.usage.keysSubcommandRequired",
                    "keys requires generate|fingerprint".to_string(),
                )
            })?;
            match sub.as_str() {
                "generate" => {
                    let mut out: Option<String> = None;
                    while let Some(flag) = cursor.next() {
                        match flag.as_str() {
                            "--out" => out = Some(cursor.value_after("--out")?),
                            other => return Err(unknown_flag(other)),
                        }
                    }
                    let out = out.ok_or_else(|| {
                        usage(
                            "cli.usage.keysGenerateRequiresOut",
                            "keys generate requires --out <path>".to_string(),
                        )
                    })?;
                    Command::Offline(OfflineJob::KeysGenerate { out })
                }
                "fingerprint" => {
                    let input = cursor.next().cloned().ok_or_else(|| {
                        usage(
                            "cli.usage.keysFingerprintRequiresIn",
                            "keys fingerprint requires --in <path>".to_string(),
                        )
                    })?;
                    if input.starts_with("--") {
                        return Err(usage(
                            "cli.usage.keysFingerprintRequiresIn",
                            "keys fingerprint requires --in <path>".to_string(),
                        ));
                    }
                    Command::Offline(OfflineJob::KeysFingerprint { input })
                }
                other => {
                    return Err(usage(
                        "cli.usage.unknownSubcommand",
                        format!("unknown keys subcommand '{other}' (generate|fingerprint)"),
                    ));
                }
            }
        }
        "service" => {
            let sub = cursor.next().cloned().ok_or_else(|| {
                usage(
                    "cli.usage.serviceSubcommandRequired",
                    "service requires detect|units".to_string(),
                )
            })?;
            match sub.as_str() {
                "detect" => Command::Offline(OfflineJob::ServiceDetect),
                "units" => {
                    let mut print = false;
                    while let Some(flag) = cursor.next() {
                        match flag.as_str() {
                            "--print" => print = true,
                            other => return Err(unknown_flag(other)),
                        }
                    }
                    if !print {
                        return Err(usage(
                            "cli.usage.unitsRequiresPrint",
                            "service units supports only --print".to_string(),
                        ));
                    }
                    Command::Offline(OfflineJob::ServiceUnits)
                }
                other => {
                    return Err(usage(
                        "cli.usage.unknownSubcommand",
                        format!("unknown service subcommand '{other}'"),
                    ));
                }
            }
        }
        // Phase 30 (T6): db check — OFFLINE read-only SQLite/config diagnostics.
        "db" => {
            let sub = cursor.next().cloned().ok_or_else(|| {
                usage(
                    "cli.usage.dbSubcommandRequired",
                    "db requires check|check-config|slow-log|report".to_string(),
                )
            })?;
            match sub.as_str() {
                "check" => {
                    let mut sqlite: Option<String> = None;
                    while let Some(flag) = cursor.next() {
                        match flag.as_str() {
                            "--sqlite" => sqlite = Some(cursor.value_after("--sqlite")?),
                            other => return Err(unknown_flag(other)),
                        }
                    }
                    let path = sqlite.ok_or_else(|| {
                        usage(
                            "cli.usage.dbCheckRequiresSqlite",
                            "db check requires --sqlite <file>".to_string(),
                        )
                    })?;
                    Command::Offline(OfflineJob::DbCheckSqlite { path })
                }
                other => {
                    return Err(usage(
                        "cli.usage.unknownSubcommand",
                        format!("unknown db subcommand '{other}'"),
                    ));
                }
            }
        }
        "doctor" => Command::Service(ServiceJob::Doctor),
        "perf" => {
            let sub = cursor
                .next()
                .cloned()
                .ok_or_else(|| usage("cli.usage.perfSubcommandRequired", PERF_SUBS.to_string()))?;
            match sub.as_str() {
                "start" => {
                    let mut interval_ms = 1000u32;
                    while let Some(flag) = cursor.next() {
                        match flag.as_str() {
                            "--interval-ms" => {
                                interval_ms = parse_u32(&mut cursor, "--interval-ms", 250, 60_000)?;
                            }
                            other => return Err(unknown_flag(other)),
                        }
                    }
                    Command::Service(ServiceJob::PerfStart { interval_ms })
                }
                "stop" => Command::Service(ServiceJob::PerfStop),
                "snapshot" => Command::Service(ServiceJob::PerfSnapshot),
                "report" => Command::Service(ServiceJob::PerfReport),
                other => {
                    return Err(usage(
                        "cli.usage.unknownSubcommand",
                        format!("unknown perf subcommand '{other}' ({PERF_SUBS})"),
                    ));
                }
            }
        }
        "optimize" => {
            let sub = cursor.next().cloned().ok_or_else(|| {
                usage(
                    "cli.usage.optimizeSubcommandRequired",
                    OPTIMIZE_SUBS.to_string(),
                )
            })?;
            match sub.as_str() {
                "plan" => {
                    let mut findings: Option<Vec<String>> = None;
                    while let Some(flag) = cursor.next() {
                        match flag.as_str() {
                            "--findings" => {
                                let value = cursor.value_after("--findings")?;
                                let ids: Vec<String> = value
                                    .split(',')
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_string)
                                    .collect();
                                findings = Some(ids);
                            }
                            other => return Err(unknown_flag(other)),
                        }
                    }
                    Command::Service(ServiceJob::OptimizePlan { findings })
                }
                "start" => {
                    let plan_id = parse_optional_id(&mut cursor, "--plan-id")?;
                    Command::Service(ServiceJob::OptimizeStart { plan_id })
                }
                "status" => {
                    let plan_id = parse_optional_id(&mut cursor, "--plan-id")?;
                    Command::Service(ServiceJob::OptimizeStatus { plan_id })
                }
                other => {
                    return Err(usage(
                        "cli.usage.unknownSubcommand",
                        format!("unknown optimize subcommand '{other}' ({OPTIMIZE_SUBS})"),
                    ));
                }
            }
        }
        "timeline" => {
            let sub = cursor.next().cloned().ok_or_else(|| {
                usage(
                    "cli.usage.timelineSubcommandRequired",
                    TIMELINE_SUBS.to_string(),
                )
            })?;
            match sub.as_str() {
                "page" => {
                    let mut page_size = 50u32;
                    let mut before_sequence = 0u64;
                    while let Some(flag) = cursor.next() {
                        match flag.as_str() {
                            "--size" => page_size = parse_u32(&mut cursor, "--size", 1, 200)?,
                            "--before" => {
                                let value = cursor.value_after("--before")?;
                                before_sequence = value.parse().map_err(|_| {
                                    usage(
                                        "cli.usage.beforeInvalid",
                                        format!(
                                            "--before expects an unsigned integer, got '{value}'"
                                        ),
                                    )
                                })?;
                            }
                            other => return Err(unknown_flag(other)),
                        }
                    }
                    Command::Service(ServiceJob::TimelinePage {
                        page_size,
                        before_sequence,
                    })
                }
                "patterns" => Command::Service(ServiceJob::TimelinePatterns),
                other => {
                    return Err(usage(
                        "cli.usage.unknownSubcommand",
                        format!("unknown timeline subcommand '{other}' ({TIMELINE_SUBS})"),
                    ));
                }
            }
        }
        "care" => {
            let sub = cursor
                .next()
                .cloned()
                .ok_or_else(|| usage("cli.usage.careSubcommandRequired", CARE_SUBS.to_string()))?;
            match sub.as_str() {
                "status" => Command::Service(ServiceJob::CareStatus),
                "start" => {
                    let mut non_interactive = false;
                    while let Some(flag) = cursor.next() {
                        match flag.as_str() {
                            "--non-interactive" => non_interactive = true,
                            other => return Err(unknown_flag(other)),
                        }
                    }
                    Command::Service(ServiceJob::CareStart { non_interactive })
                }
                "cancel" => Command::Service(ServiceJob::CareCancel),
                "consent-grant" => Command::Service(ServiceJob::CareConsentGrant),
                other => {
                    return Err(usage(
                        "cli.usage.unknownSubcommand",
                        format!("unknown care subcommand '{other}' ({CARE_SUBS})"),
                    ));
                }
            }
        }
        "insights" => {
            let sub = cursor.next().cloned().ok_or_else(|| {
                usage(
                    "cli.usage.insightsSubcommandRequired",
                    INSIGHTS_SUBS.to_string(),
                )
            })?;
            match sub.as_str() {
                "list" => Command::Service(ServiceJob::InsightsList),
                "explain" => {
                    let mut question_key = "insight.question.overview".to_string();
                    while let Some(flag) = cursor.next() {
                        match flag.as_str() {
                            "--question" => question_key = cursor.value_after("--question")?,
                            other => return Err(unknown_flag(other)),
                        }
                    }
                    Command::Service(ServiceJob::InsightsExplain { question_key })
                }
                "dismiss" => {
                    let mut insight_id: Option<String> = None;
                    while let Some(flag) = cursor.next() {
                        match flag.as_str() {
                            "--insight-id" => {
                                insight_id = Some(cursor.value_after("--insight-id")?)
                            }
                            other => return Err(unknown_flag(other)),
                        }
                    }
                    let insight_id = insight_id.ok_or_else(|| {
                        usage(
                            "cli.usage.insightIdRequired",
                            "insights dismiss requires --insight-id <id>".to_string(),
                        )
                    })?;
                    Command::Service(ServiceJob::InsightsDismiss { insight_id })
                }
                other => {
                    return Err(usage(
                        "cli.usage.unknownSubcommand",
                        format!("unknown insights subcommand '{other}' ({INSIGHTS_SUBS})"),
                    ));
                }
            }
        }
        "scan" => {
            let sub = cursor
                .next()
                .cloned()
                .ok_or_else(|| usage("cli.usage.scanSubcommandRequired", SCAN_SUBS.to_string()))?;
            match sub.as_str() {
                "start" => Command::Service(ServiceJob::ScanStart),
                "cancel" => {
                    let mut scan_id: Option<String> = None;
                    while let Some(flag) = cursor.next() {
                        match flag.as_str() {
                            "--scan-id" => scan_id = Some(cursor.value_after("--scan-id")?),
                            other => return Err(unknown_flag(other)),
                        }
                    }
                    let scan_id = scan_id.ok_or_else(|| {
                        usage(
                            "cli.usage.scanIdRequired",
                            "scan cancel requires --scan-id <id>".to_string(),
                        )
                    })?;
                    Command::Service(ServiceJob::ScanCancel { scan_id })
                }
                "status" => Command::Service(ServiceJob::ScanStatus),
                "history" => {
                    let mut limit = 20u32;
                    while let Some(flag) = cursor.next() {
                        match flag.as_str() {
                            "--limit" => limit = parse_u32(&mut cursor, "--limit", 1, 200)?,
                            other => return Err(unknown_flag(other)),
                        }
                    }
                    Command::Service(ServiceJob::ScanHistory { limit })
                }
                other => {
                    return Err(usage(
                        "cli.usage.unknownSubcommand",
                        format!("unknown scan subcommand '{other}' ({SCAN_SUBS})"),
                    ));
                }
            }
        }
        other => {
            return Err(usage(
                "cli.usage.unknownCommand",
                format!("unknown command '{other}'"),
            ));
        }
    };

    if cursor.peek().is_some() {
        return Err(usage(
            "cli.usage.unexpectedPositional",
            format!(
                "unexpected argument '{}'",
                cursor.peek().unwrap_or(&String::new())
            ),
        ));
    }

    Ok(Invocation {
        config,
        command,
        version_requested,
        lang: crate::i18n::resolve(lang_flag.map(|l| match l {
            crate::i18n::Lang::Ar => "ar",
            _ => "en",
        })),
    })
}

const PERF_SUBS: &str = "start|stop|snapshot|report";
const OPTIMIZE_SUBS: &str = "plan|start|status";
const TIMELINE_SUBS: &str = "page|patterns";
const CARE_SUBS: &str = "status|start|cancel|consent-grant";
const INSIGHTS_SUBS: &str = "list|explain|dismiss";
const SCAN_SUBS: &str = "start|cancel|status|history";

fn unknown_flag(flag: &str) -> CliError {
    usage("cli.usage.unknownFlag", format!("unknown flag '{flag}'"))
}

fn parse_optional_id(cursor: &mut Cursor, flag: &str) -> Result<Option<String>, CliError> {
    if cursor.peek() == Some(&flag.to_string()) {
        let _ = cursor.next();
        return Ok(Some(cursor.value_after(flag)?));
    }
    Ok(None)
}

/// Phase 29 (T3): signed 64-bit integer parser for --from-ms/--to-ms bounds.
fn parse_i64(cursor: &mut Cursor, flag: &str) -> Result<i64, CliError> {
    let raw = cursor.value_after(flag)?;
    raw.parse::<i64>().map_err(|_| {
        usage(
            "cli.usage.integerInvalid",
            format!("{flag} expects a signed integer, got '{raw}'"),
        )
    })
}

fn parse_u32(cursor: &mut Cursor, flag: &str, min: u32, max: u32) -> Result<u32, CliError> {
    let raw = cursor.value_after(flag)?;
    let parsed: u32 = raw.parse().map_err(|_| {
        usage(
            &format!("cli.usage.flag{}Invalid", flag.trim_start_matches('-')),
            format!("{flag} expects an integer in [{min}, {max}], got '{raw}'"),
        )
    })?;
    if !(min..=max).contains(&parsed) {
        return Err(usage(
            &format!("cli.usage.flag{}OutOfRange", flag.trim_start_matches('-')),
            format!("{flag} must be in [{min}, {max}], got {parsed}"),
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(items: &[&str]) -> Vec<String> {
        std::iter::once("aetherctl".to_string())
            .chain(items.iter().map(|s| s.to_string()))
            .collect()
    }

    #[test]
    fn parses_global_flags_before_command_only() {
        let invocation = parse(&argv(&[
            "--socket-dir",
            "/tmp/x",
            "--timeout-ms",
            "5000",
            "--output",
            "json",
            "--no-color",
            "capabilities",
        ]))
        .unwrap();
        assert_eq!(invocation.config.timeout, Duration::from_millis(5000));
        assert!(matches!(invocation.config.output, OutputMode::Json));
        assert!(matches!(
            invocation.command,
            Command::Offline(OfflineJob::Capabilities)
        ));
    }

    #[test]
    fn global_flag_after_command_is_usage_error() {
        let error = parse(&argv(&["capabilities", "--output", "json"])).unwrap_err();
        assert_eq!(error.exit_code(), crate::exit::ExitCode::Usage);
    }

    #[test]
    fn unknown_command_is_typed_usage() {
        let error = parse(&argv(&["frobnicate"])).unwrap_err();
        assert_eq!(error.exit_code(), crate::exit::ExitCode::Usage);
        assert!(error.message_key().starts_with("cli.usage."));
    }

    #[test]
    fn timeout_must_be_positive_integer() {
        assert!(parse(&argv(&["--timeout-ms", "abc", "about"])).is_err());
        assert!(parse(&argv(&["--timeout-ms", "0", "about"])).is_err());
    }

    #[test]
    fn care_start_parses_non_interactive() {
        let invocation = parse(&argv(&["care", "start", "--non-interactive"])).unwrap();
        assert!(matches!(
            invocation.command,
            Command::Service(ServiceJob::CareStart {
                non_interactive: true
            })
        ));
    }

    #[test]
    fn dismiss_requires_insight_id() {
        assert!(parse(&argv(&["insights", "dismiss"])).is_err());
        assert!(parse(&argv(&["insights", "dismiss", "--insight-id", "i1"])).is_ok());
    }

    #[test]
    fn phase33_compliance_audit_parses_without_explicit_targets() {
        let invocation = parse(&argv(&[
            "sec",
            "audit",
            "--profile",
            "cis-l1",
            "--out",
            "/tmp/report.json",
            "--format",
            "both",
        ]))
        .unwrap();
        assert!(matches!(
            invocation.command,
            Command::Offline(OfflineJob::ComplianceAudit { ref profile, ref format, ref targets, .. })
                if profile == "cis-l1" && format == "both" && targets.is_empty()
        ));
    }

    #[test]
    fn phase33_sign_requires_explicit_key() {
        assert!(
            parse(&argv(&[
                "sec",
                "audit",
                "--profile",
                "cis-l1",
                "--out",
                "/tmp/r.json",
                "--sign",
            ]))
            .is_err()
        );
    }

    #[test]
    fn phase33_format_is_closed_enum() {
        assert!(
            parse(&argv(&[
                "sec",
                "audit",
                "--profile",
                "cis-l1",
                "--out",
                "/tmp/r.json",
                "--format",
                "pdf",
            ]))
            .is_err()
        );
    }

    #[test]
    fn phase33_offline_verify_parses() {
        let invocation = parse(&argv(&["compliance", "verify", "/tmp/report.json"])).unwrap();
        assert!(matches!(
            invocation.command,
            Command::Offline(OfflineJob::ComplianceVerify { ref file }) if file == "/tmp/report.json"
        ));
    }
}
