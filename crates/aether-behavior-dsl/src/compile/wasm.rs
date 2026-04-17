//! WASM backend.
//!
//! Emits a validated WebAssembly module that:
//! * Exports `memory` (for host-read string constants) and `tick`.
//! * Imports one host function per DSL verb (`aether.spawn`, `aether.move`,
//!   `aether.damage`, `aether.trigger`, `aether.dialogue`) plus helpers
//!   (`aether.timer_start`, `aether.timer_elapsed_ms`, `aether.log_int`).
//! * Lays out every string literal as `(ptr, len)` pairs in a data segment.
//!
//! The WASM validator in `wasmparser` accepts the output.
//!
//! `BehaviorStatus` encoding in the returned i32: 0=Success, 1=Failure,
//! 2=Running. These are also declared in [`crate::types::BehaviorStatus`].

use std::collections::BTreeSet;

use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection,
    Function, FunctionSection, Ieee32, ImportSection, MemorySection, MemoryType,
    Module as WasmModule, TypeSection, ValType,
};

use crate::ast::{Combinator, Expr, ExprKind, Literal, Node, NodeKind, Verb};
use crate::typeck::CheckedModule;

/// Result of compiling a behavior to WASM.
#[derive(Debug, Clone)]
pub struct CompiledModule {
    pub bytes: Vec<u8>,
    pub summary: WasmSummary,
}

/// A structural summary of a compiled WASM module, used for golden tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmSummary {
    /// Imports as ("module", "name") pairs, sorted for stability.
    pub imports: Vec<(String, String)>,
    /// Exports as (name, kind) pairs, sorted.
    pub exports: Vec<(String, String)>,
    /// Total number of functions in the module (imports + definitions).
    pub function_count: u32,
    /// Number of defined (non-imported) functions.
    pub defined_function_count: u32,
}

impl WasmSummary {
    /// Render the summary in the snapshot format stored as `.wasm-summary.txt`.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str("imports:\n");
        for (m, n) in &self.imports {
            out.push_str(&format!("  {}.{}\n", m, n));
        }
        out.push_str("exports:\n");
        for (n, k) in &self.exports {
            out.push_str(&format!("  {} ({})\n", n, k));
        }
        out.push_str(&format!("function_count: {}\n", self.function_count));
        out.push_str(&format!(
            "defined_function_count: {}\n",
            self.defined_function_count
        ));
        out
    }

    /// Inspect a compiled WASM module and produce its summary.
    pub fn from_bytes(bytes: &[u8]) -> Result<WasmSummary, String> {
        use wasmparser::{BinaryReaderError, Parser, Payload};
        let mut imports = Vec::new();
        let mut exports = Vec::new();
        let mut imported_fn_count = 0u32;
        let mut defined_fn_count = 0u32;
        for payload in Parser::new(0).parse_all(bytes) {
            let payload: Payload = payload.map_err(|e: BinaryReaderError| e.to_string())?;
            match payload {
                Payload::ImportSection(reader) => {
                    for imp in reader.into_imports() {
                        let imp = imp.map_err(|e: BinaryReaderError| e.to_string())?;
                        imports.push((imp.module.to_string(), imp.name.to_string()));
                        if matches!(
                            imp.ty,
                            wasmparser::TypeRef::Func(_) | wasmparser::TypeRef::FuncExact(_)
                        ) {
                            imported_fn_count += 1;
                        }
                    }
                }
                Payload::FunctionSection(reader) => {
                    defined_fn_count = reader.count();
                }
                Payload::ExportSection(reader) => {
                    for exp in reader {
                        let exp = exp.map_err(|e: BinaryReaderError| e.to_string())?;
                        let kind = match exp.kind {
                            wasmparser::ExternalKind::Func
                            | wasmparser::ExternalKind::FuncExact => "func",
                            wasmparser::ExternalKind::Memory => "memory",
                            wasmparser::ExternalKind::Global => "global",
                            wasmparser::ExternalKind::Table => "table",
                            wasmparser::ExternalKind::Tag => "tag",
                        };
                        exports.push((exp.name.to_string(), kind.to_string()));
                    }
                }
                _ => {}
            }
        }
        imports.sort();
        exports.sort();
        Ok(WasmSummary {
            imports,
            exports,
            function_count: imported_fn_count + defined_fn_count,
            defined_function_count: defined_fn_count,
        })
    }
}

/// BehaviorStatus encoding.
const STATUS_SUCCESS: i32 = 0;
const STATUS_FAILURE: i32 = 1;
const STATUS_RUNNING: i32 = 2;

/// Imported host-function indices. Assigned in the order imports are declared.
const IMPORT_SPAWN: u32 = 0;
const IMPORT_MOVE: u32 = 1;
const IMPORT_DAMAGE: u32 = 2;
const IMPORT_TRIGGER: u32 = 3;
const IMPORT_DIALOGUE: u32 = 4;
const IMPORT_TIMER_START: u32 = 5;
const IMPORT_TIMER_ELAPSED_MS: u32 = 6;
const IMPORTS_COUNT: u32 = 7;

/// Base address of interned string constants in linear memory.
const STRING_POOL_BASE: u32 = 1024;

/// Helper locals inside a behavior-node helper function.
///
/// Layout: locals 0 and 1 are the two function parameters (`world`, `entity`).
/// Locals 2..=4 are `i32` scratch slots used by combinator codegen.
const LOCAL_SCRATCH_A: u32 = 2;
const LOCAL_SCRATCH_B: u32 = 3;
const LOCAL_SCRATCH_C: u32 = 4;
/// Number of additional i32 locals declared on each helper function.
const HELPER_LOCAL_COUNT: u32 = 3;

/// Compile a type-checked module into a WASM binary.
pub fn compile_module(checked: &CheckedModule) -> CompiledModule {
    let mut ctx = CompileCtx::default();

    // Flatten the tree — each node becomes one helper function. Helper #0 is
    // the root, so `tick` can call it unconditionally.
    let flat = flatten_tree(&checked.module.root);

    // Intern every string literal that will appear in codegen.
    for node in &flat {
        if let Some(args) = node_verb_args(node) {
            for arg in args {
                intern_strings_in_expr(arg, &mut ctx);
            }
        }
    }

    let mut module = WasmModule::new();

    // ---- Type section ----
    // Indices:
    // 0: (i32, f32, f32, f32) -> i32                  spawn
    // 1: (i32, f32, f32, f32, f32) -> i32             move
    // 2: (i32, i32) -> i32                            damage
    // 3: (i32, i32, i32, i32) -> i32                  trigger(name_ptr, name_len, data_ptr, data_len)
    // 4: (i32, i32, i32, i32, i32) -> i32             dialogue
    // 5: () -> i32                                    timer_start
    // 6: (i32) -> i32                                 timer_elapsed_ms
    // 7: (i32, i32) -> i32                            tick + node helpers
    let mut types = TypeSection::new();
    types.ty().function(
        [ValType::I32, ValType::F32, ValType::F32, ValType::F32],
        [ValType::I32],
    );
    types.ty().function(
        [
            ValType::I32,
            ValType::F32,
            ValType::F32,
            ValType::F32,
            ValType::F32,
        ],
        [ValType::I32],
    );
    types
        .ty()
        .function([ValType::I32, ValType::I32], [ValType::I32]);
    types.ty().function(
        [ValType::I32, ValType::I32, ValType::I32, ValType::I32],
        [ValType::I32],
    );
    types.ty().function(
        [
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
        ],
        [ValType::I32],
    );
    types.ty().function([], [ValType::I32]);
    types.ty().function([ValType::I32], [ValType::I32]);
    types
        .ty()
        .function([ValType::I32, ValType::I32], [ValType::I32]);
    module.section(&types);
    const HELPER_TYPE_IDX: u32 = 7;

    // ---- Import section ----
    let mut imports = ImportSection::new();
    imports.import("aether", "spawn", EntityType::Function(0));
    imports.import("aether", "move", EntityType::Function(1));
    imports.import("aether", "damage", EntityType::Function(2));
    imports.import("aether", "trigger", EntityType::Function(3));
    imports.import("aether", "dialogue", EntityType::Function(4));
    imports.import("aether", "timer_start", EntityType::Function(5));
    imports.import("aether", "timer_elapsed_ms", EntityType::Function(6));
    module.section(&imports);

    // ---- Function section ----
    let mut functions = FunctionSection::new();
    for _ in &flat {
        functions.function(HELPER_TYPE_IDX);
    }
    // `tick` at the end.
    functions.function(HELPER_TYPE_IDX);
    module.section(&functions);

    // ---- Memory section (1 page = 64 KiB). ----
    let mut memory = MemorySection::new();
    memory.memory(MemoryType {
        minimum: 1,
        maximum: Some(1),
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    module.section(&memory);

    // ---- Export section ----
    let tick_fn_idx = IMPORTS_COUNT + flat.len() as u32;
    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    exports.export("tick", ExportKind::Func, tick_fn_idx);
    module.section(&exports);

    // ---- Code section ----
    let node_fn_base = IMPORTS_COUNT;
    let mut codes = CodeSection::new();
    for node in &flat {
        let mut f = Function::new(vec![(HELPER_LOCAL_COUNT, ValType::I32)]);
        emit_node_body(&mut f, node, &flat, node_fn_base, &mut ctx);
        f.instructions().end();
        codes.function(&f);
    }
    // `tick` dispatches to helper #0 (root).
    let mut tick_fn = Function::new(vec![]);
    tick_fn
        .instructions()
        .local_get(0)
        .local_get(1)
        .call(node_fn_base)
        .end();
    codes.function(&tick_fn);
    module.section(&codes);

    // ---- Data section ----
    let mut data = DataSection::new();
    if !ctx.string_bytes.is_empty() {
        let offset = ConstExpr::i32_const(STRING_POOL_BASE as i32);
        data.active(0, &offset, ctx.string_bytes.iter().copied());
    }
    module.section(&data);

    let bytes = module.finish();
    let summary = WasmSummary::from_bytes(&bytes).expect("own-generated wasm must summarise");
    CompiledModule { bytes, summary }
}

/// Per-compilation context.
#[derive(Default)]
struct CompileCtx {
    string_bytes: Vec<u8>,
    /// Interned strings → (offset, len).
    strings: Vec<(String, u32, u32)>,
}

impl CompileCtx {
    fn intern(&mut self, s: &str) -> (u32, u32) {
        if let Some((_, offset, len)) = self.strings.iter().find(|(k, _, _)| k == s) {
            return (*offset, *len);
        }
        let offset = STRING_POOL_BASE + self.string_bytes.len() as u32;
        self.string_bytes.extend_from_slice(s.as_bytes());
        let len = s.len() as u32;
        self.strings.push((s.to_string(), offset, len));
        (offset, len)
    }
}

/// Flatten the AST to a pre-order list of `&Node`. Children come after their
/// parent. The root is always at index 0.
fn flatten_tree(root: &Node) -> Vec<&Node> {
    let mut out = Vec::new();
    fn walk<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
        out.push(node);
        if let NodeKind::Combinator { children, .. } = &node.kind {
            for c in children {
                walk(c, out);
            }
        }
    }
    walk(root, &mut out);
    out
}

fn node_verb_args(node: &Node) -> Option<&[Expr]> {
    match &node.kind {
        NodeKind::Verb { args, .. } => Some(args),
        _ => None,
    }
}

/// Look up a child node's ordinal (position in the flattened list).
fn ordinal_of(flat: &[&Node], needle: &Node) -> u32 {
    flat.iter()
        .position(|n| std::ptr::eq(*n, needle as *const Node))
        .expect("every child must appear in flat list") as u32
}

fn intern_strings_in_expr(expr: &Expr, ctx: &mut CompileCtx) {
    match &expr.kind {
        ExprKind::Literal(lit) => match lit {
            Literal::String(s) => {
                ctx.intern(s);
            }
            Literal::Vec3(x, y, z) => {
                intern_strings_in_expr(x, ctx);
                intern_strings_in_expr(y, ctx);
                intern_strings_in_expr(z, ctx);
            }
            Literal::List(items) => {
                for it in items {
                    intern_strings_in_expr(it, ctx);
                }
            }
            Literal::Map(entries) => {
                for (k, v) in entries {
                    ctx.intern(k);
                    intern_strings_in_expr(v, ctx);
                }
            }
            _ => {}
        },
        ExprKind::Ident(_) => {}
        ExprKind::DialogueOption(opt) => {
            intern_strings_in_expr(&opt.label, ctx);
            intern_strings_in_expr(&opt.id, ctx);
        }
    }
}

fn lookup_string(ctx: &CompileCtx, s: &str) -> (u32, u32) {
    ctx.strings
        .iter()
        .find(|(k, _, _)| k == s)
        .map(|(_, p, l)| (*p, *l))
        .unwrap_or((0, 0))
}

// -------- Argument evaluation --------

fn eval_int(expr: &Expr) -> i32 {
    match &expr.kind {
        ExprKind::Literal(Literal::Int(n)) => *n as i32,
        ExprKind::Literal(Literal::Float(f)) => *f as i32,
        _ => 0,
    }
}

fn eval_f32(expr: &Expr) -> f32 {
    match &expr.kind {
        ExprKind::Literal(Literal::Float(f)) => *f as f32,
        ExprKind::Literal(Literal::Int(n)) => *n as f32,
        _ => 0.0,
    }
}

fn eval_entity(expr: &Expr) -> i32 {
    match &expr.kind {
        ExprKind::Ident(name) => crate::ast::builtin_entity_handle(name).unwrap_or(0),
        ExprKind::Literal(Literal::Int(n)) => *n as i32,
        _ => 0,
    }
}

fn eval_vec3(expr: &Expr) -> (f32, f32, f32) {
    if let ExprKind::Literal(Literal::Vec3(x, y, z)) = &expr.kind {
        (eval_f32(x), eval_f32(y), eval_f32(z))
    } else {
        (0.0, 0.0, 0.0)
    }
}

fn eval_string(expr: &Expr, ctx: &CompileCtx) -> (u32, u32) {
    if let ExprKind::Literal(Literal::String(s)) = &expr.kind {
        lookup_string(ctx, s)
    } else {
        (0, 0)
    }
}

// -------- Node emission --------

fn emit_node_body(
    f: &mut Function,
    node: &Node,
    flat: &[&Node],
    node_fn_base: u32,
    ctx: &mut CompileCtx,
) {
    match &node.kind {
        NodeKind::Verb { verb, args } => emit_verb(f, *verb, args, ctx),
        NodeKind::Combinator {
            combinator,
            parameter,
            children,
        } => {
            let child_ordinals: Vec<u32> =
                children.iter().map(|c| ordinal_of(flat, c)).collect();
            emit_combinator(f, *combinator, *parameter, &child_ordinals, node_fn_base);
        }
    }
}

fn emit_verb(f: &mut Function, verb: Verb, args: &[Expr], ctx: &mut CompileCtx) {
    match verb {
        Verb::Spawn => {
            let (ptr, _len) = args
                .first()
                .map(|e| eval_string(e, ctx))
                .unwrap_or((0, 0));
            let (x, y, z) = args.get(1).map(eval_vec3).unwrap_or((0.0, 0.0, 0.0));
            f.instructions()
                .i32_const(ptr as i32)
                .f32_const(Ieee32::from(x))
                .f32_const(Ieee32::from(y))
                .f32_const(Ieee32::from(z))
                .call(IMPORT_SPAWN)
                .drop()
                .i32_const(STATUS_SUCCESS);
        }
        Verb::Move => {
            let entity = args.first().map(eval_entity).unwrap_or(-1);
            let (x, y, z) = args.get(1).map(eval_vec3).unwrap_or((0.0, 0.0, 0.0));
            let speed = args.get(2).map(eval_f32).unwrap_or(1.0);
            f.instructions()
                .i32_const(entity)
                .f32_const(Ieee32::from(x))
                .f32_const(Ieee32::from(y))
                .f32_const(Ieee32::from(z))
                .f32_const(Ieee32::from(speed))
                .call(IMPORT_MOVE);
        }
        Verb::Damage => {
            let entity = args.first().map(eval_entity).unwrap_or(-1);
            let amount = args.get(1).map(eval_int).unwrap_or(0);
            f.instructions()
                .i32_const(entity)
                .i32_const(amount)
                .call(IMPORT_DAMAGE);
        }
        Verb::Trigger => {
            let (ptr, len) = args
                .first()
                .map(|e| eval_string(e, ctx))
                .unwrap_or((0, 0));
            f.instructions()
                .i32_const(ptr as i32)
                .i32_const(len as i32)
                .i32_const(0)
                .i32_const(0)
                .call(IMPORT_TRIGGER)
                .drop()
                .i32_const(STATUS_SUCCESS);
        }
        Verb::Dialogue => {
            let speaker = args.first().map(eval_entity).unwrap_or(-1);
            let (ptr, len) = args
                .get(1)
                .map(|e| eval_string(e, ctx))
                .unwrap_or((0, 0));
            f.instructions()
                .i32_const(speaker)
                .i32_const(ptr as i32)
                .i32_const(len as i32)
                .i32_const(0)
                .i32_const(0)
                .call(IMPORT_DIALOGUE)
                .drop()
                .i32_const(STATUS_SUCCESS);
        }
    }
}

fn emit_combinator(
    f: &mut Function,
    combinator: Combinator,
    parameter: Option<i64>,
    children: &[u32],
    node_fn_base: u32,
) {
    match combinator {
        Combinator::Sequence => emit_sequence(f, children, node_fn_base),
        Combinator::Selector => emit_selector(f, children, node_fn_base),
        Combinator::Parallel => emit_parallel(f, children, node_fn_base),
        Combinator::Invert => emit_invert(f, children[0], node_fn_base),
        Combinator::Retry => emit_retry(
            f,
            children[0],
            parameter.unwrap_or(1).max(1) as i32,
            node_fn_base,
        ),
        Combinator::Timeout => emit_timeout(
            f,
            children[0],
            parameter.unwrap_or(i32::MAX as i64).min(i32::MAX as i64) as i32,
            node_fn_base,
        ),
    }
}

fn pass_tick_args(f: &mut Function) {
    f.instructions().local_get(0).local_get(1);
}

/// Sequence: Success if all succeed; otherwise first non-Success status.
///
/// Compiled as a block that leaves an i32 on its stack. Each non-last child is
/// evaluated into scratch A; if status != Success we branch out of the block
/// with that status. The final child's status is the fallthrough result.
fn emit_sequence(f: &mut Function, children: &[u32], base: u32) {
    f.instructions().block(BlockType::Result(ValType::I32));
    let last = children.len() - 1;
    for (i, child) in children.iter().enumerate() {
        pass_tick_args(f);
        f.instructions().call(base + *child);
        if i == last {
            // Leave status on stack; block ends.
        } else {
            // Stack: [status]
            f.instructions().local_tee(LOCAL_SCRATCH_A);
            f.instructions().i32_const(STATUS_SUCCESS).i32_ne();
            f.instructions().if_(BlockType::Empty);
            f.instructions().local_get(LOCAL_SCRATCH_A).br(1);
            f.instructions().end();
        }
    }
    f.instructions().end();
}

/// Selector: first child to return non-Failure is the result; if all Failure,
/// returns Failure (the last child's status).
fn emit_selector(f: &mut Function, children: &[u32], base: u32) {
    f.instructions().block(BlockType::Result(ValType::I32));
    let last = children.len() - 1;
    for (i, child) in children.iter().enumerate() {
        pass_tick_args(f);
        f.instructions().call(base + *child);
        if i == last {
        } else {
            f.instructions().local_tee(LOCAL_SCRATCH_A);
            f.instructions().i32_const(STATUS_FAILURE).i32_ne();
            f.instructions().if_(BlockType::Empty);
            f.instructions().local_get(LOCAL_SCRATCH_A).br(1);
            f.instructions().end();
        }
    }
    f.instructions().end();
}

/// Parallel: evaluate all; Failure if any Failure, else Running if any Running,
/// else Success.
fn emit_parallel(f: &mut Function, children: &[u32], base: u32) {
    // scratch A = any_failure, B = any_running, C = last status
    f.instructions().i32_const(0).local_set(LOCAL_SCRATCH_A);
    f.instructions().i32_const(0).local_set(LOCAL_SCRATCH_B);
    for child in children {
        pass_tick_args(f);
        f.instructions().call(base + *child);
        f.instructions().local_tee(LOCAL_SCRATCH_C);
        // if status == Failure
        f.instructions().i32_const(STATUS_FAILURE).i32_eq();
        f.instructions().if_(BlockType::Empty);
        f.instructions().i32_const(1).local_set(LOCAL_SCRATCH_A);
        f.instructions().end();
        // if status == Running
        f.instructions()
            .local_get(LOCAL_SCRATCH_C)
            .i32_const(STATUS_RUNNING)
            .i32_eq();
        f.instructions().if_(BlockType::Empty);
        f.instructions().i32_const(1).local_set(LOCAL_SCRATCH_B);
        f.instructions().end();
    }
    f.instructions()
        .local_get(LOCAL_SCRATCH_A)
        .if_(BlockType::Result(ValType::I32))
        .i32_const(STATUS_FAILURE)
        .else_()
        .local_get(LOCAL_SCRATCH_B)
        .if_(BlockType::Result(ValType::I32))
        .i32_const(STATUS_RUNNING)
        .else_()
        .i32_const(STATUS_SUCCESS)
        .end()
        .end();
}

/// Invert: Success <-> Failure, Running unchanged.
fn emit_invert(f: &mut Function, child: u32, base: u32) {
    pass_tick_args(f);
    f.instructions().call(base + child);
    f.instructions().local_tee(LOCAL_SCRATCH_A);
    f.instructions().i32_const(STATUS_SUCCESS).i32_eq();
    f.instructions().if_(BlockType::Result(ValType::I32));
    f.instructions().i32_const(STATUS_FAILURE);
    f.instructions().else_();
    f.instructions()
        .local_get(LOCAL_SCRATCH_A)
        .i32_const(STATUS_FAILURE)
        .i32_eq();
    f.instructions().if_(BlockType::Result(ValType::I32));
    f.instructions().i32_const(STATUS_SUCCESS);
    f.instructions().else_();
    f.instructions().local_get(LOCAL_SCRATCH_A);
    f.instructions().end();
    f.instructions().end();
}

/// Retry(n): run child; on Failure, retry up to `n` times this tick. If all
/// attempts fail, returns Failure. First non-Failure status is the result.
fn emit_retry(f: &mut Function, child: u32, max: i32, base: u32) {
    let max = max.max(1);
    // scratch A = status, B = counter remaining
    f.instructions().i32_const(max).local_set(LOCAL_SCRATCH_B);
    f.instructions().block(BlockType::Result(ValType::I32));
    f.instructions().loop_(BlockType::Empty);
    pass_tick_args(f);
    f.instructions().call(base + child);
    f.instructions().local_tee(LOCAL_SCRATCH_A);
    f.instructions().i32_const(STATUS_FAILURE).i32_ne();
    f.instructions().if_(BlockType::Empty);
    // Non-failure — break outer block with the status.
    f.instructions().local_get(LOCAL_SCRATCH_A).br(2);
    f.instructions().end();
    // Decrement counter.
    f.instructions()
        .local_get(LOCAL_SCRATCH_B)
        .i32_const(1)
        .i32_sub()
        .local_tee(LOCAL_SCRATCH_B);
    f.instructions().i32_const(0).i32_gt_s();
    f.instructions().br_if(0);
    f.instructions().end();
    // Fell out of loop without early break: all attempts failed.
    f.instructions().i32_const(STATUS_FAILURE);
    f.instructions().end();
}

/// Timeout(ms): run child; if it returns Running and elapsed > ms, Failure.
fn emit_timeout(f: &mut Function, child: u32, ms: i32, base: u32) {
    // scratch A = status, B = timer handle
    f.instructions()
        .call(IMPORT_TIMER_START)
        .local_set(LOCAL_SCRATCH_B);
    pass_tick_args(f);
    f.instructions().call(base + child);
    f.instructions().local_tee(LOCAL_SCRATCH_A);
    f.instructions().i32_const(STATUS_RUNNING).i32_eq();
    f.instructions().if_(BlockType::Result(ValType::I32));
    f.instructions()
        .local_get(LOCAL_SCRATCH_B)
        .call(IMPORT_TIMER_ELAPSED_MS);
    f.instructions().i32_const(ms);
    f.instructions().i32_gt_s();
    f.instructions().if_(BlockType::Result(ValType::I32));
    f.instructions().i32_const(STATUS_FAILURE);
    f.instructions().else_();
    f.instructions().i32_const(STATUS_RUNNING);
    f.instructions().end();
    f.instructions().else_();
    f.instructions().local_get(LOCAL_SCRATCH_A);
    f.instructions().end();
}

/// The list of host imports the compiled module will require.
pub fn required_imports() -> Vec<(String, String)> {
    [
        "spawn",
        "move",
        "damage",
        "trigger",
        "dialogue",
        "timer_start",
        "timer_elapsed_ms",
    ]
    .iter()
    .map(|n| ("aether".to_string(), n.to_string()))
    .collect()
}

/// The set of exports every compiled behavior module advertises.
pub fn required_exports() -> BTreeSet<(String, String)> {
    [
        ("memory".to_string(), "memory".to_string()),
        ("tick".to_string(), "func".to_string()),
    ]
    .into_iter()
    .collect()
}

