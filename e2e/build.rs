use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest path"));
    let scenarios_dir = manifest_dir
        .parent()
        .expect("e2e package parent directory")
        .join("src/bin/park-e2e/scenarios");
    println!("cargo:rerun-if-changed={}", scenarios_dir.display());

    let mut modules = fs::read_dir(&scenarios_dir)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", scenarios_dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .filter(|path| path.file_stem().is_some_and(|stem| stem != "mod"))
        .collect::<Vec<_>>();
    modules.sort();

    let mut generated = String::from("use super::Scenario;\n\n");
    for path in &modules {
        let module = module_name(path);
        generated.push_str(&format!(
            "#[path = {:?}]\nmod {module};\n\n",
            path.to_string_lossy().to_string()
        ));
    }
    generated.push_str("static SCENARIOS: &[&Scenario] = &[\n");
    for path in &modules {
        generated.push_str(&format!("    &{}::SCENARIO,\n", module_name(path)));
    }
    generated.push_str("];\n\npub fn all() -> &'static [&'static Scenario] {\n    SCENARIOS\n}\n");

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("build output directory"))
        .join("park_e2e_scenarios.rs");
    fs::write(&output, generated)
        .unwrap_or_else(|error| panic!("could not write {}: {error}", output.display()));
}

fn module_name(path: &Path) -> String {
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_else(|| {
            panic!(
                "scenario file has an invalid module name: {}",
                path.display()
            )
        });
    if name.is_empty()
        || name
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        panic!("scenario file is not a valid Rust module name: {name}");
    }
    name.to_owned()
}
