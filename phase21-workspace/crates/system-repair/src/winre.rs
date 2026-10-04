//! A narrowly qualified REAgentC configuration observation, never a usable-image guarantee.
use crate::RepairCheck;

pub(crate) const CONFIGURED: &str = "REAgentC reports Windows RE as configured. Recovery-image usability and boot readiness remain unverified; recovery protection is unknown.";
pub(crate) const DISABLED: &str = "REAgentC reports Windows RE as disabled for this installation.";
pub(crate) const UNVERIFIED: &str = "Windows RE configuration could not be verified from a supported REAgentC response; recovery protection is unknown.";

pub(crate) fn from_info(output: &str, exit_code: i32) -> RepairCheck {
    let status = if exit_code == 0 {
        configuration_status(output)
    } else {
        None
    };
    let (result_code, stage, detail) = match status {
        Some("Enabled") => ("WinReConfiguredProtectionUnverified", "Unknown", CONFIGURED),
        Some("Disabled") => ("WinReUnavailable", "Attention", DISABLED),
        _ => ("WinReInfoUnverified", "Unknown", UNVERIFIED),
    };
    RepairCheck {
        id: "winre-state".into(),
        title: "Windows Recovery Environment".into(),
        stage: stage.into(),
        result_code: result_code.into(),
        exit_code,
        detail: detail.into(),
        log_hint: "reagentc.exe /info".into(),
    }
}

fn configuration_status(output: &str) -> Option<&str> {
    // Only the measured en-US frame is admitted. A UI Arabic locale does not qualify an
    // Arabic Windows console format. Lossy/oversized/truncated/foreign frames stay Unknown.
    if output.len() > 48_000 || !output.is_ascii() || output.contains('\0') {
        return None;
    }
    let lines: Vec<_> = output
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if lines.first()? != &"Windows Recovery Environment (Windows RE) and system reset configuration"
        || lines.get(1)? != &"Information:"
        || lines.last()? != &"REAGENTC.EXE: Operation Successful."
    {
        return None;
    }
    let mut fields = std::collections::BTreeMap::new();
    for line in &lines[2..lines.len() - 1] {
        let (key, value) = line.split_once(':')?;
        if !matches!(
            key,
            "Windows RE status"
                | "Windows RE location"
                | "Boot Configuration Data (BCD) identifier"
                | "Recovery image location"
                | "Recovery image index"
                | "Custom image location"
                | "Custom image index"
                | "Windows RE Version"
        ) || fields.insert(key, value.trim()).is_some()
        {
            return None;
        }
    }
    for key in [
        "Windows RE status",
        "Windows RE location",
        "Boot Configuration Data (BCD) identifier",
        "Recovery image location",
        "Recovery image index",
        "Custom image location",
        "Custom image index",
    ] {
        fields.get(key)?;
    }
    for key in ["Recovery image index", "Custom image index"] {
        fields[key].parse::<u32>().ok()?;
    }
    if let Some(version) = fields.get("Windows RE Version")
        && (version.split('.').count() != 4
            || !version
                .split('.')
                .all(|s| !s.is_empty() && s.parse::<u32>().is_ok()))
    {
        return None;
    }
    let id = fields["Boot Configuration Data (BCD) identifier"];
    if id.len() != 36
        || !id.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
    {
        return None;
    }
    match fields["Windows RE status"] {
        "Disabled" => Some("Disabled"),
        "Enabled"
            if id.bytes().any(|b| b.is_ascii_hexdigit() && b != b'0')
                && local_winre_location(fields["Windows RE location"]) =>
        {
            Some("Enabled")
        }
        _ => None,
    }
}

fn local_winre_location(value: &str) -> bool {
    let Some(rest) = value.strip_prefix(r"\\?\GLOBALROOT\device\harddisk") else {
        return false;
    };
    let Some((disk, rest)) = rest.split_once(r"\partition") else {
        return false;
    };
    let Some((partition, suffix)) = rest.split_once('\\') else {
        return false;
    };
    disk.parse::<u32>().is_ok()
        && partition.parse::<u32>().is_ok_and(|p| p > 0)
        && suffix == r"Recovery\WindowsRE"
}

#[cfg(test)]
mod tests {
    use super::*;
    // Measured owner en-US frame, with the local disk/BCD identity replaced by fixture values.
    const INFO: &str = "Windows Recovery Environment (Windows RE) and system reset configuration\nInformation:\n\n    Windows RE status: Enabled\n    Windows RE location: \\\\?\\GLOBALROOT\\device\\harddisk0\\partition4\\Recovery\\WindowsRE\n    Boot Configuration Data (BCD) identifier: 11111111-2222-3333-4444-555555555555\n    Recovery image location:\n    Recovery image index: 0\n    Custom image location:\n    Custom image index: 0\n    Windows RE Version: 10.0.26100.9444\n\nREAGENTC.EXE: Operation Successful.\n";

    #[test]
    fn measured_configured_frame_is_observed_but_never_protection_available() {
        let check = from_info(INFO, 0);
        assert_eq!(check.result_code, "WinReConfiguredProtectionUnverified");
        assert_eq!(check.stage, "Unknown");
        assert_eq!(check.detail, CONFIGURED);
        assert_eq!(
            crate::check_to_fact(&check).unwrap().state,
            crate::FactState::Unknown
        );
        let readiness = crate::recovery_from_checks(&[check]);
        assert_eq!(readiness.win_re, crate::FactState::Unknown);
        // The existing journal recovery is separate protection. This observation cannot add
        // WinRE protection or change the protection predicate from its prior baseline.
        assert_eq!(
            readiness.has_mandatory_protection(),
            crate::recovery_from_checks(&[]).has_mandatory_protection()
        );
    }

    #[test]
    fn recognized_disabled_is_unavailable_without_fabricating_image_availability() {
        let check = from_info(&INFO.replace("status: Enabled", "status: Disabled"), 0);
        assert_eq!(check.result_code, "WinReUnavailable");
        assert_eq!(
            crate::recovery_from_checks(&[check]).win_re,
            crate::FactState::Unavailable
        );
    }

    #[test]
    fn incomplete_mixed_or_unqualified_responses_remain_unknown() {
        for bad in [
            INFO.replace(
                "11111111-2222-3333-4444-555555555555",
                "00000000-0000-0000-0000-000000000000",
            ),
            INFO.replace("11111111-2222-3333-4444-555555555555", "invalid-guid"),
            INFO.replace(
                "Windows RE status: Enabled",
                "Windows RE status: Enabled\nWindows RE status: Disabled",
            ),
            INFO.replace("Windows RE location:", "foreign location:"),
            INFO.replace(
                "\\\\?\\GLOBALROOT\\device\\harddisk0\\partition4\\Recovery\\WindowsRE",
                "\\\\remote\\share\\WindowsRE",
            ),
            INFO.replace("status: Enabled", "status: MaybeEnabled"),
            INFO.replace("Windows RE status: Enabled", "حالة Windows RE: مُمكّن"),
            INFO.replace("REAGENTC.EXE: Operation Successful.", ""),
            INFO.replace("Information:", "extra diagnostic text\nInformation:"),
            INFO.replace("10.0.26100.9444", "invalid.version"),
            INFO.replace("partition4", "partition0"),
            INFO.replace("harddisk0", "harddisk.."),
            "reagentc.exe exists".into(),
            "Enabled".into(),
            INFO.replace("Enabled", "Enabled\u{fffd}"),
        ] {
            assert_eq!(
                from_info(&bad, 0).result_code,
                "WinReInfoUnverified",
                "{bad}"
            );
        }
        assert_eq!(from_info(INFO, 1).result_code, "WinReInfoUnverified");
        assert_eq!(
            from_info(&"x".repeat(48_001), 0).result_code,
            "WinReInfoUnverified"
        );
    }
}
