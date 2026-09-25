//! Phase 31 (W6) — aetherctl typed i18n catalog: EN + AR.
//!
//! Selection order: `--lang` flag → `AETHERCORE_LANG` env → "en" fallback.
//! Arabic is RTL-safe plain text; technical terms (flags, commands, paths) stay
//! English inside backticks. Parity is enforced programmatically by the phase31
//! audit gate `p31-cli-i18n-parity`.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    En,
    Ar,
}

impl Lang {
    pub fn parse(value: &str) -> Option<Lang> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ar" | "ar-eg" | "ara" => Some(Lang::Ar),
            "en" | "en-us" => Some(Lang::En),
            _ => None,
        }
    }
}

/// Resolves the active language: flag > env > en.
pub fn resolve(flag: Option<&str>) -> Lang {
    if let Some(f) = flag
        && let Some(l) = Lang::parse(f)
    {
        return l;
    }
    if let Ok(env) = std::env::var("AETHERCORE_LANG")
        && let Some(l) = Lang::parse(&env)
    {
        return l;
    }
    Lang::En
}

/// Every user-facing CLI string. Keys are stable identifiers; both catalogs must
/// define every key (parity gate).
pub const KEYS: &[&str] = &[
    "usage.header",
    "usage.globalFlags",
    "usage.offlineCommands",
    "usage.serviceCommands",
    "err.unknownCommand",
    "err.unknownFlag",
    "err.commandRequired",
    "err.subcommandRequired",
    "err.flagNeedsValue",
    "err.unexpectedPositional",
    "err.daemonUnreachable",
    "err.staleEndpoint",
    "err.timeout",
    "err.rejectedByService",
    "err.consentRequired",
    "err.protocolViolation",
    "err.localIo",
    "err.exportSubcommandRequired",
    "err.exportRequiresOut",
    "err.exportVerifyRequiresFile",
    "err.keysSubcommandRequired",
    "err.keysGenerateRequiresOut",
    "err.keysFingerprintRequiresIn",
    "err.dbSubcommandRequired",
    "err.dbCheckRequiresSqlite",
    "err.integerInvalid",
    "err.usage",
    "ok.exportWritten",
    "ok.verified",
    "ok.keysGenerated",
    "label.platform",
    "label.engineSource",
    "label.capabilities",
    "label.recordCount",
    "label.signed",
    "label.digest",
    "label.fingerprint",
    "state.native",
    "state.degraded",
    "state.notAvailable",
    "detect.online",
    "detect.staleEndpoint",
    // Phase 32: security-audit surface.
    "sec.auditHeader",
    "sec.lane.ok",
    "sec.lane.notAvailable",
    "sec.findingsCount",
    "sec.traversalRejected",
    "sec.invalidTargets",
    "sec.reportInvalid",
    "sec.cisMapInvalid",
    "sec.vulndbInvalidCandidate",
    "sec.vulndbManifestWrite",
    "sec.complianceProfile",
    "sec.controlsPass",
    "sec.controlsFail",
    "sec.controlsNotRun",
    "sec.controlsUnmapped",
    "compliance.profile.cisL1",
    "compliance.profile.cisL2",
    "compliance.help",
    "compliance.family.general",
    "compliance.family.ssh",
    "compliance.family.access",
    "compliance.family.filesystem",
    "compliance.family.secrets",
    "compliance.family.auth",
    "compliance.finding",
    "compliance.not_verified",
    "compliance.verified",
    "compliance.not_applicable",
    "cli.usage.compliance",
    "cli.usage.complianceProfile",
    "cli.usage.complianceFormat",
    "cli.usage.complianceSigning",
    "cli.usage.complianceRequiresInputs",
    "sec.signatureMismatch",
    // Phase 32 finding summary keys (renderer-facing; plural forms included).
    "sec.ssh.permitRootLogin",
    "sec.ssh.permitRootLogin.plural",
    "sec.ssh.passwordAuthentication",
    "sec.ssh.pubkeyAuthentication",
    "sec.ssh.x11Forwarding",
    "sec.ssh.maxAuthTries",
    "sec.ssh.clientAlive",
    "sec.pass.maxDays",
    "sec.pass.minDays",
    "sec.pass.minLen",
    "sec.sudo.noPasswd",
    "sec.sudo.noPasswd.plural",
    "sec.sudo.wildcardAll",
    "sec.fs.worldWritable",
    "sec.fs.worldWritable.plural",
    "sec.fs.suidInventory",
    "sec.fs.suidInventory.plural",
    "sec.fs.sshDirPerms",
    "sec.fs.sshKeyPerms",
    "sec.fs.scanTruncated",
    "sec.auth.failureBurst",
    "sec.fw.configPresent",
    "sec.fw.notAvailable",
    "sec.secrets.awsKey",
    "sec.secrets.awsKey.plural",
    "sec.secrets.privateKey",
    "sec.secrets.privateKey.plural",
    "sec.secrets.genericAssignment",
    "sec.secrets.dotenv",
    "sec.secrets.scanTruncated",
    "sec.cve.vulnerablePackage",
    "sec.cve.vulnerablePackage.plural",
    // Phase 34: fleet surface.
    "fleet.rejected",
    "fleet.noHosts",
    "fleet.unknownHost",
    "fleet.unknownSchedule",
    "fleet.unknownProfile",
    "fleet.schedulesInvalid",
    "fleet.scheduleRunDue",
    "fleet.help",
];

/// EN catalog — keyed by the same stable keys.
pub fn en(key: &str) -> &'static str {
    match key {
        "usage.header" => "aetherctl -- AetherCore headless command surface",
        "usage.globalFlags" => "GLOBAL FLAGS (before the command):",
        "usage.offlineCommands" => "OFFLINE COMMANDS (no service required, strictly read-only):",
        "usage.serviceCommands" => "SERVICE COMMANDS (require the maintenance-service endpoint):",
        "err.unknownCommand" => "unknown command",
        "err.unknownFlag" => "unknown flag",
        "err.commandRequired" => "a command is required (see `help`)",
        "err.subcommandRequired" => "a subcommand is required",
        "err.flagNeedsValue" => "flag requires a value",
        "err.unexpectedPositional" => "unexpected argument",
        "err.daemonUnreachable" => {
            "maintenance service not reachable — start it with \
`aethercore-maintenance-service --foreground`"
        }
        "err.staleEndpoint" => {
            "stale endpoint detected (previous daemon did not clean up); \
retry after removing the socket file"
        }
        "err.timeout" => "request timed out",
        "err.rejectedByService" => "rejected by service policy",
        "err.consentRequired" => "consent required for this operation",
        "err.protocolViolation" => "unexpected response shape from the service",
        "err.localIo" => "local file operation failed",
        "err.exportSubcommandRequired" => "`export` requires `journal` or `verify`",
        "err.exportRequiresOut" => "`export journal` requires `--out <file>`",
        "err.exportVerifyRequiresFile" => "`export verify` requires a file path",
        "err.keysSubcommandRequired" => "`keys` requires `generate` or `fingerprint`",
        "err.keysGenerateRequiresOut" => "`keys generate` requires `--out <path>`",
        "err.keysFingerprintRequiresIn" => "`keys fingerprint` requires `--in <path>`",
        "err.dbSubcommandRequired" => {
            "`db` requires `check`, `check-config`, `slow-log` \
or `report`"
        }
        "err.dbCheckRequiresSqlite" => "`db check` requires `--sqlite <file>`",
        "err.integerInvalid" => "expects a signed integer",
        "err.usage" => "invalid command line",
        "ok.exportWritten" => "export written",
        "ok.verified" => "chain verified",
        "ok.keysGenerated" => "signing key generated (seed 0600)",
        "label.platform" => "platform",
        "label.engineSource" => "engine source",
        "label.capabilities" => "capabilities",
        "label.recordCount" => "records",
        "label.signed" => "signed",
        "label.digest" => "digest",
        "label.fingerprint" => "public-key fingerprint",
        "state.native" => "native",
        "state.degraded" => "degraded",
        "state.notAvailable" => "not available",
        "detect.online" => "maintenance service reachable",
        "detect.staleEndpoint" => "stale endpoint detected",
        "sec.auditHeader" => "security audit report (read-only lanes)",
        "sec.lane.ok" => "ok",
        "sec.lane.notAvailable" => "not available",
        "sec.findingsCount" => "findings",
        "sec.traversalRejected" => "path traversal rejected",
        "sec.invalidTargets" => "invalid audit targets",
        "sec.reportInvalid" => "report file is not a valid security audit report",
        "sec.cisMapInvalid" => "CIS map asset is invalid or malformed",
        "sec.vulndbInvalidCandidate" => "candidate vulnerability DB failed validation",
        "sec.vulndbManifestWrite" => "failed to write vulndb manifest pin",
        "sec.complianceProfile" => "compliance profile",
        "sec.controlsPass" => "pass",
        "sec.controlsFail" => "fail",
        "sec.controlsNotRun" => "not run",
        "sec.controlsUnmapped" => "unmapped",
        "sec.ssh.permitRootLogin" => "sshd permits direct root login",
        "sec.ssh.permitRootLogin.plural" => "sshd root-login findings",
        "sec.ssh.passwordAuthentication" => "sshd allows password authentication",
        "sec.ssh.pubkeyAuthentication" => "sshd disables public-key authentication",
        "sec.ssh.x11Forwarding" => "sshd enables X11 forwarding",
        "sec.ssh.maxAuthTries" => "sshd authentication attempt limit too high",
        "sec.ssh.clientAlive" => "sshd client-alive policy missing or out of range",
        "sec.pass.maxDays" => "password maximum age exceeds policy",
        "sec.pass.minDays" => "password minimum age below policy",
        "sec.pass.minLen" => "minimum password length below policy",
        "sec.sudo.noPasswd" => "NOPASSWD sudo grant present",
        "sec.sudo.noPasswd.plural" => "NOPASSWD sudo grants present",
        "sec.sudo.wildcardAll" => "unrestricted ALL=(ALL:ALL) sudo grant",
        "sec.fs.worldWritable" => "world-writable file outside sanctioned sets",
        "sec.fs.worldWritable.plural" => "world-writable files outside sanctioned sets",
        "sec.fs.suidInventory" => "SUID binary inventoried",
        "sec.fs.suidInventory.plural" => "SUID binaries inventoried",
        "sec.fs.sshDirPerms" => ".ssh directory permissions too open",
        "sec.fs.sshKeyPerms" => "SSH key material permissions too open",
        "sec.fs.scanTruncated" => "filesystem scan hit its bounds; coverage partial",
        "sec.auth.failureBurst" => "authentication failure burst suspected",
        "sec.fw.configPresent" => "host firewall configuration file present",
        "sec.fw.notAvailable" => "firewall state not available without elevation",
        "sec.secrets.awsKey" => "AWS access key material found",
        "sec.secrets.awsKey.plural" => "AWS access key materials found",
        "sec.secrets.privateKey" => "private key block found",
        "sec.secrets.privateKey.plural" => "private key blocks found",
        "sec.secrets.genericAssignment" => "high-entropy credential assignment found",
        "sec.secrets.dotenv" => "secret-bearing environment assignment found",
        "sec.secrets.scanTruncated" => "secrets scan hit its bounds; coverage partial",
        "sec.cve.vulnerablePackage" => "installed package matched a known CVE range",
        "sec.cve.vulnerablePackage.plural" => "installed packages matched known CVE ranges",
        "fleet.rejected" => "fleet operation rejected",
        "fleet.noHosts" => "no enabled hosts match the requested scope",
        "fleet.unknownHost" => "unknown fleet host id",
        "fleet.unknownSchedule" => "unknown fleet schedule id",
        "fleet.unknownProfile" => "unknown compliance profile (expected cis-l1 or cis-l2)",
        "fleet.schedulesInvalid" => "fleet schedules file is malformed",
        "fleet.scheduleRunDue" => "executed due fleet schedules (read-only compliance)",
        "fleet.help" => {
            "fleet subcommands manage remote AetherCore hosts: add/list/show/remove inventory, trust pinning, read-only probe/audit/compliance and schedules"
        }
        "compliance.profile.cisL1" => "CIS Level 1 compliance",
        "compliance.profile.cisL2" => "CIS Level 2 compliance",
        "compliance.help" => {
            "`sec audit --profile cis-l1|cis-l2 --out <file> [--format json|html|both] [--sign --key <seedfile>]`; `compliance verify <report.json>`"
        }
        "compliance.family.general" => "General controls",
        "compliance.family.ssh" => "SSH controls",
        "compliance.family.access" => "Access controls",
        "compliance.family.filesystem" => "Filesystem controls",
        "compliance.family.secrets" => "Secret-handling controls",
        "compliance.family.auth" => "Authentication controls",
        "compliance.finding" => "a mapped security finding failed this control",
        "compliance.not_verified" => "evidence not verified",
        "compliance.verified" => "evidence verified",
        "compliance.not_applicable" => "not applicable to this platform",
        "cli.usage.compliance" => "invalid compliance command usage",
        "cli.usage.complianceProfile" => "profile must be `cis-l1` or `cis-l2`",
        "cli.usage.complianceFormat" => "format must be `json`, `html`, or `both`",
        "cli.usage.complianceSigning" => "`--sign` requires an explicit `--key <seedfile>`",
        "cli.usage.complianceRequiresInputs" => "compliance summary requires report and map inputs",
        "sec.signatureMismatch" => "compliance report signature mismatch",
        _ => "",
    }
}

/// AR catalog — professionally worded; technical terms stay English inside backticks.
pub fn ar(key: &str) -> &'static str {
    match key {
        "usage.header" => "`aetherctl` — واجهة الأوامر اللاتفاعلية لـ AetherCore",
        "usage.globalFlags" => "الرايات العامة (قبل الأمر):",
        "usage.offlineCommands" => "أوامر بلا خدمة (للقراءة فقط تماماً):",
        "usage.serviceCommands" => "أوامر الخدمة (تتطلب نقطة نهاية خدمة الصيانة):",
        "err.unknownCommand" => "أمر غير معروف",
        "err.unknownFlag" => "راية غير معروفة",
        "err.commandRequired" => "الأمر مطلوب (انظر `help`)",
        "err.subcommandRequired" => "الأمر الفرعي مطلوب",
        "err.flagNeedsValue" => "الراية تتطلب قيمة",
        "err.unexpectedPositional" => "معامل غير متوقع",
        "err.daemonUnreachable" => {
            "خدمة الصيانة غير قابلة للوصول — شغّلها بـ `aethercore-maintenance-service --foreground`"
        }
        "err.staleEndpoint" => {
            "نقطة نهاية متروكة (daemon سابق لم ينظف)؛ أعد المحاولة بعد حذف ملف الـ socket"
        }
        "err.timeout" => "انتهت مهلة الطلب",
        "err.rejectedByService" => "رفضته سياسة الخدمة",
        "err.consentRequired" => "هذه العملية تتطلب موافقة",
        "err.protocolViolation" => "شكل استجابة غير متوقع من الخدمة",
        "err.localIo" => "فشلت عملية ملف محلي",
        "err.exportSubcommandRequired" => "`export` يتطلب `journal` أو `verify`",
        "err.exportRequiresOut" => "`export journal` يتطلب `--out <file>`",
        "err.exportVerifyRequiresFile" => "`export verify` يتطلب مسار ملف",
        "err.keysSubcommandRequired" => "`keys` يتطلب `generate` أو `fingerprint`",
        "err.keysGenerateRequiresOut" => "`keys generate` يتطلب `--out <path>`",
        "err.keysFingerprintRequiresIn" => "`keys fingerprint` يتطلب `--in <path>`",
        "err.dbSubcommandRequired" => {
            "`db` يتطلب `check` أو `check-config` أو `slow-log` أو `report`"
        }
        "err.dbCheckRequiresSqlite" => "`db check` يتطلب `--sqlite <file>`",
        "err.integerInvalid" => "يتوقع عدداً صحيحاً بإشارة",
        "err.usage" => "سطر أوامر غير صالح",
        "ok.exportWritten" => "تم كتابة التصدير",
        "ok.verified" => "تم التحقق من السلسلة",
        "ok.keysGenerated" => "تم توليد مفتاح التوقيع (بصلاحية 0600)",
        "label.platform" => "المنصة",
        "label.engineSource" => "مصدر المحرك",
        "label.capabilities" => "القدرات",
        "label.recordCount" => "السجلات",
        "label.signed" => "موقّع",
        "label.digest" => "البصمة",
        "label.fingerprint" => "بصمة المفتاح العام",
        "state.native" => "أصلي",
        "state.degraded" => "محدود",
        "state.notAvailable" => "غير متاح",
        "detect.online" => "خدمة الصيانة قابلة للوصول",
        "detect.staleEndpoint" => "يُكتشف endpoint متروك",
        "sec.auditHeader" => "تقرير التدقيق الأمني (مسارات للقراءة فقط)",
        "sec.lane.ok" => "سليم",
        "sec.lane.notAvailable" => "غير متاح",
        "sec.findingsCount" => "النتائج",
        "sec.traversalRejected" => "رُفض مسار يجتاز المجلدات (`..`)",
        "sec.invalidTargets" => "أهداف تدقيق غير صالحة",
        "sec.reportInvalid" => "ملف التقرير ليس تقرير تدقيق أمني صالحاً",
        "sec.cisMapInvalid" => "أصل خريطة CIS غير صالح أو تالف",
        "sec.vulndbInvalidCandidate" => "فشل التحقق من قاعدة الثغرات المرشحة",
        "sec.vulndbManifestWrite" => "تعذّرت كتابة بصمة manifest لقاعدة الثغرات",
        "sec.complianceProfile" => "ملف الامتثال",
        "sec.controlsPass" => "ناجح",
        "sec.controlsFail" => "راسب",
        "sec.controlsNotRun" => "لم يُنفذ",
        "sec.controlsUnmapped" => "غير مربوط",
        "sec.ssh.permitRootLogin" => "`sshd` يسمح بدخول root المباشر",
        "sec.ssh.permitRootLogin.plural" => "نتائج دخول root عبر `sshd`",
        "sec.ssh.passwordAuthentication" => "`sshd` يسمح بالمصادقة بكلمة المرور",
        "sec.ssh.pubkeyAuthentication" => "`sshd` يعطّل المصادقة بالمفتاح العام",
        "sec.ssh.x11Forwarding" => "`sshd` يفعّل إعادة توجيه X11",
        "sec.ssh.maxAuthTries" => "حد محاولات المصادقة في `sshd` مرتفع جداً",
        "sec.ssh.clientAlive" => "سياسة `ClientAlive` مفقودة أو خارج النطاق",
        "sec.pass.maxDays" => "العمر الأقصى لكلمة المرور يتجاوز السياسة",
        "sec.pass.minDays" => "العمر الأدنى لكلمة المرور أقل من السياسة",
        "sec.pass.minLen" => "الحد الأدنى لطول كلمة المرور أقل من السياسة",
        "sec.sudo.noPasswd" => "إذن `NOPASSWD` في sudoers موجود",
        "sec.sudo.noPasswd.plural" => "أذونات `NOPASSWD` متعددة موجودة",
        "sec.sudo.wildcardAll" => "إذن `ALL=(ALL:ALL)` غير مقيّد",
        "sec.fs.worldWritable" => "ملف قابل للكتابة للجميع خارج المجموعات المعتمدة",
        "sec.fs.worldWritable.plural" => "ملفات قابلة للكتابة للجميع خارج المجموعات المعتمدة",
        "sec.fs.suidInventory" => "ثنائي SUID مدرج ضمن الجرد",
        "sec.fs.suidInventory.plural" => "ثنائيات SUID مدرجة ضمن الجرد",
        "sec.fs.sshDirPerms" => "صلاحيات مجلد `.ssh` واسعة أكثر من اللازم",
        "sec.fs.sshKeyPerms" => "صلاحيات مفاتيح SSH واسعة أكثر من اللازم",
        "sec.fs.scanTruncated" => "بلغ فحص نظام الملفات حدوده؛ التغطية جزئية",
        "sec.auth.failureBurst" => "اشتباه بموجة فشل مصادقة",
        "sec.fw.configPresent" => "ملف إعداد الجدار الناري موجود على المضيف",
        "sec.fw.notAvailable" => "حالة الجدار الناري غير متاحة دون صلاحيات مرتفعة",
        "sec.secrets.awsKey" => "عُثر على مفتاح وصول AWS",
        "sec.secrets.awsKey.plural" => "عُثر على مفاتيح وصول AWS",
        "sec.secrets.privateKey" => "عُثر على كتلة مفتاح خاص",
        "sec.secrets.privateKey.plural" => "عُثر على كتل مفاتيح خاصة",
        "sec.secrets.genericAssignment" => "عُثر على إسناد اعتماد عالي الإنتروبيا",
        "sec.secrets.dotenv" => "عُثر على إسناد بيئة يحتوي سراً",
        "sec.secrets.scanTruncated" => "بلغ فحص الأسرار حدوده؛ التغطية جزئية",
        "sec.cve.vulnerablePackage" => "حزمة مثبتة تطابق نطاق CVE معروف",
        "sec.cve.vulnerablePackage.plural" => "حزم مثبتة تطابق نطاقات CVE معروفة",
        "fleet.rejected" => "تم رفض عملية الأسطول",
        "fleet.noHosts" => "لا يوجد مضيفون مفعّلون مطابقون للنطاق المطلوب",
        "fleet.unknownHost" => "معرّف مضيف أسطول غير معروف",
        "fleet.unknownSchedule" => "معرّف جدولة أسطول غير معروف",
        "fleet.unknownProfile" => "ملف توافق غير معروف (المتوقع cis-l1 أو cis-l2)",
        "fleet.schedulesInvalid" => "ملف جدولة الأسطول تالف",
        "fleet.scheduleRunDue" => "تم تنفيذ جدولات الأسطول المستحقة (توافق للقراءة فقط)",
        "fleet.help" => {
            "تدير أوامر الأسطول المضيفين البعيدين لـ AetherCore: قائمة الجرد، تثبيت الثقة، الفحص/التدقيق/التوافق للقراءة فقط، والجدولة"
        }
        "compliance.profile.cisL1" => "امتثال CIS المستوى 1",
        "compliance.profile.cisL2" => "امتثال CIS المستوى 2",
        "compliance.help" => {
            "`sec audit --profile cis-l1|cis-l2 --out <file> [--format json|html|both] [--sign --key <seedfile>]`؛ و`compliance verify <report.json>`"
        }
        "compliance.family.general" => "ضوابط عامة",
        "compliance.family.ssh" => "ضوابط SSH",
        "compliance.family.access" => "ضوابط الوصول",
        "compliance.family.filesystem" => "ضوابط نظام الملفات",
        "compliance.family.secrets" => "ضوابط التعامل مع الأسرار",
        "compliance.family.auth" => "ضوابط المصادقة",
        "compliance.finding" => "أخفقت نتيجة أمنية مربوطة هذا الضابط",
        "compliance.not_verified" => "لم يُتحقق من الدليل",
        "compliance.verified" => "تم التحقق من الدليل",
        "compliance.not_applicable" => "لا ينطبق على هذه المنصة",
        "cli.usage.compliance" => "استخدام غير صالح لأمر الامتثال",
        "cli.usage.complianceProfile" => "يجب أن يكون الملف `cis-l1` أو `cis-l2`",
        "cli.usage.complianceFormat" => "يجب أن تكون الصيغة `json` أو `html` أو `both`",
        "cli.usage.complianceSigning" => "يتطلب `--sign` مفتاحًا صريحًا عبر `--key <seedfile>`",
        "cli.usage.complianceRequiresInputs" => "يتطلب ملخص الامتثال ملف التقرير والخريطة",
        "sec.signatureMismatch" => "عدم تطابق توقيع تقرير الامتثال",
        _ => "",
    }
}

/// Translates a key in the given language (empty string when unknown — callers fall
/// back to the key itself so nothing silently masquerades).
pub fn t(lang: Lang, key: &str) -> &'static str {
    match lang {
        Lang::En => en(key),
        Lang::Ar => ar(key),
    }
}

/// Usage-block headings, localized by `cli::usage_text`.
pub const USAGE_HEADINGS: [&str; 4] = [
    "usage.header",
    "usage.globalFlags",
    "usage.offlineCommands",
    "usage.serviceCommands",
];

/// Catalog text for a failure, most specific first: its own message key, the `err.*`
/// twin of a `cli.usage.*` key, then its kind. Never empty.
pub fn error_text(lang: Lang, error: &crate::error::CliError) -> &'static str {
    use crate::error::CliError as E;
    let key = error.message_key();
    let twin = key.strip_prefix("cli.usage.").map(|name| format!("err.{name}"));
    let needs_value = key.starts_with("cli.usage.flag") && key.ends_with("RequiresValue");
    let kind_key = match error {
        E::Usage { .. } if needs_value => "err.flagNeedsValue",
        E::Usage { .. } => "err.usage",
        E::ServiceUnreachable { message_key } if message_key == "cli.detect.staleEndpoint" => {
            "err.staleEndpoint"
        }
        E::ServiceUnreachable { .. } => "err.daemonUnreachable",
        E::Timeout => "err.timeout",
        E::Rejected { .. } => "err.rejectedByService",
        E::ConsentRequired { .. } => "err.consentRequired",
        E::CapabilityUnavailable { .. } => "state.notAvailable",
        E::LocalIo { .. } => "err.localIo",
        E::ProtocolViolation { .. } => "err.protocolViolation",
    };
    [Some(key), twin, Some(kind_key.to_string())]
        .into_iter()
        .flatten()
        .map(|k| t(lang, &k))
        .find(|text| !text.is_empty())
        .unwrap_or_default()
}

/// Catalog key of a data field that has a human label.
fn field_key(field: &str) -> Option<&'static str> {
    Some(match field {
        "platform" => "label.platform",
        "source" => "label.engineSource",
        "capabilities" => "label.capabilities",
        "recordCount" => "label.recordCount",
        "signed" => "label.signed",
        "digest" => "label.digest",
        "publicKeyFingerprint" | "signerFingerprint" => "label.fingerprint",
        _ => return None,
    })
}

/// Human label for a data field; the JSON name when the catalog has none.
pub fn field_label(lang: Lang, field: &str) -> String {
    field_key(field)
        .map(|key| t(lang, key).to_string())
        .unwrap_or_else(|| field.to_string())
}

/// Human text for a string value: catalog keys carried as data (finding summaries,
/// compliance reasons, detection states, availability states) are translated; any
/// other value is data and is shown verbatim.
pub fn value_text(lang: Lang, value: &str) -> String {
    let key = match value {
        "native" => "state.native",
        "degraded" => "state.degraded",
        "notAvailable" => "state.notAvailable",
        "cli.detect.reachable" => "detect.online",
        "cli.detect.staleEndpoint" => "detect.staleEndpoint",
        other => other,
    };
    match t(lang, key) {
        "" => value.to_string(),
        text => text.to_string(),
    }
}

/// One-line outcome printed above a command's data, where the catalog has one.
pub fn outcome_text(lang: Lang, command: &str) -> Option<&'static str> {
    let key = match command {
        "export journal" => "ok.exportWritten",
        "export verify" => "ok.verified",
        "keys generate" => "ok.keysGenerated",
        _ => return None,
    };
    Some(t(lang, key))
}

/// Parity helper used by the audit and tests: true when every KEYS entry resolves
/// non-empty in BOTH catalogs.
pub fn parity_ok() -> bool {
    KEYS.iter().all(|k| !en(k).is_empty() && !ar(k).is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn en_ar_parity_over_all_keys() {
        assert!(KEYS.len() >= 40);
        assert!(parity_ok(), "every key must exist in both catalogs");
    }

    #[test]
    fn selection_order_flag_env_fallback() {
        // env unset → fallback en (edition-2024 marks remove_var unsafe in tests too).
        #[allow(unused_unsafe)]
        unsafe {
            std::env::remove_var("AETHERCORE_LANG");
        }
        assert_eq!(resolve(Some("ar")), Lang::Ar);
        assert_eq!(resolve(Some("bogus")), Lang::En);
        assert_eq!(resolve(None), Lang::En);
    }

    #[test]
    fn unknown_key_returns_empty_not_panic() {
        assert_eq!(t(Lang::En, "nope.nope"), "");
        assert_eq!(t(Lang::Ar, "nope.nope"), "");
    }

    #[test]
    fn every_error_kind_has_text_in_both_languages() {
        use crate::error::CliError;
        let errors = [
            CliError::usage("cli.usage.exportRequiresOut"),
            CliError::usage("cli.usage.flagoutRequiresValue"),
            CliError::usage("cli.usage.somethingUncatalogued"),
            CliError::service_unreachable("cli.detect.offline"),
            CliError::service_unreachable("cli.detect.staleEndpoint"),
            CliError::Timeout,
            CliError::Rejected {
                message_key: "sec.traversalRejected".into(),
                detail: None,
            },
            CliError::ConsentRequired {
                message_key: "cli.consent.declined".into(),
            },
            CliError::capability_unavailable("x"),
            CliError::local_io("local.io.read"),
            CliError::ProtocolViolation { detail: "x".into() },
        ];
        for error in &errors {
            assert!(!error_text(Lang::En, error).is_empty(), "{error:?}");
            assert!(!error_text(Lang::Ar, error).is_empty(), "{error:?}");
        }
        assert_eq!(error_text(Lang::Ar, &errors[0]), ar("err.exportRequiresOut"));
        assert_eq!(error_text(Lang::Ar, &errors[1]), ar("err.flagNeedsValue"));
        assert_eq!(error_text(Lang::Ar, &errors[4]), ar("err.staleEndpoint"));
        assert_eq!(error_text(Lang::Ar, &errors[6]), ar("sec.traversalRejected"));
    }

    #[test]
    fn usage_headings_are_found_and_translated() {
        let arabic = crate::cli::usage_text(Lang::Ar);
        for key in USAGE_HEADINGS {
            assert!(crate::cli::USAGE.contains(en(key)), "{key} not in USAGE");
            assert!(arabic.contains(ar(key)) && !arabic.contains(en(key)), "{key}");
        }
        assert_eq!(crate::cli::usage_text(Lang::En), crate::cli::USAGE);
    }

    #[test]
    fn data_values_that_are_catalog_keys_are_translated_and_data_is_not() {
        assert_eq!(value_text(Lang::Ar, "sec.ssh.permitRootLogin"), ar("sec.ssh.permitRootLogin"));
        assert_eq!(value_text(Lang::Ar, "compliance.not_verified"), ar("compliance.not_verified"));
        assert_eq!(value_text(Lang::Ar, "notAvailable"), ar("state.notAvailable"));
        assert_eq!(value_text(Lang::Ar, "cli.detect.reachable"), ar("detect.online"));
        assert_eq!(value_text(Lang::Ar, "/etc/ssh/sshd_config"), "/etc/ssh/sshd_config");
        assert_eq!(field_label(Lang::Ar, "recordCount"), ar("label.recordCount"));
        assert_eq!(field_label(Lang::Ar, "file"), "file");
    }

    #[test]
    fn compliance_help_is_present_in_both_languages() {
        assert!(en("compliance.help").contains("sec audit --profile"));
        assert!(ar("compliance.help").contains("sec audit --profile"));
    }
}
