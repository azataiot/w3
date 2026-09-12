use std::ffi::OsString;
use std::io::{self, Write};

use serde::Serialize;
use serde_json::{Value, json};

pub struct Output {
    pub json: bool,
    pub command: Option<&'static str>,
    pub data: Value,
}

#[derive(Debug)]
pub struct Failure {
    code: &'static str,
    message: String,
    details: Value,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Failure {}

pub fn failure(code: &'static str, message: impl Into<String>, details: Value) -> anyhow::Error {
    Failure {
        code,
        message: message.into(),
        details,
    }
    .into()
}

#[derive(Serialize)]
struct Envelope<'a> {
    schema_version: u32,
    command: Option<&'a str>,
    ok: bool,
    data: &'a Value,
    error: Option<Value>,
}

impl Output {
    pub fn new(args: &[OsString]) -> Self {
        let mut format = std::env::var("W3_FORMAT").ok();
        let mut command = None;
        let mut args = args.iter().skip(1);
        while let Some(arg) = args.next() {
            let Some(arg) = arg.to_str() else { continue };
            if arg == "--" {
                break;
            }
            if arg == "--format" {
                format = args.next().and_then(|arg| arg.to_str()).map(str::to_owned);
            } else if let Some(value) = arg.strip_prefix("--format=") {
                format = Some(value.to_owned());
            } else if command.is_none() {
                command = command_name(arg);
            }
        }
        Self {
            json: format.as_deref() == Some("json"),
            command,
            data: json!({}),
        }
    }

    pub fn finish(&self, error: Option<&anyhow::Error>) -> io::Result<()> {
        if self.json {
            let envelope = Envelope {
                schema_version: 1,
                command: self.command,
                ok: error.is_none(),
                data: &self.data,
                error: error.map(error_value),
            };
            let mut stdout = io::stdout().lock();
            serde_json::to_writer(&mut stdout, &envelope)?;
            writeln!(stdout)
        } else if let Some(error) = error {
            writeln!(io::stderr().lock(), "Error: {error:#}")
        } else {
            Ok(())
        }
    }
}

fn command_name(value: &str) -> Option<&'static str> {
    match value {
        "list" => Some("list"),
        "add" => Some("add"),
        "cp" => Some("cp"),
        "cd" => Some("cd"),
        "init" => Some("init"),
        "remove" => Some("remove"),
        _ => None,
    }
}

fn error_value(error: &anyhow::Error) -> Value {
    if let Some(failure) = error.downcast_ref::<Failure>() {
        return json!({
            "code": failure.code,
            "message": format!("{error:#}"),
            "details": failure.details,
        });
    }
    let code = match error.downcast_ref::<w3::Error>() {
        Some(w3::Error::UnsafeRemoval { reason, path }) => {
            return json!({
                "code": "unsafe_removal", "message": format!("{error:#}"),
                "details": {"reason": reason, "path": path.to_string_lossy()}
            });
        }
        Some(w3::Error::InvalidName(_)) => "invalid_name",
        Some(w3::Error::Git(_)) => "git_error",
        Some(w3::Error::Spawn(_)) => "io_error",
        Some(w3::Error::Parse(_)) => "git_output_invalid",
        None if error.downcast_ref::<io::Error>().is_some() => "io_error",
        None => "operation_failed",
    };
    json!({"code": code, "message": format!("{error:#}"), "details": {}})
}
