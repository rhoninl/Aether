//! Visual-graph model for the Behavior DSL.
//!
//! A [`VisualGraph`] is a flat list of [`VisualNode`]s plus parent/child
//! edges. [`ast_to_graph`] and [`graph_to_ast`] are inverses modulo layout
//! metadata — that is, `graph_to_ast(ast_to_graph(m)) == m` ignoring source
//! spans and graph layout positions.
//!
//! The graph is what a visual-programming front-end (editor) would render.

use serde::{Deserialize, Serialize};

use crate::ast::*;
use crate::caps::CapabilitySet;

/// Compact representation of a single behavior node as a visual graph node.
///
/// For verbs, `kind == NodeKindTag::Verb(...)` and the verb arguments are
/// stored as pre-serialised JSON blobs inside `arg_json`. This keeps the graph
/// representation uniform (no recursive `Expr` type to store in a node) and
/// serialisable via a single `#[derive(Serialize)]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualNode {
    pub id: u32,
    pub kind: NodeKindTag,
    /// Visual layout position (ignored by the round-trip comparison).
    pub layout: Layout,
    /// For verbs: JSON-serialised [`Expr`] values, one per argument.
    /// For combinators: empty.
    pub arg_json: Vec<String>,
    /// For `retry(n)` / `timeout(ms)`: the integer parameter.
    pub parameter: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKindTag {
    Verb(Verb),
    Combinator(Combinator),
}

/// Optional layout metadata — ignored by `graph_to_ast` semantics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub x: f32,
    pub y: f32,
}

/// A directed edge from parent to child, with a sibling-order index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualEdge {
    pub parent: u32,
    pub child: u32,
    /// Position among the parent's children (0-indexed).
    pub order: u32,
}

/// The full visual graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualGraph {
    pub behavior_name: String,
    pub caps: CapabilitySet,
    pub version: u32,
    pub root_id: u32,
    pub nodes: Vec<VisualNode>,
    pub edges: Vec<VisualEdge>,
}

/// Errors from `graph_to_ast`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    RootNotFound(u32),
    OrphanNode(u32),
    DuplicateNodeId(u32),
    InvalidArgJson { node_id: u32, reason: String },
    CycleDetected,
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::RootNotFound(id) => write!(f, "root node {} not found", id),
            GraphError::OrphanNode(id) => write!(f, "orphan node {}", id),
            GraphError::DuplicateNodeId(id) => write!(f, "duplicate node id {}", id),
            GraphError::InvalidArgJson { node_id, reason } => {
                write!(f, "invalid arg_json on node {}: {}", node_id, reason)
            }
            GraphError::CycleDetected => write!(f, "cycle detected in graph"),
        }
    }
}

impl std::error::Error for GraphError {}

/// Convert a Module AST into a visual graph.
pub fn ast_to_graph(module: &Module) -> VisualGraph {
    let mut ctx = BuildCtx {
        next_id: 0,
        nodes: Vec::new(),
        edges: Vec::new(),
    };
    let root_id = build_node(&mut ctx, &module.root, None, 0);
    VisualGraph {
        behavior_name: module.name.clone(),
        caps: module.caps.clone(),
        version: module.version,
        root_id,
        nodes: ctx.nodes,
        edges: ctx.edges,
    }
}

/// Convert a visual graph back into a Module AST.
pub fn graph_to_ast(graph: &VisualGraph) -> Result<Module, GraphError> {
    // Basic validation.
    let mut seen = std::collections::BTreeSet::new();
    for n in &graph.nodes {
        if !seen.insert(n.id) {
            return Err(GraphError::DuplicateNodeId(n.id));
        }
    }
    // Build a parent→[(order, child)] map.
    let mut children_of: std::collections::BTreeMap<u32, Vec<(u32, u32)>> =
        std::collections::BTreeMap::new();
    for e in &graph.edges {
        children_of
            .entry(e.parent)
            .or_default()
            .push((e.order, e.child));
    }
    for list in children_of.values_mut() {
        list.sort_by_key(|(ord, _)| *ord);
    }
    // Reject cycles via a traversal that counts visits vs node count.
    let mut visited = std::collections::BTreeSet::new();
    fn visit(
        id: u32,
        graph: &VisualGraph,
        children_of: &std::collections::BTreeMap<u32, Vec<(u32, u32)>>,
        visited: &mut std::collections::BTreeSet<u32>,
    ) -> Result<(), GraphError> {
        if !visited.insert(id) {
            return Err(GraphError::CycleDetected);
        }
        if let Some(kids) = children_of.get(&id) {
            for (_, child) in kids {
                visit(*child, graph, children_of, visited)?;
            }
        }
        Ok(())
    }
    if !graph.nodes.iter().any(|n| n.id == graph.root_id) {
        return Err(GraphError::RootNotFound(graph.root_id));
    }
    visit(graph.root_id, graph, &children_of, &mut visited)?;
    if visited.len() != graph.nodes.len() {
        // Find any orphan.
        for n in &graph.nodes {
            if !visited.contains(&n.id) {
                return Err(GraphError::OrphanNode(n.id));
            }
        }
    }
    let root = build_ast_node(graph.root_id, graph, &children_of)?;
    Ok(Module {
        name: graph.behavior_name.clone(),
        caps: graph.caps.clone(),
        version: graph.version,
        root,
        span: Span::DUMMY,
    })
}

/// Structural-equality check of two Modules that ignores spans.
pub fn modules_structurally_equal(a: &Module, b: &Module) -> bool {
    a.name == b.name
        && a.caps == b.caps
        && a.version == b.version
        && nodes_structurally_equal(&a.root, &b.root)
}

fn nodes_structurally_equal(a: &Node, b: &Node) -> bool {
    match (&a.kind, &b.kind) {
        (
            NodeKind::Verb {
                verb: va,
                args: aa,
            },
            NodeKind::Verb {
                verb: vb,
                args: ab,
            },
        ) => va == vb && exprs_structurally_equal(aa, ab),
        (
            NodeKind::Combinator {
                combinator: ca,
                parameter: pa,
                children: ka,
            },
            NodeKind::Combinator {
                combinator: cb,
                parameter: pb,
                children: kb,
            },
        ) => {
            ca == cb
                && pa == pb
                && ka.len() == kb.len()
                && ka.iter().zip(kb).all(|(x, y)| nodes_structurally_equal(x, y))
        }
        _ => false,
    }
}

fn exprs_structurally_equal(a: &[Expr], b: &[Expr]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| expr_eq(x, y))
}

fn expr_eq(a: &Expr, b: &Expr) -> bool {
    match (&a.kind, &b.kind) {
        (ExprKind::Ident(xa), ExprKind::Ident(xb)) => xa == xb,
        (ExprKind::Literal(la), ExprKind::Literal(lb)) => literal_eq(la, lb),
        (ExprKind::DialogueOption(oa), ExprKind::DialogueOption(ob)) => {
            expr_eq(&oa.label, &ob.label) && expr_eq(&oa.id, &ob.id)
        }
        _ => false,
    }
}

fn literal_eq(a: &Literal, b: &Literal) -> bool {
    match (a, b) {
        (Literal::Int(x), Literal::Int(y)) => x == y,
        (Literal::Float(x), Literal::Float(y)) => (x - y).abs() < 1e-9,
        (Literal::Bool(x), Literal::Bool(y)) => x == y,
        (Literal::String(x), Literal::String(y)) => x == y,
        (Literal::Vec3(ax, ay, az), Literal::Vec3(bx, by, bz)) => {
            expr_eq(ax, bx) && expr_eq(ay, by) && expr_eq(az, bz)
        }
        (Literal::List(xs), Literal::List(ys)) => {
            xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| expr_eq(x, y))
        }
        (Literal::Map(xs), Literal::Map(ys)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .zip(ys)
                    .all(|((ka, va), (kb, vb))| ka == kb && expr_eq(va, vb))
        }
        _ => false,
    }
}

struct BuildCtx {
    next_id: u32,
    nodes: Vec<VisualNode>,
    edges: Vec<VisualEdge>,
}

fn build_node(ctx: &mut BuildCtx, node: &Node, parent: Option<u32>, order: u32) -> u32 {
    let id = ctx.next_id;
    ctx.next_id += 1;

    let (kind, arg_json, parameter) = match &node.kind {
        NodeKind::Verb { verb, args } => {
            let arg_json = args
                .iter()
                .map(|e| serde_json::to_string(e).expect("expr is always JSON-serialisable"))
                .collect();
            (NodeKindTag::Verb(*verb), arg_json, None)
        }
        NodeKind::Combinator {
            combinator,
            parameter,
            ..
        } => (NodeKindTag::Combinator(*combinator), Vec::new(), *parameter),
    };

    ctx.nodes.push(VisualNode {
        id,
        kind,
        layout: Layout::default(),
        arg_json,
        parameter,
    });
    if let Some(p) = parent {
        ctx.edges.push(VisualEdge {
            parent: p,
            child: id,
            order,
        });
    }

    if let NodeKind::Combinator { children, .. } = &node.kind {
        for (i, child) in children.iter().enumerate() {
            build_node(ctx, child, Some(id), i as u32);
        }
    }
    id
}

fn build_ast_node(
    id: u32,
    graph: &VisualGraph,
    children_of: &std::collections::BTreeMap<u32, Vec<(u32, u32)>>,
) -> Result<Node, GraphError> {
    let node = graph
        .nodes
        .iter()
        .find(|n| n.id == id)
        .ok_or(GraphError::OrphanNode(id))?;
    let kind = match &node.kind {
        NodeKindTag::Verb(verb) => {
            let args: Result<Vec<Expr>, GraphError> = node
                .arg_json
                .iter()
                .map(|s| {
                    serde_json::from_str::<Expr>(s).map_err(|e| GraphError::InvalidArgJson {
                        node_id: id,
                        reason: e.to_string(),
                    })
                })
                .collect();
            NodeKind::Verb {
                verb: *verb,
                args: args?,
            }
        }
        NodeKindTag::Combinator(c) => {
            let kids = children_of
                .get(&id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|(_, cid)| build_ast_node(cid, graph, children_of))
                .collect::<Result<Vec<_>, _>>()?;
            NodeKind::Combinator {
                combinator: *c,
                parameter: node.parameter,
                children: kids,
            }
        }
    };
    Ok(Node {
        kind,
        span: Span::DUMMY,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn round_trip_simple_sequence() {
        let src = "behavior T { @caps(Movement) version 1 sequence { move(self, vec3(0,0,0), 1.0); move(self, vec3(1,0,0), 1.0); } }";
        let ast = parse(src).unwrap();
        let graph = ast_to_graph(&ast);
        let back = graph_to_ast(&graph).unwrap();
        assert!(modules_structurally_equal(&ast, &back));
    }

    #[test]
    fn round_trip_selector_with_invert() {
        let src = "behavior T { @caps(Combat) version 2 selector { invert { damage(self, 1) } damage(self, 5) } }";
        let ast = parse(src).unwrap();
        let graph = ast_to_graph(&ast);
        let back = graph_to_ast(&graph).unwrap();
        assert!(modules_structurally_equal(&ast, &back));
    }
}
