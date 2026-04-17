//! Structured telemetry contract for simulation reports.
//!
//! The [`Telemetry`] struct is the sole output channel for data emitted
//! during a scenario run. It is intentionally forward-compatible: new
//! event kinds and counters can be added without breaking consumers that
//! pattern-match on strings.
//!
//! # Stability contract
//!
//! * `Event.tick` is monotonically non-decreasing across a single report.
//! * `Event.kind` uses a dotted namespace (`vr.comfort.violation`,
//!   `mmo.coherence.double_spawn`, etc.). Kinds never disappear — only
//!   new kinds are added.
//! * `Event.data` is always a JSON object (never a bare scalar) so
//!   additional fields can be appended.
//! * `counters` keys use the same dotted namespace. Counters are
//!   cumulative within a run.
//! * `timings` records named phase durations (sim loop, scorer, etc.).

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

// NOTE: we use `BTreeMap` (not `HashMap`) for `timings` and `counters`
// so that serialization order is deterministic — a hard requirement for
// the harness's byte-identical determinism contract. The API shape
// remains map-like; downstream consumers treat this as an ordered map.

/// One observable event emitted during a scenario run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub tick: u64,
    pub kind: String,
    pub data: serde_json::Value,
}

impl Event {
    pub fn new(tick: u64, kind: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            tick,
            kind: kind.into(),
            data,
        }
    }

    /// Create an event with an empty data object.
    pub fn bare(tick: u64, kind: impl Into<String>) -> Self {
        Self::new(tick, kind, serde_json::json!({}))
    }
}

/// Serializable wrapper around [`Duration`] that preserves nanos and keeps
/// JSON output human-readable and deterministic.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DurationNs {
    pub nanos: u128,
}

impl DurationNs {
    pub fn from_duration(d: Duration) -> Self {
        Self { nanos: d.as_nanos() }
    }

    pub fn to_duration(&self) -> Duration {
        // Cap at u64::MAX seconds; 1<<64 nanos is ~584 years — plenty.
        Duration::from_nanos(self.nanos.min(u64::MAX as u128) as u64)
    }
}

/// Structured telemetry collected during one scenario run.
///
/// Events are ordered by `tick` then by insertion order (stable).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Telemetry {
    pub events: Vec<Event>,
    pub timings: BTreeMap<String, DurationNs>,
    pub counters: BTreeMap<String, u64>,
}

impl Telemetry {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            timings: BTreeMap::new(),
            counters: BTreeMap::new(),
        }
    }

    pub fn emit(&mut self, event: Event) {
        self.events.push(event);
    }

    pub fn record_timing(&mut self, key: impl Into<String>, d: Duration) {
        self.timings
            .insert(key.into(), DurationNs::from_duration(d));
    }

    pub fn incr(&mut self, key: impl Into<String>, by: u64) {
        let k = key.into();
        *self.counters.entry(k).or_insert(0) += by;
    }

    pub fn counter(&self, key: &str) -> u64 {
        self.counters.get(key).copied().unwrap_or(0)
    }

    pub fn events_with_kind<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Event> + 'a {
        self.events.iter().filter(move |e| e.kind == kind)
    }
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_and_find() {
        let mut t = Telemetry::new();
        t.emit(Event::bare(0, "tick.begin"));
        t.emit(Event::new(1, "spawn", serde_json::json!({"entity": 7})));
        assert_eq!(t.events.len(), 2);
        assert_eq!(t.events_with_kind("spawn").count(), 1);
    }

    #[test]
    fn counters_accumulate() {
        let mut t = Telemetry::new();
        t.incr("collisions", 1);
        t.incr("collisions", 2);
        assert_eq!(t.counter("collisions"), 3);
        assert_eq!(t.counter("absent"), 0);
    }

    #[test]
    fn timings_record() {
        let mut t = Telemetry::new();
        t.record_timing("scorer", Duration::from_micros(500));
        let v = t.timings.get("scorer").unwrap();
        assert_eq!(v.nanos, 500_000);
        assert_eq!(v.to_duration(), Duration::from_micros(500));
    }

    #[test]
    fn serde_roundtrip_preserves_telemetry() {
        let mut t = Telemetry::new();
        t.emit(Event::bare(3, "tick.end"));
        t.incr("foo", 9);
        t.record_timing("phase", Duration::from_nanos(1234));
        let s = serde_json::to_string(&t).unwrap();
        let back: Telemetry = serde_json::from_str(&s).unwrap();
        assert_eq!(t, back);
    }
}
