//! # Aether Behavior DSL (BDSL)
//!
//! A narrow typed DSL for authoring AI-first behavior scripts in Aether.
//! Programs are parsed, type-checked, and compiled to a validated
//! WebAssembly module that imports host functions from a `"aether"` namespace.
//!
//! ## Mini-spec (authoritative for this crate)
//!
//! ### Module
//! ```text
//! behavior <Name> {
//!     @caps(Cap1, Cap2)   // optional, default ()
//!     version <n>         // required, u32
//!     <node>              // root behavior-tree node
//! }
//! ```
//!
//! ### The 5 MVP verbs
//!
//! | Verb       | Signature                                                          | Effect      |
//! |------------|--------------------------------------------------------------------|-------------|
//! | `spawn`    | `(prototype: String, position: Vec3) -> EntityRef`                 | Pure        |
//! | `move`     | `(entity: EntityRef, to: Vec3, speed: Float) -> BehaviorStatus`    | Movement    |
//! | `damage`   | `(target: EntityRef, amount: Int) -> BehaviorStatus`               | Combat      |
//! | `trigger`  | `(event_name: String, data: Map<String, Any>) -> ()`               | Network     |
//! | `dialogue` | `(speaker: EntityRef, text: String, opts: List<DialogueOption>) -> ChoiceId` | Pure |
//!
//! ### Combinators
//! * `sequence { ... }` — succeed if all succeed
//! * `selector { ... }` — succeed on first success
//! * `parallel { ... }` — run all, combine
//! * `invert { <node> }` — swap Success/Failure
//! * `retry(n) { <node> }` — retry on Failure up to `n` times
//! * `timeout(ms) { <node> }` — fail if child is still Running after `ms`
//!
//! ### Types
//! `Int`, `Float`, `Bool`, `String`, `EntityRef`, `Vec3`, `Timer`, `BehaviorStatus`
//! (`Success`/`Failure`/`Running`), `DialogueOption`, `ChoiceId`, plus
//! `List<T>`, `Map<String, Any>`. No user-defined types.
//!
//! ### Effects
//! `Pure`, `Movement`, `Combat`, `Network`, `Persistence`, `Economy`.
//! Verbs declare their effect; combinators union. A module's effect set
//! requires matching capabilities in `@caps(...)`.
//!
//! ### Capability tokens
//! `@caps(Network, Economy)` — the compiler refuses a behavior that uses a
//! capability-gated effect without the matching cap.
//!
//! ### Error codes
//! Structured errors `BDSL-E0001` through `BDSL-E0020` — see
//! [`error::BehaviorDslError`].
//!
//! ### WASM contract
//! Emitted modules export `memory` and `tick(world: i32, entity: i32) -> i32`.
//! The returned `i32` encodes `BehaviorStatus`: `0=Success`, `1=Failure`,
//! `2=Running`. Each declared verb is imported as `aether.<verb>` with a
//! fixed signature; see [`compile::required_imports`].
//!
//! ## Pipeline
//!
//! ```text
//!   source -> parse() -> typeck::check() -> compile::compile_module() -> Vec<u8>
//! ```
//!
//! ## Typical usage
//!
//! ```no_run
//! use aether_behavior_dsl::{parse, typeck, compile};
//!
//! let src = r#"
//!   behavior Patrol {
//!     @caps(Movement)
//!     version 1
//!     sequence {
//!       move(self, vec3(0.0, 0.0, 0.0), 1.5);
//!       move(self, vec3(10.0, 0.0, 0.0), 1.5);
//!     }
//!   }
//! "#;
//! let ast = parse(src).expect("parses");
//! let checked = typeck::check(ast).expect("type-checks");
//! let wasm = compile::compile_module(&checked);
//! assert!(wasmparser_validate(&wasm.bytes).is_ok());
//! # fn wasmparser_validate(_bytes: &[u8]) -> Result<(), ()> { Ok(()) }
//! ```

pub mod ast;
pub mod caps;
pub mod compile;
pub mod effects;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod typeck;
pub mod types;
pub mod visual_graph;

pub use ast::{Combinator, Module, Node, Span, Verb};
pub use caps::{Capability, CapabilitySet};
pub use compile::{compile_module, WasmSummary};
pub use effects::{Effect, EffectSet};
pub use error::{BehaviorDslError, BehaviorDslResult};
pub use parser::parse;
pub use typeck::{check, CheckedModule};
pub use types::{BehaviorStatus, Type};
pub use visual_graph::{ast_to_graph, graph_to_ast, modules_structurally_equal, VisualGraph};
