//! Health probe configuration for Kubernetes liveness and readiness checks.
//!
//! Generates HTTP-GET probe specs that distinguish between
//! "process alive" (liveness) and "ready to accept players" (readiness).

use serde::Serialize;

/// Default liveness probe endpoint.
const DEFAULT_LIVENESS_PATH: &str = "/healthz";

/// Default readiness probe endpoint.
const DEFAULT_READINESS_PATH: &str = "/ready";

/// Default probe port.
const DEFAULT_PROBE_PORT: u16 = 8080;

/// Default initial delay before probes start (seconds).
const DEFAULT_INITIAL_DELAY_SECS: u32 = 15;

/// Default probe period (seconds).
const DEFAULT_PERIOD_SECS: u32 = 10;

/// Default failure threshold before marking unhealthy.
const DEFAULT_FAILURE_THRESHOLD: u32 = 3;

/// Configuration for liveness and readiness probes.
#[derive(Debug, Clone, Serialize)]
pub struct ProbeConfig {
    pub liveness_path: String,
    pub readiness_path: String,
    pub port: u16,
    pub initial_delay_secs: u32,
    pub period_secs: u32,
    pub failure_threshold: u32,
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self {
            liveness_path: DEFAULT_LIVENESS_PATH.to_string(),
            readiness_path: DEFAULT_READINESS_PATH.to_string(),
            port: DEFAULT_PROBE_PORT,
            initial_delay_secs: DEFAULT_INITIAL_DELAY_SECS,
            period_secs: DEFAULT_PERIOD_SECS,
            failure_threshold: DEFAULT_FAILURE_THRESHOLD,
        }
    }
}

/// A single probe specification.
#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    pub path: String,
    pub port: u16,
    pub initial_delay_secs: u32,
    pub period_secs: u32,
    pub failure_threshold: u32,
}

impl ProbeConfig {
    /// Returns the liveness probe specification.
    pub fn liveness_probe(&self) -> Probe {
        Probe {
            path: self.liveness_path.clone(),
            port: self.port,
            initial_delay_secs: self.initial_delay_secs,
            period_secs: self.period_secs,
            failure_threshold: self.failure_threshold,
        }
    }

    /// Returns the readiness probe specification.
    pub fn readiness_probe(&self) -> Probe {
        Probe {
            path: self.readiness_path.clone(),
            port: self.port,
            initial_delay_secs: self.initial_delay_secs,
            period_secs: self.period_secs,
            failure_threshold: self.failure_threshold,
        }
    }

    /// Generates the liveness probe as a serde_yaml::Value for manifest embedding.
    pub fn liveness_probe_value(&self) -> serde_yaml::Value {
        self.probe_to_value(&self.liveness_path)
    }

    /// Generates the readiness probe as a serde_yaml::Value for manifest embedding.
    pub fn readiness_probe_value(&self) -> serde_yaml::Value {
        self.probe_to_value(&self.readiness_path)
    }

    fn probe_to_value(&self, path: &str) -> serde_yaml::Value {
        use serde_yaml::Value;

        Value::Mapping({
            let mut probe = serde_yaml::Mapping::new();
            probe.insert(
                Value::String("httpGet".to_string()),
                Value::Mapping({
                    let mut http = serde_yaml::Mapping::new();
                    http.insert(
                        Value::String("path".to_string()),
                        Value::String(path.to_string()),
                    );
                    http.insert(
                        Value::String("port".to_string()),
                        Value::Number(self.port.into()),
                    );
                    http
                }),
            );
            probe.insert(
                Value::String("initialDelaySeconds".to_string()),
                Value::Number(self.initial_delay_secs.into()),
            );
            probe.insert(
                Value::String("periodSeconds".to_string()),
                Value::Number(self.period_secs.into()),
            );
            probe.insert(
                Value::String("failureThreshold".to_string()),
                Value::Number(self.failure_threshold.into()),
            );
            probe
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_liveness_path() {
        let cfg = ProbeConfig::default();
        assert_eq!(cfg.liveness_path, "/healthz");
    }

    #[test]
    fn default_readiness_path() {
        let cfg = ProbeConfig::default();
        assert_eq!(cfg.readiness_path, "/ready");
    }

    #[test]
    fn liveness_probe_has_correct_path() {
        let cfg = ProbeConfig {
            liveness_path: "/health".to_string(),
            ..ProbeConfig::default()
        };
        let probe = cfg.liveness_probe();
        assert_eq!(probe.path, "/health");
    }

    #[test]
    fn readiness_probe_has_correct_path() {
        let cfg = ProbeConfig {
            readiness_path: "/readyz".to_string(),
            ..ProbeConfig::default()
        };
        let probe = cfg.readiness_probe();
        assert_eq!(probe.path, "/readyz");
    }

    #[test]
    fn probes_share_port() {
        let cfg = ProbeConfig {
            port: 9090,
            ..ProbeConfig::default()
        };
        assert_eq!(cfg.liveness_probe().port, 9090);
        assert_eq!(cfg.readiness_probe().port, 9090);
    }

    #[test]
    fn probes_share_initial_delay() {
        let cfg = ProbeConfig {
            initial_delay_secs: 30,
            ..ProbeConfig::default()
        };
        assert_eq!(cfg.liveness_probe().initial_delay_secs, 30);
        assert_eq!(cfg.readiness_probe().initial_delay_secs, 30);
    }

    #[test]
    fn probes_share_period() {
        let cfg = ProbeConfig {
            period_secs: 20,
            ..ProbeConfig::default()
        };
        assert_eq!(cfg.liveness_probe().period_secs, 20);
        assert_eq!(cfg.readiness_probe().period_secs, 20);
    }

    #[test]
    fn probes_share_failure_threshold() {
        let cfg = ProbeConfig {
            failure_threshold: 5,
            ..ProbeConfig::default()
        };
        assert_eq!(cfg.liveness_probe().failure_threshold, 5);
        assert_eq!(cfg.readiness_probe().failure_threshold, 5);
    }

    #[test]
    fn liveness_and_readiness_probes_differ_only_in_path() {
        let cfg = ProbeConfig::default();
        let l = cfg.liveness_probe();
        let r = cfg.readiness_probe();
        assert_ne!(l.path, r.path);
        assert_eq!(l.port, r.port);
        assert_eq!(l.initial_delay_secs, r.initial_delay_secs);
        assert_eq!(l.period_secs, r.period_secs);
        assert_eq!(l.failure_threshold, r.failure_threshold);
    }

    #[test]
    fn liveness_probe_value_contains_http_get() {
        let cfg = ProbeConfig::default();
        let yaml = serde_yaml::to_string(&cfg.liveness_probe_value()).unwrap();
        assert!(yaml.contains("httpGet"));
    }

    #[test]
    fn liveness_probe_value_contains_path() {
        let cfg = ProbeConfig::default();
        let yaml = serde_yaml::to_string(&cfg.liveness_probe_value()).unwrap();
        assert!(yaml.contains("/healthz"));
    }

    #[test]
    fn readiness_probe_value_contains_path() {
        let cfg = ProbeConfig::default();
        let yaml = serde_yaml::to_string(&cfg.readiness_probe_value()).unwrap();
        assert!(yaml.contains("/ready"));
    }

    #[test]
    fn probe_value_contains_initial_delay() {
        let cfg = ProbeConfig {
            initial_delay_secs: 42,
            ..ProbeConfig::default()
        };
        let yaml = serde_yaml::to_string(&cfg.liveness_probe_value()).unwrap();
        assert!(yaml.contains("initialDelaySeconds: 42"));
    }

    #[test]
    fn probe_value_contains_period() {
        let cfg = ProbeConfig {
            period_secs: 7,
            ..ProbeConfig::default()
        };
        let yaml = serde_yaml::to_string(&cfg.liveness_probe_value()).unwrap();
        assert!(yaml.contains("periodSeconds: 7"));
    }

    #[test]
    fn probe_value_contains_failure_threshold() {
        let cfg = ProbeConfig {
            failure_threshold: 10,
            ..ProbeConfig::default()
        };
        let yaml = serde_yaml::to_string(&cfg.liveness_probe_value()).unwrap();
        assert!(yaml.contains("failureThreshold: 10"));
    }

    #[test]
    fn probe_value_contains_port() {
        let cfg = ProbeConfig {
            port: 3000,
            ..ProbeConfig::default()
        };
        let yaml = serde_yaml::to_string(&cfg.liveness_probe_value()).unwrap();
        assert!(yaml.contains("port: 3000"));
    }

    #[test]
    fn custom_probe_config_roundtrip() {
        let cfg = ProbeConfig {
            liveness_path: "/alive".to_string(),
            readiness_path: "/ok".to_string(),
            port: 4000,
            initial_delay_secs: 5,
            period_secs: 3,
            failure_threshold: 2,
        };
        let l = cfg.liveness_probe();
        let r = cfg.readiness_probe();
        assert_eq!(l.path, "/alive");
        assert_eq!(r.path, "/ok");
        assert_eq!(l.port, 4000);
        assert_eq!(r.port, 4000);
    }
}
