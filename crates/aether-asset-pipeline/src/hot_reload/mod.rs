//! Asset hot-reloading system.
//!
//! Watches asset directories for file changes, debounces rapid modifications,
//! tracks dependencies, and emits reload events via a channel.

pub mod asset_type;
pub mod debouncer;
pub mod dependency;
pub mod events;

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use log::{error, info, warn};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use asset_type::AssetType;
use debouncer::Debouncer;
use dependency::DependencyGraph;
use events::{ChangeKind, ReloadEvent};

/// Default debounce window in milliseconds.
const DEFAULT_DEBOUNCE_MS: u64 = 300;

/// Default watch path.
const DEFAULT_WATCH_PATH: &str = "./assets";

/// Default ignore patterns (comma-separated).
const DEFAULT_IGNORE_PATTERNS: &str = "*.tmp,*.swp,*~";

/// Polling interval for the debounce drain loop in milliseconds.
const DEBOUNCE_POLL_INTERVAL_MS: u64 = 50;

/// Environment variable names.
const ENV_HOT_RELOAD_ENABLED: &str = "AETHER_HOT_RELOAD_ENABLED";
const ENV_HOT_RELOAD_DEBOUNCE_MS: &str = "AETHER_HOT_RELOAD_DEBOUNCE_MS";
const ENV_HOT_RELOAD_WATCH_PATHS: &str = "AETHER_HOT_RELOAD_WATCH_PATHS";
const ENV_HOT_RELOAD_IGNORE_PATTERNS: &str = "AETHER_HOT_RELOAD_IGNORE_PATTERNS";

/// Configuration for the hot-reload system.
#[derive(Debug, Clone)]
pub struct HotReloadConfig {
    /// Whether hot-reloading is enabled.
    pub enabled: bool,
    /// Debounce window in milliseconds.
    pub debounce_ms: u64,
    /// Directories to watch for changes.
    pub watch_paths: Vec<PathBuf>,
    /// Glob patterns for files to ignore.
    pub ignore_patterns: Vec<String>,
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            debounce_ms: DEFAULT_DEBOUNCE_MS,
            watch_paths: vec![PathBuf::from(DEFAULT_WATCH_PATH)],
            ignore_patterns: parse_comma_separated(DEFAULT_IGNORE_PATTERNS),
        }
    }
}

impl HotReloadConfig {
    /// Load configuration from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        let enabled = std::env::var(ENV_HOT_RELOAD_ENABLED)
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);

        let debounce_ms = std::env::var(ENV_HOT_RELOAD_DEBOUNCE_MS)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_DEBOUNCE_MS);

        let watch_paths = std::env::var(ENV_HOT_RELOAD_WATCH_PATHS)
            .map(|v| v.split(',').map(|s| PathBuf::from(s.trim())).collect())
            .unwrap_or_else(|_| vec![PathBuf::from(DEFAULT_WATCH_PATH)]);

        let ignore_patterns = std::env::var(ENV_HOT_RELOAD_IGNORE_PATTERNS)
            .map(|v| parse_comma_separated(&v))
            .unwrap_or_else(|_| parse_comma_separated(DEFAULT_IGNORE_PATTERNS));

        Self {
            enabled,
            debounce_ms,
            watch_paths,
            ignore_patterns,
        }
    }
}

/// Check if a file path matches any of the ignore patterns.
pub fn should_ignore(path: &Path, patterns: &[String]) -> bool {
    let file_name = match path.file_name() {
        Some(name) => name.to_string_lossy(),
        None => return false,
    };

    for pattern in patterns {
        if glob_match::glob_match(pattern, &file_name) {
            return true;
        }
    }
    false
}

/// Parse a comma-separated string into a vector of trimmed strings.
fn parse_comma_separated(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Convert a notify EventKind to our ChangeKind.
fn event_kind_to_change_kind(kind: &EventKind) -> Option<ChangeKind> {
    match kind {
        EventKind::Create(_) => Some(ChangeKind::Created),
        EventKind::Modify(_) => Some(ChangeKind::Modified),
        EventKind::Remove(_) => Some(ChangeKind::Deleted),
        _ => None,
    }
}

/// The hot-reload watcher. Watches directories and emits `ReloadEvent`s.
pub struct HotReloadWatcher {
    /// The underlying file system watcher (kept alive to maintain watches).
    _watcher: RecommendedWatcher,
    /// Channel receiver for reload events.
    event_rx: mpsc::Receiver<ReloadEvent>,
    /// Shared dependency graph.
    dependency_graph: Arc<Mutex<DependencyGraph>>,
    /// Handle to the background drain thread.
    _drain_handle: thread::JoinHandle<()>,
}

impl HotReloadWatcher {
    /// Start watching with the given configuration.
    ///
    /// Returns `None` if hot-reloading is disabled.
    /// Returns an error string if setup fails.
    pub fn start(config: HotReloadConfig) -> Result<Option<Self>, String> {
        if !config.enabled {
            info!(target: "hot_reload", "Hot-reloading is disabled");
            return Ok(None);
        }

        let (reload_tx, reload_rx) = mpsc::channel::<ReloadEvent>();
        let dependency_graph = Arc::new(Mutex::new(DependencyGraph::new()));

        // Channel for raw FS events from notify -> debounce thread
        let (raw_tx, raw_rx) = mpsc::channel::<(PathBuf, ChangeKind)>();

        let ignore_patterns = config.ignore_patterns.clone();

        // Create file watcher
        let watcher_ignore = ignore_patterns.clone();
        let raw_tx_clone = raw_tx.clone();
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    if let Some(change_kind) = event_kind_to_change_kind(&event.kind) {
                        for path in event.paths {
                            if !should_ignore(&path, &watcher_ignore) {
                                let _ = raw_tx_clone.send((path, change_kind.clone()));
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(target: "hot_reload", "File watcher error: {}", e);
                }
            }
        })
        .map_err(|e| format!("Failed to create file watcher: {}", e))?;

        // Start watching paths
        for watch_path in &config.watch_paths {
            if watch_path.exists() {
                watcher
                    .watch(watch_path, RecursiveMode::Recursive)
                    .map_err(|e| format!("Failed to watch {}: {}", watch_path.display(), e))?;
                info!(
                    target: "hot_reload",
                    "Watching directory: {}",
                    watch_path.display()
                );
            } else {
                warn!(
                    target: "hot_reload",
                    "Watch path does not exist: {}",
                    watch_path.display()
                );
            }
        }

        // Background thread: drain debounced events and emit ReloadEvents
        let drain_dep_graph = Arc::clone(&dependency_graph);
        let debounce_ms = config.debounce_ms;
        let drain_handle = thread::spawn(move || {
            let mut debouncer = Debouncer::new(debounce_ms);

            loop {
                // Collect raw events (non-blocking)
                while let Ok((path, kind)) = raw_rx.try_recv() {
                    debouncer.record(path, kind);
                }

                // Drain settled events
                let now = Instant::now();
                let settled = debouncer.drain_settled(now);

                for (path, change_kind) in settled {
                    let asset_type = AssetType::from_path(&path);
                    let dependents = {
                        let graph = drain_dep_graph.lock().unwrap_or_else(|e| e.into_inner());
                        graph.get_dependents(&path)
                    };

                    info!(
                        target: "hot_reload",
                        "Asset changed: path={}, type={}, kind={:?}, dependents={}",
                        path.display(),
                        asset_type,
                        change_kind,
                        dependents.len()
                    );

                    let event = ReloadEvent::new(
                        path,
                        asset_type,
                        change_kind,
                        dependents,
                    );

                    if reload_tx.send(event).is_err() {
                        // Receiver dropped, exit thread
                        return;
                    }
                }

                thread::sleep(Duration::from_millis(DEBOUNCE_POLL_INTERVAL_MS));
            }
        });

        info!(
            target: "hot_reload",
            "Hot-reload watcher started with debounce={}ms, paths={:?}",
            config.debounce_ms,
            config.watch_paths
        );

        Ok(Some(Self {
            _watcher: watcher,
            event_rx: reload_rx,
            dependency_graph,
            _drain_handle: drain_handle,
        }))
    }

    /// Try to receive the next reload event (non-blocking).
    pub fn try_recv(&self) -> Option<ReloadEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Receive the next reload event, blocking until one is available.
    pub fn recv(&self) -> Option<ReloadEvent> {
        self.event_rx.recv().ok()
    }

    /// Receive the next reload event with a timeout.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<ReloadEvent> {
        self.event_rx.recv_timeout(timeout).ok()
    }

    /// Register a dependency: `dependent` depends on `dependency`.
    pub fn add_dependency(&self, dependent: PathBuf, dependency: PathBuf) {
        let mut graph = self
            .dependency_graph
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        graph.add_dependency(dependent, dependency);
    }

    /// Remove a dependency edge.
    pub fn remove_dependency(&self, dependent: &PathBuf, dependency: &PathBuf) {
        let mut graph = self
            .dependency_graph
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        graph.remove_dependency(dependent, dependency);
    }

    /// Remove all dependencies involving the given asset.
    pub fn remove_asset(&self, asset: &PathBuf) {
        let mut graph = self
            .dependency_graph
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        graph.remove_asset(asset);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Config tests --

    #[test]
    fn default_config_values() {
        let config = HotReloadConfig::default();
        assert!(config.enabled);
        assert_eq!(config.debounce_ms, 300);
        assert_eq!(config.watch_paths, vec![PathBuf::from("./assets")]);
        assert_eq!(
            config.ignore_patterns,
            vec!["*.tmp".to_string(), "*.swp".to_string(), "*~".to_string()]
        );
    }

    #[test]
    fn config_clone() {
        let config = HotReloadConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.enabled, config.enabled);
        assert_eq!(cloned.debounce_ms, config.debounce_ms);
    }

    // -- Ignore pattern tests --

    #[test]
    fn ignore_tmp_files() {
        let patterns = vec!["*.tmp".to_string()];
        assert!(should_ignore(Path::new("file.tmp"), &patterns));
        assert!(!should_ignore(Path::new("file.png"), &patterns));
    }

    #[test]
    fn ignore_swp_files() {
        let patterns = vec!["*.swp".to_string()];
        assert!(should_ignore(Path::new(".file.swp"), &patterns));
    }

    #[test]
    fn ignore_tilde_backup_files() {
        let patterns = vec!["*~".to_string()];
        assert!(should_ignore(Path::new("file.txt~"), &patterns));
    }

    #[test]
    fn ignore_multiple_patterns() {
        let patterns = vec![
            "*.tmp".to_string(),
            "*.swp".to_string(),
            "*~".to_string(),
        ];
        assert!(should_ignore(Path::new("x.tmp"), &patterns));
        assert!(should_ignore(Path::new("x.swp"), &patterns));
        assert!(should_ignore(Path::new("x~"), &patterns));
        assert!(!should_ignore(Path::new("x.png"), &patterns));
    }

    #[test]
    fn ignore_no_patterns() {
        let patterns: Vec<String> = vec![];
        assert!(!should_ignore(Path::new("anything.tmp"), &patterns));
    }

    #[test]
    fn ignore_with_directory_path() {
        let patterns = vec!["*.tmp".to_string()];
        assert!(should_ignore(Path::new("/some/dir/file.tmp"), &patterns));
        assert!(!should_ignore(Path::new("/some/dir/file.png"), &patterns));
    }

    #[test]
    fn ignore_no_filename() {
        let patterns = vec!["*.tmp".to_string()];
        // Root path has no file_name
        assert!(!should_ignore(Path::new("/"), &patterns));
    }

    // -- parse_comma_separated tests --

    #[test]
    fn parse_comma_separated_basic() {
        let result = parse_comma_separated("a,b,c");
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_comma_separated_with_spaces() {
        let result = parse_comma_separated(" a , b , c ");
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_comma_separated_empty_string() {
        let result = parse_comma_separated("");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_comma_separated_single_item() {
        let result = parse_comma_separated("only");
        assert_eq!(result, vec!["only"]);
    }

    #[test]
    fn parse_comma_separated_trailing_comma() {
        let result = parse_comma_separated("a,b,");
        assert_eq!(result, vec!["a", "b"]);
    }

    // -- event_kind_to_change_kind tests --

    #[test]
    fn convert_create_event() {
        let kind = EventKind::Create(notify::event::CreateKind::File);
        assert_eq!(event_kind_to_change_kind(&kind), Some(ChangeKind::Created));
    }

    #[test]
    fn convert_modify_event() {
        let kind = EventKind::Modify(notify::event::ModifyKind::Data(
            notify::event::DataChange::Any,
        ));
        assert_eq!(
            event_kind_to_change_kind(&kind),
            Some(ChangeKind::Modified)
        );
    }

    #[test]
    fn convert_remove_event() {
        let kind = EventKind::Remove(notify::event::RemoveKind::File);
        assert_eq!(event_kind_to_change_kind(&kind), Some(ChangeKind::Deleted));
    }

    #[test]
    fn convert_access_event_is_none() {
        let kind = EventKind::Access(notify::event::AccessKind::Read);
        assert_eq!(event_kind_to_change_kind(&kind), None);
    }

    #[test]
    fn convert_other_event_is_none() {
        let kind = EventKind::Other;
        assert_eq!(event_kind_to_change_kind(&kind), None);
    }

    // -- Integration test with real file system --

    #[test]
    fn watcher_disabled_returns_none() {
        let config = HotReloadConfig {
            enabled: false,
            ..Default::default()
        };
        let result = HotReloadWatcher::start(config).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn watcher_detects_file_creation() {
        let dir = tempfile::tempdir().unwrap();
        let config = HotReloadConfig {
            enabled: true,
            debounce_ms: 50,
            watch_paths: vec![dir.path().to_path_buf()],
            ignore_patterns: vec!["*.tmp".to_string()],
        };

        let watcher = HotReloadWatcher::start(config).unwrap().unwrap();

        // Give the watcher time to start
        thread::sleep(Duration::from_millis(100));

        // Create a file
        let file_path = dir.path().join("test.png");
        std::fs::write(&file_path, b"test data").unwrap();

        // Wait for debounce + processing
        let event = watcher.recv_timeout(Duration::from_secs(2));
        assert!(event.is_some(), "Expected a reload event for file creation");
        let event = event.unwrap();
        assert_eq!(event.asset_type, AssetType::Texture);
    }

    #[test]
    fn watcher_ignores_tmp_files() {
        let dir = tempfile::tempdir().unwrap();
        let dir_canon = dir.path().canonicalize().unwrap();
        let config = HotReloadConfig {
            enabled: true,
            debounce_ms: 50,
            watch_paths: vec![dir_canon.clone()],
            ignore_patterns: vec!["*.tmp".to_string()],
        };

        let watcher = HotReloadWatcher::start(config).unwrap().unwrap();
        thread::sleep(Duration::from_millis(200));

        // Drain any directory-creation events
        while watcher.recv_timeout(Duration::from_millis(100)).is_some() {}

        // Create an ignored file
        std::fs::write(dir_canon.join("scratch.tmp"), b"ignored").unwrap();

        // Should not receive any event for the .tmp file
        // Wait long enough for debounce to expire
        let event = watcher.recv_timeout(Duration::from_millis(500));
        assert!(event.is_none(), "Should not receive events for ignored files");
    }

    #[test]
    fn watcher_dependency_tracking() {
        let dir = tempfile::tempdir().unwrap();
        // Canonicalize to handle macOS /var -> /private/var symlink
        let dir_canon = dir.path().canonicalize().unwrap();
        let texture_path = dir_canon.join("diffuse.png");
        let material_path = dir_canon.join("material.mat");

        // Create initial files
        std::fs::write(&texture_path, b"initial").unwrap();
        std::fs::write(&material_path, b"initial").unwrap();

        let config = HotReloadConfig {
            enabled: true,
            debounce_ms: 50,
            watch_paths: vec![dir_canon.clone()],
            ignore_patterns: vec![],
        };

        let watcher = HotReloadWatcher::start(config).unwrap().unwrap();
        thread::sleep(Duration::from_millis(200));

        // Drain any creation events first
        while watcher.recv_timeout(Duration::from_millis(200)).is_some() {}

        // Register dependency: material depends on texture
        watcher.add_dependency(material_path.clone(), texture_path.clone());

        // Modify the texture
        std::fs::write(&texture_path, b"updated texture data").unwrap();

        // Wait for the event
        let event = watcher.recv_timeout(Duration::from_secs(2));
        assert!(event.is_some(), "Expected reload event for texture change");
        let event = event.unwrap();
        assert_eq!(event.path, texture_path);
        assert!(
            event.affected_dependents.contains(&material_path),
            "Material should be listed as affected dependent"
        );
    }

    #[test]
    fn watcher_nonexistent_path_warns_but_succeeds() {
        let config = HotReloadConfig {
            enabled: true,
            debounce_ms: 50,
            watch_paths: vec![PathBuf::from("/nonexistent/path/12345")],
            ignore_patterns: vec![],
        };
        // Should succeed (just warn about missing path)
        let result = HotReloadWatcher::start(config);
        assert!(result.is_ok());
    }

    #[test]
    fn watcher_try_recv_returns_none_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let config = HotReloadConfig {
            enabled: true,
            debounce_ms: 50,
            watch_paths: vec![dir.path().to_path_buf()],
            ignore_patterns: vec![],
        };
        let watcher = HotReloadWatcher::start(config).unwrap().unwrap();
        assert!(watcher.try_recv().is_none());
    }
}
