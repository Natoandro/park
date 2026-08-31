use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_contains, expect_success};

#[e2e(
    story = "PARK-INSPECT-003",
    scope = "process-status",
    priority = "P0",
    description = "Inspect active and terminal records with human-readable status",
    tags = ["inspection", "status", "human-output"]
)]
pub fn inspect_record_with_human_status() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-INSPECT-003")?;
    let launch = environment.run(&["human-status", "--", "/bin/sleep", "30"])?;
    expect_success("launch", &launch)?;

    let active = environment.run(&["status", "human-status"])?;
    expect_human_status("active status", &active)?;
    let active_text = String::from_utf8_lossy(&active.stdout);
    let project = environment.project_path().to_string_lossy();
    expect_contains(&active_text, &format!("Project: {project}"))?;
    expect_contains(&active_text, &format!("Working directory: {project}"))?;
    expect_contains(&active_text, "Name: human-status")?;
    expect_contains(&active_text, "State: running")?;
    expect_contains(&active_text, "Command: /bin/sleep 30")?;

    let stop = environment.run(&["stop", "human-status", "--force"])?;
    expect_success("force stop", &stop)?;
    let terminal = environment.run(&["status", "human-status"])?;
    expect_human_status("terminal status", &terminal)?;
    let terminal_text = String::from_utf8_lossy(&terminal.stdout);
    expect_contains(&terminal_text, "State: killed")?;
    expect_contains(&terminal_text, "Termination signal: 9")?;
    Ok(())
}

fn expect_human_status(operation: &str, output: &std::process::Output) -> Result<(), String> {
    expect_success(operation, output)?;
    if output.stdout.is_empty() {
        return Err(format!("{operation} returned no human output"));
    }
    if !output.stderr.is_empty() {
        return Err(format!(
            "{operation} wrote unexpected stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}
