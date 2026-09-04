use std::env;

#[path = "../../src/bin/park-e2e/scenarios/mod.rs"]
mod scenarios;
#[path = "../../src/bin/park-e2e/support.rs"]
mod support;

pub struct Scenario {
    pub story: &'static str,
    pub name: &'static str,
    pub scope: &'static str,
    pub priority: &'static str,
    pub description: &'static str,
    pub tags: &'static [&'static str],
    pub run: fn() -> Result<(), String>,
}

struct Options {
    list: bool,
    filter: Option<String>,
    tag: Option<String>,
}

fn main() {
    let options = match parse_options() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("park-e2e: {error}");
            std::process::exit(2);
        }
    };
    let selected = scenarios::all()
        .iter()
        .copied()
        .filter(|scenario| matches_options(scenario, &options))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        eprintln!("park-e2e: no scenarios matched the requested selection");
        std::process::exit(1);
    }
    if options.list {
        for scenario in selected {
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}",
                scenario.story,
                scenario.name,
                scenario.scope,
                scenario.priority,
                scenario.tags.join(","),
                scenario.description
            );
        }
        return;
    }

    let mut failed = false;
    for scenario in selected {
        println!("{}: {}", scenario.story, scenario.description);
        match (scenario.run)() {
            Ok(()) => println!("{} passed", scenario.story),
            Err(error) => {
                eprintln!("{} failed: {error}", scenario.story);
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}

fn parse_options() -> Result<Options, String> {
    let mut list = false;
    let mut filter = None;
    let mut tag = None;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--list" => list = true,
            "--filter" => filter = Some(next_option_value(&mut arguments, "--filter")?),
            "--tag" => tag = Some(next_option_value(&mut arguments, "--tag")?),
            "--help" | "-h" => {
                println!("usage: park-e2e [--list] [--filter TEXT] [--tag TAG]");
                std::process::exit(0);
            }
            _ => return Err(format!("unknown option `{argument}`")),
        }
    }
    Ok(Options { list, filter, tag })
}

fn next_option_value<I>(arguments: &mut I, option: &str) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    arguments
        .next()
        .filter(|value| !value.starts_with('-'))
        .ok_or_else(|| format!("{option} requires a value"))
}

fn matches_options(scenario: &Scenario, options: &Options) -> bool {
    let filter_matches = options.filter.as_ref().is_none_or(|filter| {
        scenario.story.contains(filter)
            || scenario.name.contains(filter)
            || scenario.description.contains(filter)
    });
    let tag_matches = options
        .tag
        .as_ref()
        .is_none_or(|tag| scenario.tags.iter().any(|candidate| candidate == tag));
    filter_matches && tag_matches
}
