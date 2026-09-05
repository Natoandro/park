use serde_json::{Value, json};

use crate::result::CommandResult;

const SOURCE: &str = "Natoandro/park";
const PROJECT_INSTALL: &str = "npx skills add Natoandro/park --skill park";
const GLOBAL_INSTALL: &str = "npx skills add Natoandro/park --skill park -g";
const ONE_OFF_USE: &str = "npx skills use Natoandro/park --skill park";

pub fn skills_help_result(json_output: bool) -> CommandResult<Value> {
    if json_output {
        CommandResult::success(Some(skills_data()), None)
    } else {
        CommandResult::success(Some(json!({"content": skills_guide()})), None)
    }
}

pub fn command_help_result() -> CommandResult<Value> {
    CommandResult::success(Some(json!({"content": crate::cli::command_help()})), None)
}

fn skills_guide() -> &'static str {
    "Park AI agent integration\n\nInstall the canonical skill:\n  Project: npx skills add Natoandro/park --skill park\n  Global:  npx skills add Natoandro/park --skill park -g\n\nThe default install commands detect available agents and let you choose when\nneeded. To target one agent explicitly, add -a <agent>, for example:\n  npx skills add Natoandro/park --skill park -a opencode\n\nUse it once without installing (prints a prompt):\n  npx skills use Natoandro/park --skill park\n\nTo start a specific supported agent, add --agent <agent>.\n\nRecommended workflow:\n  1. Run from the project directory associated with the process.\n  2. Inspect records: park ps --json\n  3. Launch: park <name> [--env-file <path>]... -- <command> [arguments...]\n  4. Wait for running or readiness output with park wait.\n  5. Diagnose with park status <name> --json and park logs <name>.\n  6. Inspect or update future environment values with park env <name> --json.\n  7. Stop or remove only records belonging to the task.\n\nSkill maintenance:\n  npx skills update park\n  npx skills remove park\n"
}

fn skills_data() -> Value {
    json!({
        "name": "park",
        "source": SOURCE,
        "install": {
            "project": PROJECT_INSTALL,
            "global": GLOBAL_INSTALL,
            "one_off": ONE_OFF_USE,
        },
        "workflow": [
            "Run from the project directory associated with the process.",
            "Inspect records with park ps --json before choosing a name.",
            "Launch with park <name> [--env-file <path>]... -- <command> [arguments...].",
            "Wait for running or a literal readiness message with park wait.",
            "Use park status <name> --json and park logs <name> to diagnose failures.",
            "Use park env <name> --json to inspect or update future environment values.",
            "Stop or remove only records belonging to the task or explicitly requested by the user.",
        ],
        "maintenance": {
            "update": "npx skills update park",
            "remove": "npx skills remove park",
        },
        "exit_codes": {
            "success": 0,
            "failure": 1,
            "usage_error": 2,
            "missing_record": 3,
            "duplicate_record": 4,
            "invalid_lifecycle_state": 5,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_human_skills_guide() {
        let result = skills_help_result(false);
        assert!(result.ok);
        assert_eq!(result.data.expect("guide data")["content"], skills_guide());
    }

    #[test]
    fn renders_machine_readable_skills_guide() {
        let result = skills_help_result(true);
        assert!(result.ok);
        let data = result.data.expect("guide data");
        assert_eq!(data["name"], "park");
        assert_eq!(data["install"]["project"], PROJECT_INSTALL);
    }

    #[test]
    fn renders_command_help() {
        let result = command_help_result();
        assert!(result.ok);
        let data = result.data.expect("help data");
        let content = data["content"].as_str().expect("help content");
        assert!(content.contains("logs     Read retained process output"));
        assert!(content.contains("The launch form is"));
    }
}
