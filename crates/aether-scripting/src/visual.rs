use crate::artifact::ScriptLanguage;

#[derive(Debug, Clone)]
pub struct VisualScriptNode {
    pub id: u32,
    pub opcode: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct VisualScriptGraph {
    pub nodes: Vec<VisualScriptNode>,
    pub links: Vec<(u32, u32)>,
}

#[derive(Debug)]
pub enum VisualScriptCompileError {
    EmptyGraph,
    DanglingNode {
        node_id: u32,
        context: &'static str,
    },
    BackendUnavailable,
    InvalidNodeId(u32),
}

pub trait VisualScriptCompiler {
    fn target_language(&self) -> ScriptLanguage;
    fn compile(&self, graph: &VisualScriptGraph) -> Result<Vec<u8>, VisualScriptCompileError>;
}

pub struct WasmVisualCompiler {
    pub language: ScriptLanguage,
}

impl Default for WasmVisualCompiler {
    fn default() -> Self {
        Self {
            language: ScriptLanguage::Unknown,
        }
    }
}

impl VisualScriptCompiler for WasmVisualCompiler {
    fn target_language(&self) -> ScriptLanguage {
        self.language
    }

    fn compile(&self, graph: &VisualScriptGraph) -> Result<Vec<u8>, VisualScriptCompileError> {
        if graph.nodes.is_empty() {
            return Err(VisualScriptCompileError::EmptyGraph);
        }
        for (from, to) in &graph.links {
            if !graph.nodes.iter().any(|node| node.id == *from) {
                return Err(VisualScriptCompileError::InvalidNodeId(*from));
            }
            if !graph.nodes.iter().any(|node| node.id == *to) {
                return Err(VisualScriptCompileError::InvalidNodeId(*to));
            }
        }

        if graph.links.is_empty() && graph.nodes.len() > 1 {
            return Err(VisualScriptCompileError::DanglingNode {
                node_id: graph.nodes[1].id,
                context: "single disconnected branch",
            });
        }

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"WASM-VISUAL-COMPILER-V1:");
        bytes.extend_from_slice(self.language_name().as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(format!("nodes={}", graph.nodes.len()).as_bytes());
        Ok(bytes)
    }
}

impl WasmVisualCompiler {
    fn language_name(&self) -> &'static str {
        match self.language {
            ScriptLanguage::Rust => "rust",
            ScriptLanguage::AssemblyScript => "assemblyscript",
            ScriptLanguage::CCpp => "cpp",
            ScriptLanguage::GoTinyGo => "tinygo",
            ScriptLanguage::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_visual_script_from_nodes() {
        let compiler = WasmVisualCompiler {
            language: ScriptLanguage::AssemblyScript,
        };
        let graph = VisualScriptGraph {
            nodes: vec![
                VisualScriptNode {
                    id: 1,
                    opcode: "spawn".to_string(),
                    args: vec!["player".to_string()],
                },
                VisualScriptNode {
                    id: 2,
                    opcode: "play_sound".to_string(),
                    args: vec!["alarm".to_string()],
                },
            ],
            links: vec![(1, 2)],
        };
        let artifact = compiler.compile(&graph).unwrap();
        assert!(artifact.starts_with(b"WASM-VISUAL-COMPILER-V1:"));
    }
}
