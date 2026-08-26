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
    if let Some(f) = flag {
        if let Some(l) = Lang::parse(f) {
            return l;
        }
    }
    if let Ok(env) = std::env::var("AETHERCORE_LANG") {
        if let Some(l) = Lang::parse(&env) {
            return l;
        }
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
}
