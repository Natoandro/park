use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-LIFE-005",
    scope = "signal-control",
    priority = "P1",
    description = "Accept supported named signals with and without SIG prefixes",
    tags = ["lifecycle", "signals", "linux"]
)]
pub fn send_supported_named_signals() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LIFE-005")?;
    let signals = [
        ("HUP", "hup"),
        ("SIGINT", "int"),
        ("QUIT", "quit"),
        ("SIGTERM", "term"),
        ("USR1", "usr1"),
        ("SIGUSR2", "usr2"),
        ("STOP", "stop"),
        ("SIGCONT", "cont"),
        ("KILL", "kill"),
    ];
    let script = "trap '' HUP INT QUIT TERM USR1 USR2; while :; do sleep 30; done";

    for (signal, suffix) in signals {
        let name = format!("signal-{suffix}");
        let launch = environment.run(&[&name, "--", "/bin/sh", "-c", script])?;
        expect_success("signal target launch", &launch)?;
        expect_success(
            "wait for signal target",
            &environment.run(&["wait", &name, "--state", "running"])? ,
        )?;

        let sent = environment.run(&["signal", &name, signal])?;
        expect_success(signal, &sent)?;
        if signal == "STOP" {
            let continued = environment.run(&["signal", &name, "SIGCONT"])?;
            expect_success("SIGCONT", &continued)?;
        }

        if signal == "KILL" {
            expect_success("wait after KILL", &environment.run(&["wait", &name, "--exit"])? )?;
            let status = environment.run(&["status", &name, "--json"])?;
            expect_success("KILL status", &status)?;
            let value = parse_json("KILL status", &status)?;
            if value
                .get("data")
                .and_then(|data| data.get("state"))
                .and_then(|state| state.as_str())
                != Some("killed")
            {
                return Err(format!("KILL did not produce a killed record: {value}"));
            }
        } else {
            let status = environment.run(&["status", &name, "--json"])?;
            expect_success("active signal status", &status)?;
            let value = parse_json("active signal status", &status)?;
            if value
                .get("data")
                .and_then(|data| data.get("state"))
                .and_then(|state| state.as_str())
                != Some("running")
            {
                return Err(format!("{signal} unexpectedly changed lifecycle state: {value}"));
            }
            expect_success(
                "clean up signal target",
                &environment.run(&["stop", &name, "--force"])? ,
            )?;
        }
    }
    Ok(())
}
