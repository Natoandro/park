use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_contains, expect_success, parse_json};

#[e2e(
    story = "PARK-CLI-008",
    scope = "aliases",
    priority = "P1",
    description = "Use long operation aliases with canonical behavior",
    tags = ["cli", "aliases", "lifecycle"]
)]
pub fn use_operation_aliases() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-CLI-008")?;

    let launch = environment.run(&["alias", "--", "/bin/true"])?;
    expect_success("launch", &launch)?;
    let wait = environment.run(&["wait", "alias", "--exit"])?;
    expect_success("wait", &wait)?;

    let canonical_ps = environment.run(&["ps", "--json"])?;
    let alias_ps = environment.run(&["--ps", "--json"])?;
    expect_success("canonical ps", &canonical_ps)?;
    expect_success("ps alias", &alias_ps)?;
    if parse_json("canonical ps", &canonical_ps)? != parse_json("ps alias", &alias_ps)? {
        return Err("ps alias differs from canonical output".to_owned());
    }

    let canonical_status = environment.run(&["status", "alias", "--json"])?;
    let alias_status = environment.run(&["--status", "alias", "--json"])?;
    expect_success("canonical status", &canonical_status)?;
    expect_success("status alias", &alias_status)?;
    if parse_json("canonical status", &canonical_status)?
        != parse_json("status alias", &alias_status)?
    {
        return Err("status alias differs from canonical output".to_owned());
    }

    let canonical_logs = environment.run(&["logs", "alias", "--stdout", "--json"])?;
    let alias_logs = environment.run(&["--logs", "alias", "--stdout", "--json"])?;
    expect_success("canonical logs", &canonical_logs)?;
    expect_success("logs alias", &alias_logs)?;
    if parse_json("canonical logs", &canonical_logs)? != parse_json("logs alias", &alias_logs)? {
        return Err("logs alias differs from canonical output".to_owned());
    }

    let restart = environment.run(&["--restart", "alias"])?;
    expect_success("restart alias", &restart)?;
    let wait = environment.run(&["--wait", "alias", "--exit"])?;
    expect_success("wait alias", &wait)?;

    let start = environment.run(&["--start", "alias"])?;
    expect_success("start alias", &start)?;
    let wait = environment.run(&["wait", "alias", "--exit"])?;
    expect_success("second wait", &wait)?;

    let stop_launch = environment.run(&["stop-target", "--", "/bin/sleep", "30"])?;
    expect_success("stop target launch", &stop_launch)?;
    let stop = environment.run(&["--stop", "stop-target", "--force"])?;
    expect_success("stop alias", &stop)?;

    let signal_launch = environment.run(&["signal-target", "--", "/bin/sleep", "30"])?;
    expect_success("signal target launch", &signal_launch)?;
    let signal = environment.run(&["--signal", "signal-target", "TERM"])?;
    expect_success("signal alias", &signal)?;
    let wait = environment.run(&["wait", "signal-target", "--exit"])?;
    expect_success("signal wait", &wait)?;

    let remove_launch = environment.run(&["remove-target", "--", "/bin/true"])?;
    expect_success("remove target launch", &remove_launch)?;
    let wait = environment.run(&["wait", "remove-target", "--exit"])?;
    expect_success("remove target wait", &wait)?;
    let remove = environment.run(&["--rm", "remove-target"])?;
    expect_success("remove alias", &remove)?;

    let clean_launch = environment.run(&["clean-target", "--", "/bin/true"])?;
    expect_success("clean target launch", &clean_launch)?;
    let wait = environment.run(&["wait", "clean-target", "--exit"])?;
    expect_success("clean target wait", &wait)?;
    let clean = environment.run(&["--clean"])?;
    expect_success("clean alias", &clean)?;
    expect_contains(
        &String::from_utf8_lossy(&clean.stdout),
        "removed",
    )?;
    Ok(())
}
