use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{
    TestEnvironment, expect_exit, expect_stderr_nonempty, expect_success, parse_json,
};

#[e2e(
    story = "PARK-CLI-007",
    scope = "usage-errors",
    priority = "P0",
    description = "Reject incomplete launch syntax without creating a record",
    tags = ["cli", "errors", "parsing"]
)]
pub fn reject_incomplete_launch_syntax() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-CLI-007")?;
    let invalid_invocations = [
        ("missing name", vec!["--", "/bin/true"]),
        ("missing separator", vec!["malformed", "/bin/true"]),
        ("missing command", vec!["malformed", "--"]),
        ("missing option value", vec!["logs", "malformed", "--tail"]),
    ];

    for (operation, arguments) in invalid_invocations {
        let output = environment.run(&arguments)?;
        expect_exit(operation, &output, 2)?;
        expect_stderr_nonempty(operation, &output)?;
        if !output.stdout.is_empty() {
            return Err(format!("{operation} wrote unexpected stdout"));
        }
    }

    let records = environment.run(&["ps", "--json"])?;
    expect_success("ps", &records)?;
    let json = parse_json("ps", &records)?;
    if !json
        .get("data")
        .and_then(|data| data.as_array())
        .is_some_and(Vec::is_empty)
    {
        return Err(format!("malformed launches created records: {json}"));
    }
    Ok(())
}
