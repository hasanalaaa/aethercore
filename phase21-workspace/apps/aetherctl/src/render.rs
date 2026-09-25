//! Text rendering + the single exit funnel. JSON and text views derive from the SAME
//! serde_json projection, so the two modes can never disagree. JSON is locale-neutral;
//! text is English ASCII, or Arabic labels and messages under `--lang ar` (numbers are
//! never locale-formatted: no thousands separators, no platform number formatting).

use crate::cli::{Config, OutputMode};
use crate::envelope;
use crate::error::CliError;
use crate::i18n::{self, Lang};
use std::io::Write as _;

/// Renders an aligned ASCII table.
pub fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let widths: Vec<usize> = headers
        .iter()
        .enumerate()
        .map(|(i, header)| {
            rows.iter()
                .map(|row| row.get(i).map(|cell| cell.chars().count()).unwrap_or(0))
                .max()
                .unwrap_or(0)
                .max(header.chars().count())
        })
        .collect();
    let mut out = String::new();
    let line = |cells: &[String]| {
        cells
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let width = widths[i];
                let padded = format!("{cell:<width$}");
                let _ = width;
                padded
            })
            .collect::<Vec<_>>()
            .join("  ")
    };
    let header_cells: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    out.push_str(&line(&header_cells));
    out.push('\n');
    out.push_str(
        &widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("  "),
    );
    out.push('\n');
    for row in rows {
        out.push_str(&line(row));
        out.push('\n');
    }
    out
}

fn scalar_to_text(lang: Lang, value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "-".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => i18n::value_text(lang, s),
        serde_json::Value::Array(items) => {
            if items.is_empty() {
                "[]".to_string()
            } else {
                items
                    .iter()
                    .map(|item| scalar_to_text(lang, item))
                    .collect::<Vec<_>>()
                    .join(",")
            }
        }
        serde_json::Value::Object(_) => "<object>".to_string(),
    }
}

fn key_value_lines(lang: Lang, prefix: &str, value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, inner) in map {
                let key = i18n::field_label(lang, key);
                let label = if prefix.is_empty() {
                    key
                } else {
                    format!("{prefix}.{key}")
                };
                match inner {
                    serde_json::Value::Object(_) => key_value_lines(lang, &label, inner, out),
                    // Arrays of objects render as tables below; scalar arrays inline here.
                    serde_json::Value::Array(items) if is_object_array(items) => {}
                    other => out.push(format!("{:<34} {}", label, scalar_to_text(lang, other))),
                }
            }
        }
        other => out.push(format!("{:<34} {}", prefix, scalar_to_text(lang, other))),
    }
}

fn is_object_array(items: &[serde_json::Value]) -> bool {
    !items.is_empty() && items.iter().all(|item| item.is_object())
}

/// Emits one aligned table per array-of-objects section (deterministic column order =
/// first row's insertion order).
fn table_sections(lang: Lang, value: &serde_json::Value, out: &mut Vec<String>) {
    let serde_json::Value::Object(map) = value else {
        return;
    };
    for (key, inner) in map {
        let serde_json::Value::Array(items) = inner else {
            continue;
        };
        if !is_object_array(items) {
            continue;
        }
        let Some(serde_json::Value::Object(first)) = items.first() else {
            continue;
        };
        let headers: Vec<&str> = first.keys().map(String::as_str).collect();
        let rows: Vec<Vec<String>> = items
            .iter()
            .map(|item| match item {
                serde_json::Value::Object(entry) => headers
                    .iter()
                    .map(|header| {
                        entry
                            .get(*header)
                            .map(|cell| scalar_to_text(lang, cell))
                            .unwrap_or_else(|| "-".to_string())
                    })
                    .collect(),
                _ => Vec::new(),
            })
            .collect();
        let labels: Vec<String> = headers.iter().map(|h| i18n::field_label(lang, h)).collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        out.push(String::new());
        out.push(format!("[{}]", i18n::field_label(lang, key)));
        out.push(table(&labels, &rows));
    }
}

/// Human view of a command's data projection: aligned key/value block plus a real
/// ASCII table for every array section. Deterministic, locale-independent.
pub fn render_text(lang: Lang, command: &str, data: &serde_json::Value) -> String {
    let mut lines = Vec::new();
    lines.push(format!("== {command} =="));
    // English output predates the catalog and is kept byte-for-byte; Arabic gets the
    // catalog's one-line outcome where one exists.
    if lang == Lang::Ar
        && let Some(outcome) = i18n::outcome_text(lang, command)
    {
        lines.push(outcome.to_string());
    }
    key_value_lines(lang, "", data, &mut lines);
    table_sections(lang, data, &mut lines);
    lines.join("\n") + "\n"
}

/// The text-mode failure line. English keeps its established wording; Arabic leads
/// with the catalog text and keeps the English reason as the technical detail.
pub fn error_line(lang: Lang, error: &CliError) -> String {
    match lang {
        Lang::En => format!("aetherctl: {} [{}]", error.text_reason(), error.kind()),
        Lang::Ar => format!(
            "aetherctl: {} ({}) [{}]",
            i18n::error_text(lang, error),
            error.text_reason(),
            error.kind()
        ),
    }
}

/// THE exit funnel: prints exactly one envelope (JSON) or one text report, returns the
/// process exit code. Refused operations always print WHY.
pub fn finish(config: &Config, command: &str, result: Result<serde_json::Value, CliError>) -> i32 {
    match result {
        Ok(data) => {
            match config.output {
                OutputMode::Json => match envelope::success(command, data) {
                    Ok(envelope) => {
                        println!("{}", serde_json::to_string(&envelope).unwrap_or_default());
                    }
                    Err(error) => {
                        eprintln!("aetherctl: envelope self-check failed: {error}");
                        return crate::exit::ExitCode::LocalIo.as_i32();
                    }
                },
                OutputMode::Text => {
                    print!("{}", render_text(config.lang, command, &data));
                }
            }
            let _ = std::io::stdout().flush();
            crate::exit::ExitCode::Ok.as_i32()
        }
        Err(error) => {
            if config.output == OutputMode::Json {
                let built = envelope::failure(
                    command,
                    error.kind(),
                    &error.message_key(),
                    Some(error.text_reason()),
                );
                match built {
                    Ok(envelope) => {
                        println!("{}", serde_json::to_string(&envelope).unwrap_or_default());
                    }
                    Err(build_error) => {
                        eprintln!("aetherctl: envelope build failed: {build_error}");
                    }
                }
            } else {
                eprintln!("{}", error_line(config.lang, &error));
            }
            let _ = std::io::stdout().flush();
            error.exit_code().as_i32()
        }
    }
}
