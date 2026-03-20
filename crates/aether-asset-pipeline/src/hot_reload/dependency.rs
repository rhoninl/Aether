//! Directed acyclic graph for tracking asset dependencies.
//!
//! An edge `A -> B` means "asset A depends on asset B".
//! When B changes, all ancestors of B (assets depending on B) must also reload.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

/// A DAG tracking which assets depend on which other assets.
#[derive(Debug, Default)]
pub struct DependencyGraph {
    /// Forward edges: asset -> set of assets it depends on.
    depends_on: HashMap<PathBuf, HashSet<PathBuf>>,
    /// Reverse edges: asset -> set of assets that depend on it.
    depended_by: HashMap<PathBuf, HashSet<PathBuf>>,
}

impl DependencyGraph {
    /// Create an empty dependency graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register that `dependent` depends on `dependency`.
    /// (e.g., a material depends on a texture)
    pub fn add_dependency(&mut self, dependent: PathBuf, dependency: PathBuf) {
        self.depends_on
            .entry(dependent.clone())
            .or_default()
            .insert(dependency.clone());
        self.depended_by
            .entry(dependency)
            .or_default()
            .insert(dependent);
    }

    /// Remove a specific dependency edge.
    pub fn remove_dependency(&mut self, dependent: &PathBuf, dependency: &PathBuf) {
        if let Some(deps) = self.depends_on.get_mut(dependent) {
            deps.remove(dependency);
            if deps.is_empty() {
                self.depends_on.remove(dependent);
            }
        }
        if let Some(rdeps) = self.depended_by.get_mut(dependency) {
            rdeps.remove(dependent);
            if rdeps.is_empty() {
                self.depended_by.remove(dependency);
            }
        }
    }

    /// Remove all edges involving the given asset (both as dependent and dependency).
    pub fn remove_asset(&mut self, asset: &PathBuf) {
        // Remove forward edges from this asset
        if let Some(deps) = self.depends_on.remove(asset) {
            for dep in deps {
                if let Some(rdeps) = self.depended_by.get_mut(&dep) {
                    rdeps.remove(asset);
                    if rdeps.is_empty() {
                        self.depended_by.remove(&dep);
                    }
                }
            }
        }

        // Remove reverse edges pointing to this asset
        if let Some(rdeps) = self.depended_by.remove(asset) {
            for rdep in rdeps {
                if let Some(deps) = self.depends_on.get_mut(&rdep) {
                    deps.remove(asset);
                    if deps.is_empty() {
                        self.depends_on.remove(&rdep);
                    }
                }
            }
        }
    }

    /// Get all assets that transitively depend on the given asset (BFS).
    /// Does NOT include the changed asset itself.
    pub fn get_dependents(&self, changed: &PathBuf) -> Vec<PathBuf> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(direct) = self.depended_by.get(changed) {
            for d in direct {
                if visited.insert(d.clone()) {
                    queue.push_back(d.clone());
                }
            }
        }

        while let Some(current) = queue.pop_front() {
            if let Some(parents) = self.depended_by.get(&current) {
                for p in parents {
                    if visited.insert(p.clone()) {
                        queue.push_back(p.clone());
                    }
                }
            }
        }

        let mut result: Vec<PathBuf> = visited.into_iter().collect();
        result.sort();
        result
    }

    /// Get the direct dependencies of an asset.
    pub fn get_dependencies(&self, asset: &PathBuf) -> Vec<PathBuf> {
        self.depends_on
            .get(asset)
            .map(|deps| {
                let mut v: Vec<PathBuf> = deps.iter().cloned().collect();
                v.sort();
                v
            })
            .unwrap_or_default()
    }

    /// Total number of unique edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.depends_on.values().map(|s| s.len()).sum()
    }

    /// Total number of unique assets tracked in the graph.
    pub fn asset_count(&self) -> usize {
        let mut all: HashSet<&PathBuf> = HashSet::new();
        for (k, vs) in &self.depends_on {
            all.insert(k);
            for v in vs {
                all.insert(v);
            }
        }
        all.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_graph_is_empty() {
        let g = DependencyGraph::new();
        assert_eq!(g.edge_count(), 0);
        assert_eq!(g.asset_count(), 0);
    }

    #[test]
    fn add_single_dependency() {
        let mut g = DependencyGraph::new();
        g.add_dependency(PathBuf::from("material.mat"), PathBuf::from("texture.png"));
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.asset_count(), 2);
    }

    #[test]
    fn get_direct_dependencies() {
        let mut g = DependencyGraph::new();
        g.add_dependency(PathBuf::from("material.mat"), PathBuf::from("texture.png"));
        let deps = g.get_dependencies(&PathBuf::from("material.mat"));
        assert_eq!(deps, vec![PathBuf::from("texture.png")]);
    }

    #[test]
    fn get_dependents_of_texture() {
        let mut g = DependencyGraph::new();
        g.add_dependency(PathBuf::from("material.mat"), PathBuf::from("texture.png"));
        let dependents = g.get_dependents(&PathBuf::from("texture.png"));
        assert_eq!(dependents, vec![PathBuf::from("material.mat")]);
    }

    #[test]
    fn get_dependents_of_asset_with_no_dependents() {
        let mut g = DependencyGraph::new();
        g.add_dependency(PathBuf::from("material.mat"), PathBuf::from("texture.png"));
        let dependents = g.get_dependents(&PathBuf::from("material.mat"));
        assert!(dependents.is_empty());
    }

    #[test]
    fn transitive_dependents() {
        let mut g = DependencyGraph::new();
        // scene depends on material, material depends on texture
        g.add_dependency(PathBuf::from("scene.toml"), PathBuf::from("material.mat"));
        g.add_dependency(PathBuf::from("material.mat"), PathBuf::from("texture.png"));
        // Changing texture should cascade to material and scene
        let dependents = g.get_dependents(&PathBuf::from("texture.png"));
        assert_eq!(
            dependents,
            vec![PathBuf::from("material.mat"), PathBuf::from("scene.toml"),]
        );
    }

    #[test]
    fn multiple_dependents_of_same_asset() {
        let mut g = DependencyGraph::new();
        g.add_dependency(PathBuf::from("mat_a.mat"), PathBuf::from("shared.png"));
        g.add_dependency(PathBuf::from("mat_b.mat"), PathBuf::from("shared.png"));
        let dependents = g.get_dependents(&PathBuf::from("shared.png"));
        assert_eq!(dependents.len(), 2);
        assert!(dependents.contains(&PathBuf::from("mat_a.mat")));
        assert!(dependents.contains(&PathBuf::from("mat_b.mat")));
    }

    #[test]
    fn remove_dependency_edge() {
        let mut g = DependencyGraph::new();
        g.add_dependency(PathBuf::from("material.mat"), PathBuf::from("texture.png"));
        g.remove_dependency(
            &PathBuf::from("material.mat"),
            &PathBuf::from("texture.png"),
        );
        assert_eq!(g.edge_count(), 0);
        let dependents = g.get_dependents(&PathBuf::from("texture.png"));
        assert!(dependents.is_empty());
    }

    #[test]
    fn remove_asset_clears_all_edges() {
        let mut g = DependencyGraph::new();
        g.add_dependency(PathBuf::from("material.mat"), PathBuf::from("texture.png"));
        g.add_dependency(PathBuf::from("scene.toml"), PathBuf::from("material.mat"));
        g.remove_asset(&PathBuf::from("material.mat"));
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn remove_nonexistent_dependency() {
        let mut g = DependencyGraph::new();
        // Should not panic
        g.remove_dependency(&PathBuf::from("nope.mat"), &PathBuf::from("nope.png"));
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn remove_nonexistent_asset() {
        let mut g = DependencyGraph::new();
        g.remove_asset(&PathBuf::from("nope.mat"));
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn diamond_dependency_graph() {
        let mut g = DependencyGraph::new();
        // A depends on B and C; B depends on D; C depends on D
        g.add_dependency(PathBuf::from("A"), PathBuf::from("B"));
        g.add_dependency(PathBuf::from("A"), PathBuf::from("C"));
        g.add_dependency(PathBuf::from("B"), PathBuf::from("D"));
        g.add_dependency(PathBuf::from("C"), PathBuf::from("D"));

        // Changing D should cascade to B, C, and A
        let dependents = g.get_dependents(&PathBuf::from("D"));
        assert_eq!(dependents.len(), 3);
        assert!(dependents.contains(&PathBuf::from("A")));
        assert!(dependents.contains(&PathBuf::from("B")));
        assert!(dependents.contains(&PathBuf::from("C")));
    }

    #[test]
    fn get_dependencies_of_unknown_asset() {
        let g = DependencyGraph::new();
        let deps = g.get_dependencies(&PathBuf::from("unknown"));
        assert!(deps.is_empty());
    }

    #[test]
    fn asset_with_multiple_dependencies() {
        let mut g = DependencyGraph::new();
        g.add_dependency(PathBuf::from("material.mat"), PathBuf::from("diffuse.png"));
        g.add_dependency(PathBuf::from("material.mat"), PathBuf::from("normal.png"));
        g.add_dependency(
            PathBuf::from("material.mat"),
            PathBuf::from("roughness.png"),
        );
        let deps = g.get_dependencies(&PathBuf::from("material.mat"));
        assert_eq!(deps.len(), 3);
    }

    #[test]
    fn duplicate_dependency_is_idempotent() {
        let mut g = DependencyGraph::new();
        g.add_dependency(PathBuf::from("A"), PathBuf::from("B"));
        g.add_dependency(PathBuf::from("A"), PathBuf::from("B"));
        assert_eq!(g.edge_count(), 1);
    }

    #[test]
    fn edge_count_tracks_correctly() {
        let mut g = DependencyGraph::new();
        g.add_dependency(PathBuf::from("A"), PathBuf::from("B"));
        g.add_dependency(PathBuf::from("A"), PathBuf::from("C"));
        g.add_dependency(PathBuf::from("B"), PathBuf::from("C"));
        assert_eq!(g.edge_count(), 3);
    }

    #[test]
    fn asset_count_tracks_unique_nodes() {
        let mut g = DependencyGraph::new();
        g.add_dependency(PathBuf::from("A"), PathBuf::from("B"));
        g.add_dependency(PathBuf::from("B"), PathBuf::from("C"));
        assert_eq!(g.asset_count(), 3);
    }
}
