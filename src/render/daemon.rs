use std::fmt::Write as _;

use serde_json::Value;

pub(super) fn human_status(data: &Value) -> String {
    let mut output = String::new();
    let fields = [
        ("PID", "pid"),
        ("Binary version", "binary_version"),
        ("Protocol version", "protocol_version"),
        ("Handoff version", "handoff_version"),
        ("Generation", "generation"),
        ("Re-exec state", "reexec_state"),
        ("Active records", "active_record_count"),
    ];
    for (label, field) in fields {
        if let Some(value) = data.get(field) {
            let _ = writeln!(output, "{label}: {}", display_value(value));
        }
    }
    output
}

pub(super) fn human_config(data: &Value) -> String {
    let source = data
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let source = if source == "defaults" {
        "built-in defaults"
    } else {
        source
    };
    let path = data.get("path").and_then(Value::as_str).unwrap_or("-");
    let config = data.get("config").and_then(Value::as_object);
    let active_processes = config
        .and_then(|config| config.get("daemon"))
        .and_then(|daemon| daemon.get("reexec"))
        .and_then(|reexec| reexec.get("active_processes"))
        .and_then(Value::as_str)
        .unwrap_or("?");
    let restart = config
        .and_then(|config| config.get("managed_processes"))
        .and_then(|managed| managed.get("restart"));
    let mut output = format!("Source: {source}\nPath: {path}\n");
    let _ = writeln!(output, "Re-exec active processes: {active_processes}");
    if let Some(restart) = restart {
        let _ = writeln!(
            output,
            "Restart policy: {}",
            restart.get("policy").and_then(Value::as_str).unwrap_or("?")
        );
        let _ = writeln!(
            output,
            "Restart max attempts: {}",
            display_value(restart.get("max_attempts").unwrap_or(&Value::Null))
        );
        let _ = writeln!(
            output,
            "Restart initial delay: {}",
            display_value(restart.get("initial_delay").unwrap_or(&Value::Null))
        );
        let _ = writeln!(
            output,
            "Restart max delay: {}",
            display_value(restart.get("max_delay").unwrap_or(&Value::Null))
        );
        let _ = writeln!(
            output,
            "Restart multiplier: {}",
            display_value(restart.get("multiplier").unwrap_or(&Value::Null))
        );
    }
    output
}

fn display_value(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_daemon_status_for_humans() {
        let output = human_status(&serde_json::json!({
            "pid": 42,
            "binary_version": "0.2.1",
            "protocol_version": 1,
            "handoff_version": 0,
            "generation": 1,
            "reexec_state": "serving",
            "active_record_count": 2
        }));
        assert!(output.contains("PID: 42"));
        assert!(output.contains("Active records: 2"));
    }

    #[test]
    fn renders_daemon_config_for_humans() {
        let output = human_config(&serde_json::json!({
            "source": "defaults",
            "path": null,
            "config": {
                "daemon": {"reexec": {"active_processes": "defer"}},
                "managed_processes": {"restart": {
                    "policy": "never",
                    "max_attempts": 3,
                    "initial_delay": "250ms",
                    "max_delay": "30s",
                    "multiplier": 2.0
                }}
            }
        }));
        assert!(output.contains("Source: built-in defaults"));
        assert!(output.contains("Re-exec active processes: defer"));
        assert!(output.contains("Restart policy: never"));
    }
}
