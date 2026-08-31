use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_contains, expect_success, hex, parse_json};

#[e2e(
    story = "PARK-CLI-002",
    scope = "arguments",
    priority = "P0",
    description = "Preserve the exact managed command across restart",
    tags = ["cli", "arguments", "lifecycle"]
)]
pub fn preserve_exact_managed_command() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-CLI-002")?;
    let script = "printf '%s\\n' \"$@\"";
    let arguments = [
        "-c",
        script,
        "sh",
        "two words",
        "quote'\"",
        "",
        "semi;colon",
        "$(printf not-interpreted)",
        "--child-flag",
    ];

    let mut launch_arguments = vec!["args", "--", "/bin/sh"];
    launch_arguments.extend(arguments);
    let launch = environment.run(&launch_arguments)?;
    expect_success("launch", &launch)?;

    let wait = environment.run(&["wait", "args", "--exit"])?;
    expect_success("first wait", &wait)?;

    let status = environment.run(&["status", "args", "--json"])?;
    expect_success("status", &status)?;
    let status_json = parse_json("status", &status)?;
    let record = status_json
        .get("data")
        .ok_or_else(|| "status response is missing data".to_owned())?;
    if record.get("executable").and_then(|value| value.as_str()) != Some("2f62696e2f7368") {
        return Err(format!("status recorded the wrong executable: {record}"));
    }
    let recorded_arguments = record
        .get("arguments")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "status response is missing arguments".to_owned())?;
    let expected_arguments = arguments
        .iter()
        .map(|argument| hex(argument.as_bytes()))
        .collect::<Vec<_>>();
    let actual_arguments = recorded_arguments
        .iter()
        .map(|argument| argument.as_str().unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    if actual_arguments != expected_arguments {
        return Err(format!(
            "recorded arguments differ: expected {expected_arguments:?}, got {actual_arguments:?}"
        ));
    }

    let expected_output = "two words\nquote'\"\n\nsemi;colon\n$(printf not-interpreted)\n--child-flag\n";
    let first_logs = environment.run(&["logs", "args", "--stdout"])?;
    expect_success("first logs", &first_logs)?;
    if first_logs.stdout != expected_output.as_bytes() {
        return Err(format!(
            "first invocation output differs: {:?}",
            String::from_utf8_lossy(&first_logs.stdout)
        ));
    }

    let restart = environment.run(&["restart", "args"])?;
    expect_success("restart", &restart)?;
    let wait = environment.run(&["wait", "args", "--exit"])?;
    expect_success("second wait", &wait)?;

    let logs = environment.run(&["logs", "args", "--stdout"])?;
    expect_success("logs", &logs)?;
    let expected = format!("{expected_output}{expected_output}");
    if logs.stdout != expected.as_bytes() {
        return Err(format!(
            "restarted output differs: {:?}",
            String::from_utf8_lossy(&logs.stdout)
        ));
    }
    expect_contains(&String::from_utf8_lossy(&logs.stdout), "$(printf not-interpreted)")?;
    Ok(())
}
