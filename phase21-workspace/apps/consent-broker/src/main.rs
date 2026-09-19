use aethercore_contracts::{
    PROTOCOL_VERSION,
    v1::{self, Request, RequestHeader, request, response},
};
use anyhow::{Context, Result, bail};
use uuid::Uuid;

fn main() -> Result<()> {
    #[cfg(not(windows))]
    bail!("AetherCore consent broker supports Windows only");

    #[cfg(windows)]
    run_windows()
}

#[cfg(windows)]
fn run_windows() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        bail!("unexpected consent broker arguments");
    }
    let intent_id = arg(&args, "--intent-id")?;
    let locale = arg(&args, "--locale")?;
    if locale != "en" && locale != "ar" {
        bail!("invalid display locale");
    }
    if Uuid::parse_str(&intent_id).is_err() {
        bail!("invalid consent intent identifier");
    }

    let intent = fetch_intent(&intent_id)?;
    if !confirm_intent(&intent, &locale)? {
        bail!("consent cancelled by user");
    }

    let response = call(request::Payload::ApproveConsentIntent(
        v1::ApproveConsentIntentRequest {
            intent_id: intent_id.clone(),
        },
    ))?;
    match response.payload {
        Some(response::Payload::ConsentApproval(approval))
            if approval.intent_id == intent_id && approval.plan_id == intent.plan_id =>
        {
            Ok(())
        }
        _ => bail!("unexpected consent approval response"),
    }
}

#[cfg(windows)]
fn fetch_intent(intent_id: &str) -> Result<v1::ConsentIntentResponse> {
    let response = call(request::Payload::GetConsentIntent(
        v1::GetConsentIntentRequest {
            intent_id: intent_id.to_owned(),
        },
    ))?;
    match response.payload {
        Some(response::Payload::ConsentIntent(intent)) if intent.intent_id == intent_id => {
            Ok(intent)
        }
        _ => bail!("unexpected consent intent response"),
    }
}

#[cfg(windows)]
fn call(payload: request::Payload) -> Result<v1::Response> {
    let req = Request {
        header: Some(RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: Uuid::new_v4().to_string(),
        }),
        payload: Some(payload),
    };
    let response = aethercore_ipc::connect(&req).context("connect to maintenance service")?;
    if response.status_code != 0 {
        // Typed ErrorInfo carries the same detail string as the deprecated
        // Response::error_message wire field (proto field 3).
        let detail = response
            .error
            .as_ref()
            .map(|e| e.technical_detail.as_str())
            .unwrap_or("unspecified rejection");
        bail!("consent request rejected: {detail}");
    }
    Ok(response)
}

#[cfg(windows)]
fn confirm_intent(intent: &v1::ConsentIntentResponse, locale: &str) -> Result<bool> {
    use windows::{
        Win32::UI::WindowsAndMessaging::{
            IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_RIGHT, MB_RTLREADING, MB_SETFOREGROUND,
            MB_YESNO, MessageBoxW,
        },
        core::PCWSTR,
    };

    let arabic = locale == "ar" || (locale.is_empty() && user_prefers_arabic_ui());
    let (title, body) = if arabic {
        let operation = localized_operation_ar(&intent.title);
        let risk = localized_risk_ar(intent.risk_code, &intent.risk);
        (
            "AetherCore — موافقة المسؤول".to_owned(),
            format!(
                "يطلب AetherCore موافقة المسؤول على هذه العملية المجمدة:\n\n{operation}\n\nمستوى المخاطر: {risk}\nعدد الإجراءات: {}\n\nستستهلك خدمة الصيانة هذه الموافقة مرة واحدة فقط عند بدء العملية. هل تريد المتابعة؟",
                format_number_ar(intent.action_count as u64),
            ),
        )
    } else {
        (
            "AetherCore — Administrator Consent".to_owned(),
            format!(
                "AetherCore is requesting administrator consent for this frozen operation:\n\n{}\n\nRisk: {}\nActions: {}\n\nThe maintenance service will consume this approval exactly once when the operation begins. Continue?",
                intent.title, intent.risk, intent.action_count
            ),
        )
    };
    let title = wide(&title);
    let body = wide(&body);
    let base_flags = MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2 | MB_SETFOREGROUND;
    let flags = if arabic {
        base_flags | MB_RIGHT | MB_RTLREADING
    } else {
        base_flags
    };
    let result = unsafe { MessageBoxW(None, PCWSTR(body.as_ptr()), PCWSTR(title.as_ptr()), flags) };
    Ok(result == IDYES)
}

#[cfg(windows)]
fn user_prefers_arabic_ui() -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    const LANG_ARABIC: u16 = 0x01;
    const PRIMARY_LANG_MASK: u16 = 0x03ff;
    let lang_id = unsafe { GetUserDefaultUILanguage() };
    (lang_id & PRIMARY_LANG_MASK) == LANG_ARABIC
}

#[cfg(windows)]
fn localized_operation_ar(title: &str) -> String {
    if title.starts_with("Install ") {
        return "تثبيت تعريفات Windows التي تمت مراجعتها".into();
    }
    if title == "Repair Windows integrity" {
        return "إصلاح تكامل Windows وفق الخطة التي تمت مراجعتها".into();
    }
    if title.starts_with("Clean ") {
        return "تنظيف العناصر التي تمت مراجعتها ضمن قائمة السماح".into();
    }
    if title.starts_with("Restore ") {
        return "استعادة تغييرات بدء التشغيل التي تمت مراجعتها".into();
    }
    if title.starts_with("Optimize ") {
        return "تطبيق تغييرات بدء التشغيل التي تمت مراجعتها".into();
    }
    "عملية صيانة محمية تمت مراجعتها".into()
}

#[cfg(windows)]
fn localized_risk_ar(risk_code: i32, fallback: &str) -> &'static str {
    match v1::RiskLevel::try_from(risk_code).ok() {
        Some(v1::RiskLevel::Green) => "منخفض",
        Some(v1::RiskLevel::Amber) => "متوسط ويستلزم الموافقة",
        Some(v1::RiskLevel::Red) => "مرتفع",
        _ if fallback.eq_ignore_ascii_case("low") || fallback.eq_ignore_ascii_case("green") => {
            "منخفض"
        }
        _ if fallback.eq_ignore_ascii_case("high") || fallback.eq_ignore_ascii_case("red") => {
            "مرتفع"
        }
        _ => "متوسط ويستلزم الموافقة",
    }
}

#[cfg(windows)]
fn format_number_ar(value: u64) -> String {
    const DIGITS: [char; 10] = ['٠', '١', '٢', '٣', '٤', '٥', '٦', '٧', '٨', '٩'];
    value
        .to_string()
        .chars()
        .map(|c| c.to_digit(10).map(|d| DIGITS[d as usize]).unwrap_or(c))
        .collect()
}

fn arg(args: &[String], name: &str) -> Result<String> {
    let index = args
        .iter()
        .position(|v| v == name)
        .with_context(|| format!("missing {name}"))?;
    let value = args
        .get(index + 1)
        .cloned()
        .with_context(|| format!("missing value for {name}"))?;
    Ok(value)
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
