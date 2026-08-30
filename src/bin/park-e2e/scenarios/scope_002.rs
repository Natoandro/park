use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_stderr_nonempty, expect_success, parse_json};

#[e2e(
    story = "PARK-SCOPE-002",
    scope = "duplicate-identity",
    priority = "P0",
    description = "Reject a duplicate name without replacing its terminal record",
    tags = ["scope", "identity", "errors"]
)]
pub fn reject_duplicate_name_in_one_project() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-SCOPE-002")?;
    let launch = environment.run(&[
        "dev",
        "--",
        "/bin/sh",
        "-c",
        "printf original-record",
    ])?;
    expect_success("initial launch", &launch)?;
    expect_success(
        "wait for initial record",
        &environment.run(&["wait", "dev", "--exit"] )?,
    )?;

    let before = environment.run(&["status", "dev", "--json"])?;
    expect_success("status before duplicate", &before)?;
    let before_json = parse_json("status before duplicate", &before)?;
    let before_record = before_json
        .get("data")
        .cloned()
        .ok_or_else(|| "status before duplicate has no record".to_owned())?;
    let logs_before = environment.run(&["logs", "dev", "--stdout"])?;
    expect_success("logs before duplicate", &logs_before)?;

    let duplicate = environment.run(&["dev", "--", "/bin/sh", "-c", "printf replacement"])?;
    expect_exit("duplicate launch", &duplicate, 4)?;
    expect_stderr_nonempty("duplicate launch", &duplicate)?;

    let after = environment.run(&["status", "dev", "--json"])?;
    expect_success("status after duplicate", &after)?;
    let after_record = parse_json("status after duplicate", &after)?
        .get("data")
        .cloned()
        .ok_or_else(|| "status after duplicate has no record".to_owned())?;
    if after_record != before_record {
        return Err(format!("duplicate launch changed the record: {before_record} -> {after_record}"));
    }

    let logs_after = environment.run(&["logs", "dev", "--stdout"])?;
    expect_success("logs after duplicate", &logs_after)?;
    if logs_after.stdout != logs_before.stdout {
        return Err(format!(
            "duplicate launch changed logs: {:?} -> {:?}",
            logs_before.stdout, logs_after.stdout
        ));
    }
    Ok(())
}
