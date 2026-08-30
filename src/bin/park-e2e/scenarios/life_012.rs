use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_success, parse_json};

#[e2e(
    story = "PARK-LIFE-012",
    scope = "remove-safety",
    priority = "P0",
    description = "Refuse removal while a managed process is active",
    tags = ["lifecycle", "remove", "safety"]
)]
pub fn refuse_remove_active_record() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LIFE-012")?;
    let launch = environment.run(&["active-remove", "--", "/bin/sleep", "30"])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait for running",
        &environment.run(&["wait", "active-remove", "--state", "running"])? ,
    )?;
    let before = environment.run(&["status", "active-remove", "--json"])?;
    expect_success("status before remove", &before)?;
    let before = parse_json("status before remove", &before)?;

    let remove = environment.run(&["rm", "active-remove"])?;
    expect_exit("remove active record", &remove, 5)?;
    let after = environment.run(&["status", "active-remove", "--json"])?;
    expect_success("status after refused remove", &after)?;
    let after = parse_json("status after refused remove", &after)?;
    if before != after
        || after
            .get("data")
            .and_then(|data| data.get("state"))
            .and_then(|state| state.as_str())
            != Some("running")
    {
        return Err(format!("active record changed after refused remove: {after}"));
    }
    let logs = environment.run(&["logs", "active-remove"])?;
    expect_success("logs after refused remove", &logs)?;
    expect_success(
        "cleanup",
        &environment.run(&["stop", "active-remove", "--force"])? ,
    )?;
    Ok(())
}
