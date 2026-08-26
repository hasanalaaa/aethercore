//! Text rendering + the single exit funnel. JSON and text views derive from the SAME
//! serde_json projection, so the two modes can never disagree. All output is ASCII,
//! locale-independent (no thousands separators, no platform number formatting).

use crate::cli::{Config, OutputMode};
use crate::envelope;
use crate::error::CliError;
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

fn scalar_to_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "-".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => {
            if items.is_empty() {
                "[]".to_string()
            } else {
                items
                    .iter()
                    .map(scalar_to_text)
                    .collect::<Vec<_>>()
                    .join(",")
            }
        }
        serde_json::Value::Object(_) => "<object>".to_string(),
    }
}

fn key_value_lines(prefix: &str, value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, inner) in map {
                let label = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                match inner {
                    serde_json::Value::Object(_) => key_value_lines(&label, inner, out),
                    // Arrays of objects render as tables below; scalar arrays inline here.
                    serde_json::Value::Array(items) if is_object_array(items) => {}
                    other => out.push(format!("{:<34} {}", label, scalar_to_text(other))),
                }
            }
        }
        other => out.push(format!("{:<34} {}", prefix, scalar_to_text(other))),
    }
}

fn is_object_array(items: &[serde_json::Value]) -> bool {
    !items.is_empty() && items.iter().all(|item| item.is_object())
}

/// Emits one aligned table per array-of-objects section (deterministic column order =
/// first row's insertion order).
fn table_sections(value: &serde_json::Value, out: &mut Vec<String>) {
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
                            .map(scalar_to_text)
                            .unwrap_or_else(|| "-".to_string())
                    })
                    .collect(),
                _ => Vec::new(),
            })
            .collect();
        out.push(String::new());
        out.push(format!("[{key}]"));
        out.push(table(&headers, &rows));
    }
}

/// Human view of a command's data projection: aligned key/value block plus a real
/// ASCII table for every array section. Deterministic, locale-independent.
pub fn render_text(command: &str, data: &serde_json::Value) -> String {
    let mut lines = Vec::new();
    lines.push(format!("== {command} =="));
    key_value_lines("", data, &mut lines);
    table_sections(data, &mut lines);
    lines.join("\n") + "\n"
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
                    print!("{}", render_text(command, &data));
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
                eprintln!("aetherctl: {} [{}]", error.text_reason(), error.kind());
            }
            let _ = std::io::stdout().flush();
            error.exit_code().as_i32()
        }
    }
}
