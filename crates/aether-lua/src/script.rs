/// Metadata parsed from Lua script annotation comments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuaScriptMeta {
    pub stage: String,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
}

const DEFAULT_STAGE: &str = "PrePhysics";

/// Parses @stage, @reads, and @writes annotations from Lua source comments.
///
/// Expected format in Lua comments:
/// ```lua
/// -- @stage: PostPhysics
/// -- @reads: Transform, RigidBody
/// -- @writes: Velocity
/// ```
pub fn parse_metadata(source: &str) -> LuaScriptMeta {
    let mut stage = DEFAULT_STAGE.to_string();
    let mut reads = Vec::new();
    let mut writes = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("--") {
            continue;
        }
        let comment_body = trimmed.trim_start_matches('-').trim();

        if let Some(rest) = comment_body.strip_prefix("@stage:") {
            let s = rest.trim();
            if !s.is_empty() {
                stage = s.to_string();
            }
        } else if let Some(rest) = comment_body.strip_prefix("@reads:") {
            for item in rest.split(',') {
                let item = item.trim();
                if !item.is_empty() {
                    reads.push(item.to_string());
                }
            }
        } else if let Some(rest) = comment_body.strip_prefix("@writes:") {
            for item in rest.split(',') {
                let item = item.trim();
                if !item.is_empty() {
                    writes.push(item.to_string());
                }
            }
        }
    }

    LuaScriptMeta {
        stage,
        reads,
        writes,
    }
}

/// Maps an ECS stage name to the corresponding Lua hook function name.
pub fn stage_to_hook_name(stage: &str) -> &str {
    match stage {
        "PrePhysics" => "on_tick",
        "PostPhysics" => "on_post_physics",
        "Input" => "on_input",
        "Animation" => "on_animate",
        "PreRender" => "on_pre_render",
        "NetworkSync" => "on_network_sync",
        _ => "on_tick",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stage() {
        let source = "-- @stage: PostPhysics\nfunction on_post_physics() end";
        let meta = parse_metadata(source);
        assert_eq!(meta.stage, "PostPhysics");
    }

    #[test]
    fn test_parse_reads() {
        let source = "-- @reads: Transform, RigidBody\nfunction on_tick() end";
        let meta = parse_metadata(source);
        assert_eq!(meta.reads, vec!["Transform", "RigidBody"]);
    }

    #[test]
    fn test_parse_writes() {
        let source = "-- @writes: Velocity\nfunction on_tick() end";
        let meta = parse_metadata(source);
        assert_eq!(meta.writes, vec!["Velocity"]);
    }

    #[test]
    fn test_parse_default_stage() {
        let source = "function on_tick() end";
        let meta = parse_metadata(source);
        assert_eq!(meta.stage, "PrePhysics");
    }

    #[test]
    fn test_parse_all_annotations() {
        let source = "\
-- @stage: PostPhysics
-- @reads: Transform, RigidBody
-- @writes: Velocity
function on_post_physics() end";
        let meta = parse_metadata(source);
        assert_eq!(meta.stage, "PostPhysics");
        assert_eq!(meta.reads, vec!["Transform", "RigidBody"]);
        assert_eq!(meta.writes, vec!["Velocity"]);
    }

    #[test]
    fn test_stage_to_hook_name_prephysics() {
        assert_eq!(stage_to_hook_name("PrePhysics"), "on_tick");
    }

    #[test]
    fn test_stage_to_hook_name_postphysics() {
        assert_eq!(stage_to_hook_name("PostPhysics"), "on_post_physics");
    }

    #[test]
    fn test_stage_to_hook_name_input() {
        assert_eq!(stage_to_hook_name("Input"), "on_input");
    }

    #[test]
    fn test_stage_to_hook_name_animation() {
        assert_eq!(stage_to_hook_name("Animation"), "on_animate");
    }

    #[test]
    fn test_stage_to_hook_name_prerender() {
        assert_eq!(stage_to_hook_name("PreRender"), "on_pre_render");
    }

    #[test]
    fn test_stage_to_hook_name_networksync() {
        assert_eq!(stage_to_hook_name("NetworkSync"), "on_network_sync");
    }

    #[test]
    fn test_annotations_with_extra_whitespace() {
        let source = "--  @stage:  PostPhysics \n--  @reads:  Transform ,  RigidBody \n";
        let meta = parse_metadata(source);
        assert_eq!(meta.stage, "PostPhysics");
        assert_eq!(meta.reads, vec!["Transform", "RigidBody"]);
    }
}
