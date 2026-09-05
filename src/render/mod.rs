use std::fmt::Write as _;

use serde_json::Value;

use park_cli::CommandResult;

mod daemon;

pub fn decode_json_result(result: &mut CommandResult<Value>) {
    if let Some(data) = &mut result.data {
        decode_value(data);
    }
}

pub fn human_result(result: &CommandResult<Value>) -> String {
    if !result.ok {
        return result.human_message().to_owned();
    }
    let Some(data) = &result.data else {
        return result.human_message().to_owned();
    };
    if let Some(content) = data.get("content").and_then(Value::as_str) {
        return content.to_owned();
    }
    if let Some(records) = data.as_array() {
        return human_process_list(records);
    }
    if is_process_record(data) {
        return human_process_record(data);
    }
    if data.get("variables").is_some() {
        return human_environment(data);
    }
    if data.get("reexec_state").is_some() {
        return daemon::human_status(data);
    }
    if data.get("config").is_some() && data.get("source").is_some() {
        return daemon::human_config(data);
    }
    if let Some(removed) = data.get("removed").and_then(Value::as_u64) {
        return format!("Removed {removed} process record(s).\n");
    }
    serde_json::to_string_pretty(data).expect("response data should serialize") + "\n"
}

fn decode_value(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                decode_value(value);
            }
        }
        Value::Object(object) => {
            if let Some(key) = object.get_mut("key")
                && let Some(key) = key.as_object_mut()
            {
                decode_field(key, "name");
            }
            decode_field(object, "executable");
            if let Some(arguments) = object.get_mut("arguments")
                && let Some(arguments) = arguments.as_array_mut()
            {
                for argument in arguments {
                    decode_hex_string(argument);
                }
            }
            for value in object.values_mut() {
                decode_value(value);
            }
        }
        _ => {}
    }
}

fn decode_field(object: &mut serde_json::Map<String, Value>, field: &str) {
    if let Some(value) = object.get_mut(field) {
        decode_hex_string(value);
    }
}

fn decode_hex_string(value: &mut Value) {
    let Some(encoded) = value.as_str() else {
        return;
    };
    let Some(decoded) = decode_hex(encoded) else {
        return;
    };
    *value = Value::String(String::from_utf8_lossy(&decoded).into_owned());
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if value.is_empty() || !value.len().is_multiple_of(2) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}

fn human_process_list(records: &[Value]) -> String {
    if records.is_empty() {
        return "No process records.\n".to_owned();
    }
    let rows = records.iter().map(process_row).collect::<Vec<_>>();
    let name_width = rows.iter().map(|row| row.0.len()).max().unwrap_or(0).max(4);
    let state_width = rows.iter().map(|row| row.1.len()).max().unwrap_or(0).max(5);
    let pid_width = rows.iter().map(|row| row.2.len()).max().unwrap_or(0).max(3);
    let mut output = format!(
        "{:<name_width$}  {:<state_width$}  {:>pid_width$}  COMMAND\n",
        "NAME", "STATE", "PID"
    );
    for (name, state, pid, command) in rows {
        let _ = writeln!(
            output,
            "{name:<name_width$}  {state:<state_width$}  {pid:>pid_width$}  {command}"
        );
    }
    output
}

fn process_row(record: &Value) -> (String, String, String, String) {
    let name = record
        .get("key")
        .and_then(|key| key.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_owned();
    let state = record
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_owned();
    let pid = record
        .get("pid")
        .and_then(Value::as_u64)
        .map(|pid| pid.to_string())
        .unwrap_or_else(|| "-".to_owned());
    let command = process_command(record);
    (name, state, pid, command)
}

fn human_process_record(record: &Value) -> String {
    let (name, state, pid, command) = process_row(record);
    let project = record
        .get("key")
        .and_then(|key| key.get("project_path"))
        .and_then(Value::as_str)
        .unwrap_or("?");
    let working_directory = record
        .get("working_directory")
        .and_then(Value::as_str)
        .unwrap_or("?");
    let mut output = String::new();
    let _ = writeln!(output, "Name: {name}");
    let _ = writeln!(output, "Project: {project}");
    let _ = writeln!(output, "Working directory: {working_directory}");
    let _ = writeln!(output, "State: {state}");
    let _ = writeln!(output, "PID: {pid}");
    let _ = writeln!(output, "Command: {command}");
    if let Some(exit_code) = record.get("exit_code").and_then(Value::as_i64) {
        let _ = writeln!(output, "Exit code: {exit_code}");
    }
    if let Some(signal) = record.get("termination_signal").and_then(Value::as_i64) {
        let _ = writeln!(output, "Termination signal: {signal}");
    }
    output
}

fn human_environment(data: &Value) -> String {
    let mut output = String::new();
    if let Some(variables) = data.get("variables").and_then(Value::as_array) {
        for variable in variables {
            let key = variable.get("key").and_then(Value::as_str).unwrap_or("?");
            let value = variable.get("value").and_then(Value::as_str).unwrap_or("");
            let _ = writeln!(output, "{key}={value}");
        }
    }
    output
}

fn process_command(record: &Value) -> String {
    let executable = record
        .get("executable")
        .and_then(Value::as_str)
        .unwrap_or("?");
    let arguments = record
        .get("arguments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(shell_quote)
        .collect::<Vec<_>>();
    if arguments.is_empty() {
        executable.to_owned()
    } else {
        format!("{} {}", shell_quote(executable), arguments.join(" "))
    }
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-./_:=,@%+".contains(&byte))
    {
        return value.to_owned();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn is_process_record(value: &Value) -> bool {
    value.get("key").is_some() && value.get("state").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use park_cli::ResultStatus;
    use serde_json::json;

    fn result(data: Value) -> CommandResult<Value> {
        CommandResult {
            status: ResultStatus::Success,
            ok: true,
            message: None,
            data: Some(data),
            error: None,
        }
    }

    #[test]
    fn decodes_process_strings_for_json() {
        let mut result = result(json!([{
            "key": {"name": "646576", "project_path": "/tmp/project"},
            "executable": "2f62696e2f7368",
            "arguments": ["2d63", "6563686f"]
        }]));
        decode_json_result(&mut result);
        assert_eq!(
            result.data,
            Some(json!([{
                "key": {"name": "dev", "project_path": "/tmp/project"},
                "executable": "/bin/sh",
                "arguments": ["-c", "echo"]
            }]))
        );
    }

    #[test]
    fn renders_process_list_for_humans() {
        let result = result(json!([{
            "key": {"name": "646576"},
            "executable": "2f62696e2f7368",
            "arguments": ["2d63", "6563686f"],
            "pid": 42,
            "state": "running"
        }]));
        let mut decoded = result.clone();
        decode_json_result(&mut decoded);
        let output = human_result(&decoded);
        assert!(output.starts_with("NAME"));
        assert!(output.contains("dev"));
        assert!(output.contains("/bin/sh -c echo"));
        assert!(!output.trim_start().starts_with('{'));
    }
}
