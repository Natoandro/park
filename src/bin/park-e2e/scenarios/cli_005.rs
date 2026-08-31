use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-CLI-005",
    scope = "names",
    priority = "P1",
    description = "Allow a dash-prefixed launch name",
    tags = ["cli", "names", "parsing"]
)]
pub fn accept_dash_prefixed_launch_names() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-CLI-005")?;

    let launch = environment.run(&["--status", "--", "/bin/true"])?;
    expect_success("launch", &launch)?;
    let wait = environment.run(&["wait", "--status", "--exit"])?;
    expect_success("wait", &wait)?;

    let status = environment.run(&["status", "--status", "--json"])?;
    expect_success("status", &status)?;
    let json = parse_json("status", &status)?;
    if json
        .get("data")
        .and_then(|data| data.get("key"))
        .and_then(|key| key.get("name"))
        .and_then(|name| name.as_str())
        != Some("--status")
    {
        return Err(format!("status did not address the `--status` record: {json}"));
    }

    let logs = environment.run(&["logs", "--status", "--stdout"])?;
    expect_success("logs", &logs)?;
    Ok(())
}
