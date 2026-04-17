//! Integration tests for the WASM backend.

use aether_behavior_dsl::{check, compile_module, parse};

fn compile(source: &str) -> Vec<u8> {
    let ast = parse(source).expect("parses");
    let checked = check(ast).expect("type-checks");
    compile_module(&checked).bytes
}

fn assert_valid_wasm(bytes: &[u8]) {
    wasmparser::validate(bytes).unwrap_or_else(|e| panic!("invalid wasm: {e}"));
}

#[test]
fn spawn_compiles_to_valid_wasm() {
    let wasm = compile("behavior X { version 1 spawn(\"goblin\", vec3(1.0, 0.0, 0.0)); }");
    assert_valid_wasm(&wasm);
}

#[test]
fn move_compiles_to_valid_wasm() {
    let wasm =
        compile("behavior X { @caps(Movement) version 1 move(self, vec3(0,0,0), 1.5); }");
    assert_valid_wasm(&wasm);
}

#[test]
fn damage_compiles_to_valid_wasm() {
    let wasm = compile("behavior X { @caps(Combat) version 1 damage(self, 5); }");
    assert_valid_wasm(&wasm);
}

#[test]
fn trigger_compiles_to_valid_wasm() {
    let wasm = compile(
        "behavior X { @caps(Network) version 1 trigger(\"evt\", {\"k\": 1}); }",
    );
    assert_valid_wasm(&wasm);
}

#[test]
fn dialogue_compiles_to_valid_wasm() {
    let wasm = compile(
        "behavior X { version 1 dialogue(self, \"hi\", [option(\"a\",\"a\"),option(\"b\",\"b\")]); }",
    );
    assert_valid_wasm(&wasm);
}

#[test]
fn combinators_compile_to_valid_wasm() {
    let wasm = compile(
        "behavior X { @caps(Combat, Movement) version 1 selector {
            retry(2) { damage(self, 1); }
            invert { damage(self, 1); }
            parallel { move(self, vec3(0,0,0), 1.0); damage(self, 2); }
            timeout(100) { move(self, vec3(0,0,0), 1.0); }
            sequence { move(self, vec3(1,0,0), 1.0); }
        } }",
    );
    assert_valid_wasm(&wasm);
}

#[test]
fn exported_tick_signature_is_i32_i32_i32() {
    use wasmparser::{FuncType, Parser, Payload, TypeRef, ValType};

    let wasm = compile(
        "behavior X { @caps(Movement) version 1 move(self, vec3(0,0,0), 1.0); }",
    );
    let mut types: Vec<FuncType> = Vec::new();
    let mut import_fn_types: Vec<u32> = Vec::new();
    let mut defined_fn_types: Vec<u32> = Vec::new();
    let mut tick_fn_idx: Option<u32> = None;
    let mut memory_exported = false;

    for payload in Parser::new(0).parse_all(&wasm) {
        match payload.unwrap() {
            Payload::TypeSection(reader) => {
                for rec_group in reader.into_iter_err_on_gc_types() {
                    let ty = rec_group.unwrap();
                    types.push(ty);
                }
            }
            Payload::ImportSection(reader) => {
                for imp in reader.into_imports() {
                    let imp = imp.unwrap();
                    if let TypeRef::Func(ti) | TypeRef::FuncExact(ti) = imp.ty {
                        import_fn_types.push(ti);
                    }
                }
            }
            Payload::FunctionSection(reader) => {
                for ti in reader {
                    defined_fn_types.push(ti.unwrap());
                }
            }
            Payload::ExportSection(reader) => {
                for exp in reader {
                    let exp = exp.unwrap();
                    if exp.name == "tick" {
                        tick_fn_idx = Some(exp.index);
                    }
                    if exp.name == "memory" {
                        memory_exported = true;
                    }
                }
            }
            _ => {}
        }
    }

    assert!(memory_exported, "memory must be exported");
    let tick_fn_idx = tick_fn_idx.expect("tick must be exported");
    let type_idx = {
        let n_imports = import_fn_types.len() as u32;
        if tick_fn_idx < n_imports {
            import_fn_types[tick_fn_idx as usize]
        } else {
            defined_fn_types[(tick_fn_idx - n_imports) as usize]
        }
    };
    let ft = &types[type_idx as usize];
    let params: Vec<_> = ft.params().iter().copied().collect();
    let results: Vec<_> = ft.results().iter().copied().collect();
    assert_eq!(params, vec![ValType::I32, ValType::I32]);
    assert_eq!(results, vec![ValType::I32]);
}

#[test]
fn compiled_module_imports_all_expected_verbs() {
    let wasm = compile("behavior X { @caps(Movement) version 1 move(self, vec3(0,0,0), 1.0); }");
    let summary =
        aether_behavior_dsl::WasmSummary::from_bytes(&wasm).expect("summary produced");
    let names: Vec<String> = summary
        .imports
        .iter()
        .filter(|(m, _)| m == "aether")
        .map(|(_, n)| n.clone())
        .collect();
    for expected in [
        "spawn",
        "move",
        "damage",
        "trigger",
        "dialogue",
        "timer_start",
        "timer_elapsed_ms",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "missing import `{}`",
            expected
        );
    }
}

#[test]
fn g01_patrol_smoke_wasm_validates_and_has_expected_surface() {
    let source = include_str!("golden/g01_patrol.beh");
    let ast = parse(source).expect("parses");
    let checked = check(ast).expect("type-checks");
    let compiled = compile_module(&checked);
    assert_valid_wasm(&compiled.bytes);
    let summary = &compiled.summary;
    assert!(summary
        .exports
        .iter()
        .any(|(n, k)| n == "tick" && k == "func"));
    assert!(summary
        .exports
        .iter()
        .any(|(n, k)| n == "memory" && k == "memory"));
    for name in ["spawn", "move", "damage", "trigger", "dialogue"] {
        assert!(summary
            .imports
            .iter()
            .any(|(m, n)| m == "aether" && n == name));
    }
}
