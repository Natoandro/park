use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success};

#[e2e(
    story = "PARK-LOG-009",
    scope = "follow-streaming",
    priority = "P0",
    description = "Follow matching output as an active process appends it",
    tags = ["logs", "follow", "streaming"]
)]
pub fn follow_active_process_output() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LOG-009")?;
    let launch = environment.run(&[
        "follow-stream",
        "--",
        "/bin/sh",
        "-c",
        "printf 'keep-initial\\nskip-initial\\n'; sleep .2; printf 'keep-later\\nskip-later\\n'",
    ])?;
    expect_success("launch", &launch)?;

    let followed = environment.run(&[
        "logs",
        "follow-stream",
        "--follow",
        "--stdout",
        "--grep",
        "keep",
    ])?;
    expect_success("follow logs", &followed)?;
    if followed.stdout != b"keep-initial\nkeep-later\n" {
        return Err(format!("follow output differs: {:?}", followed.stdout));
    }
    Ok(())
}
