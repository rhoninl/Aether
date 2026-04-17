//! Golden-test suite: all 10 common NPC behaviors compile and their AST +
//! WASM summary match the stored snapshots.

use std::path::PathBuf;

use aether_behavior_dsl::{check, compile_module, parse, WasmSummary};

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(golden_dir().join(name))
        .unwrap_or_else(|e| panic!("read {}: {}", name, e))
}

fn compare_or_refresh(name: &str, actual: &str) {
    let path = golden_dir().join(name);
    if std::env::var("BDSL_REFRESH_GOLDENS").is_ok() || !path.exists() {
        std::fs::write(&path, actual).expect("write golden");
        return;
    }
    let expected =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {}", name, e));
    assert_eq!(
        normalise(&expected),
        normalise(actual),
        "{} mismatch — set BDSL_REFRESH_GOLDENS=1 to refresh",
        name
    );
}

fn normalise(s: &str) -> String {
    s.replace("\r\n", "\n").trim_end_matches('\n').to_string()
}

fn run_golden(stem: &str) {
    let src = read(&format!("{}.beh", stem));
    let ast = parse(&src).unwrap_or_else(|e| panic!("{}: parse: {:?}", stem, e));
    let checked = check(ast.clone())
        .unwrap_or_else(|e| panic!("{}: typecheck: {:?}", stem, e));

    // AST snapshot.
    let ast_json = serde_json::to_string_pretty(&ast).expect("json");
    compare_or_refresh(&format!("{}.ast.json", stem), &ast_json);

    // Compile + WASM validation + summary snapshot.
    let compiled = compile_module(&checked);
    wasmparser::validate(&compiled.bytes)
        .unwrap_or_else(|e| panic!("{}: wasm validate: {}", stem, e));
    // Re-parse from bytes so the summary matches the *binary* output.
    let summary = WasmSummary::from_bytes(&compiled.bytes).expect("summary");
    compare_or_refresh(&format!("{}.wasm-summary.txt", stem), &summary.to_text());
}

#[test]
fn g01_patrol() {
    run_golden("g01_patrol");
}
#[test]
fn g02_guard() {
    run_golden("g02_guard");
}
#[test]
fn g03_merchant() {
    run_golden("g03_merchant");
}
#[test]
fn g04_enemy_melee() {
    run_golden("g04_enemy_melee");
}
#[test]
fn g05_enemy_ranged() {
    run_golden("g05_enemy_ranged");
}
#[test]
fn g06_quest_giver() {
    run_golden("g06_quest_giver");
}
#[test]
fn g07_door_opener() {
    run_golden("g07_door_opener");
}
#[test]
fn g08_chest_loot() {
    run_golden("g08_chest_loot");
}
#[test]
fn g09_companion() {
    run_golden("g09_companion");
}
#[test]
fn g10_boss_phase() {
    run_golden("g10_boss_phase");
}
