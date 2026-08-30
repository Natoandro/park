use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-LOG-012",
    scope = "log-json",
    priority = "P0",
    description = "Return selected, combined, and filtered logs as structured JSON",
    tags = ["logs", "json", "automation"]
)]
pub fn return_structured_log_json() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LOG-012")?;
    let launch = environment.run(&[
        "json-logs",
        "--",
        "/bin/sh",
        "-c",
        "printf 'alpha\\nbeta\\n'; printf 'diagnostic\\n' >&2",
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait",
        &environment.run(&["wait", "json-logs", "--exit"] )?,
    )?;

    let stdout = environment.run(&["logs", "json-logs", "--stdout"])?;
    expect_success("plain stdout logs", &stdout)?;
    let stdout_json = environment.run(&["logs", "json-logs", "--stdout", "--json"])?;
    expect_success("stdout JSON logs", &stdout_json)?;
    let selected = parse_json("stdout JSON logs", &stdout_json)?;
    assert_log_json(&selected, "stdout", &String::from_utf8_lossy(&stdout.stdout), "exited")?;
    if !stdout_json.stderr.is_empty() {
        return Err("stdout JSON logs wrote diagnostics to stderr".to_owned());
    }

    let combined = environment.run(&["logs", "json-logs", "--json"])?;
    expect_success("combined JSON logs", &combined)?;
    let combined_value = parse_json("combined JSON logs", &combined)?;
    assert_log_json(
        &combined_value,
        "combined",
        "alpha\nbeta\ndiagnostic\n",
        "exited",
    )?;

    let filtered_plain = environment.run(&["logs", "json-logs", "--stdout", "--grep", "beta"])?;
    expect_success("plain filtered logs", &filtered_plain)?;
    let filtered_json = environment.run(&[
        "logs",
        "json-logs",
        "--stdout",
        "--grep",
        "beta",
        "--json",
    ])?;
    expect_success("filtered JSON logs", &filtered_json)?;
    let filtered = parse_json("filtered JSON logs", &filtered_json)?;
    assert_log_json(
        &filtered,
        "stdout",
        &String::from_utf8_lossy(&filtered_plain.stdout),
        "exited",
    )?;
    Ok(())
}

fn assert_log_json(
    value: &serde_json::Value,
    stream: &str,
    content: &str,
    state: &str,
) -> Result<(), String> {
    if value["status"] != "success"
        || value["ok"] != true
        || value["data"]["stream"] != stream
        || value["data"]["content"] != content
        || value["data"]["state"] != state
    {
        return Err(format!("log JSON has unexpected shape: {value}"));
    }
    Ok(())
}
