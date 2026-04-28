// Placeholder trait for the per-frame handle. The full surface (predicted display
// time, view location, layer builder, RAII begin/end semantics) lands in P2-A/P2-B
// per design doc §5.4. P2-C (this unit) only needs the marker trait so that
// `XrAction::current(&impl XrFrame)` has something to bind to.
// TODO(P2-A/P2-B): replace with the full XrFrame trait per §5.4.

pub trait XrFrame {}
