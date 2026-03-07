use aether_ecs::Entity;

/// A physics trigger event (enter or exit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEventKind {
    Enter,
    Exit,
}

/// A trigger event recording which entity entered/exited which trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerEvent {
    pub trigger_entity: Entity,
    pub other_entity: Entity,
    pub kind: TriggerEventKind,
}

/// Collects trigger events during a physics step.
#[derive(Debug, Default)]
pub struct TriggerEventQueue {
    events: Vec<TriggerEvent>,
    /// Tracks which entity pairs are currently overlapping.
    active_pairs: Vec<(Entity, Entity)>,
}

impl TriggerEventQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that two entities are currently intersecting.
    /// Emits an Enter event if they were not previously overlapping.
    pub fn report_intersection(&mut self, trigger_entity: Entity, other_entity: Entity) {
        let pair = normalize_pair(trigger_entity, other_entity);
        if !self.active_pairs.contains(&pair) {
            self.active_pairs.push(pair);
            self.events.push(TriggerEvent {
                trigger_entity,
                other_entity,
                kind: TriggerEventKind::Enter,
            });
        }
    }

    /// Called at the end of a physics step to detect exits.
    /// `current_intersections` is the set of pairs still overlapping.
    pub fn detect_exits(&mut self, current_intersections: &[(Entity, Entity)]) {
        let normalized: Vec<(Entity, Entity)> = current_intersections
            .iter()
            .map(|&(a, b)| normalize_pair(a, b))
            .collect();

        let mut exits = Vec::new();
        self.active_pairs.retain(|pair| {
            if normalized.contains(pair) {
                true
            } else {
                exits.push(*pair);
                false
            }
        });

        for (trigger_entity, other_entity) in exits {
            self.events.push(TriggerEvent {
                trigger_entity,
                other_entity,
                kind: TriggerEventKind::Exit,
            });
        }
    }

    pub fn drain_events(&mut self) -> Vec<TriggerEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn events(&self) -> &[TriggerEvent] {
        &self.events
    }

    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    pub fn active_pair_count(&self) -> usize {
        self.active_pairs.len()
    }
}

fn normalize_pair(a: Entity, b: Entity) -> (Entity, Entity) {
    if a.index() <= b.index() {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        // Use the Entity struct's internal constructor pattern
        // Entity { index, generation: 0 } but Entity fields are pub(crate)
        // We'll use a test helper
        unsafe { std::mem::transmute::<(u32, u32), Entity>((index, 0)) }
    }

    #[test]
    fn enter_event_on_first_intersection() {
        let mut queue = TriggerEventQueue::new();
        let trigger = entity(0);
        let other = entity(1);

        queue.report_intersection(trigger, other);

        let events = queue.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, TriggerEventKind::Enter);
        assert_eq!(events[0].trigger_entity, trigger);
        assert_eq!(events[0].other_entity, other);
    }

    #[test]
    fn no_duplicate_enter_events() {
        let mut queue = TriggerEventQueue::new();
        let trigger = entity(0);
        let other = entity(1);

        queue.report_intersection(trigger, other);
        queue.report_intersection(trigger, other);
        queue.report_intersection(trigger, other);

        assert_eq!(queue.events().len(), 1);
    }

    #[test]
    fn exit_event_when_pair_disappears() {
        let mut queue = TriggerEventQueue::new();
        let trigger = entity(0);
        let other = entity(1);

        queue.report_intersection(trigger, other);
        queue.clear_events();

        // Next frame: no intersections
        queue.detect_exits(&[]);

        let events = queue.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, TriggerEventKind::Exit);
    }

    #[test]
    fn no_exit_while_still_intersecting() {
        let mut queue = TriggerEventQueue::new();
        let trigger = entity(0);
        let other = entity(1);

        queue.report_intersection(trigger, other);
        queue.clear_events();

        queue.detect_exits(&[(trigger, other)]);
        assert!(queue.events().is_empty());
        assert_eq!(queue.active_pair_count(), 1);
    }

    #[test]
    fn multiple_triggers() {
        let mut queue = TriggerEventQueue::new();
        let t1 = entity(0);
        let t2 = entity(1);
        let player = entity(2);

        queue.report_intersection(t1, player);
        queue.report_intersection(t2, player);

        assert_eq!(queue.events().len(), 2);
        assert_eq!(queue.active_pair_count(), 2);
    }

    #[test]
    fn drain_clears_events() {
        let mut queue = TriggerEventQueue::new();
        queue.report_intersection(entity(0), entity(1));

        let events = queue.drain_events();
        assert_eq!(events.len(), 1);
        assert!(queue.events().is_empty());
        // But active pairs remain
        assert_eq!(queue.active_pair_count(), 1);
    }

    #[test]
    fn enter_exit_enter_cycle() {
        let mut queue = TriggerEventQueue::new();
        let trigger = entity(0);
        let other = entity(1);

        // Enter
        queue.report_intersection(trigger, other);
        assert_eq!(queue.drain_events().len(), 1);

        // Exit
        queue.detect_exits(&[]);
        let exits = queue.drain_events();
        assert_eq!(exits.len(), 1);
        assert_eq!(exits[0].kind, TriggerEventKind::Exit);

        // Re-enter
        queue.report_intersection(trigger, other);
        let enters = queue.drain_events();
        assert_eq!(enters.len(), 1);
        assert_eq!(enters[0].kind, TriggerEventKind::Enter);
    }
}
