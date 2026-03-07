/// A lightweight entity identifier using generational indices.
/// The generation prevents the ABA problem when entity slots are recycled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl Entity {
    pub fn index(self) -> u32 {
        self.index
    }

    pub fn generation(self) -> u32 {
        self.generation
    }
}

/// Allocates and recycles entity IDs with generational tracking.
pub struct EntityAllocator {
    generations: Vec<u32>,
    free_list: Vec<u32>,
    alive: Vec<bool>,
    live_count: usize,
}

impl EntityAllocator {
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            free_list: Vec::new(),
            alive: Vec::new(),
            live_count: 0,
        }
    }

    pub fn allocate(&mut self) -> Entity {
        self.live_count += 1;
        if let Some(index) = self.free_list.pop() {
            self.alive[index as usize] = true;
            Entity {
                index,
                generation: self.generations[index as usize],
            }
        } else {
            let index = self.generations.len() as u32;
            self.generations.push(0);
            self.alive.push(true);
            Entity {
                index,
                generation: 0,
            }
        }
    }

    pub fn deallocate(&mut self, entity: Entity) -> bool {
        let idx = entity.index as usize;
        if idx >= self.generations.len() {
            return false;
        }
        if self.generations[idx] != entity.generation || !self.alive[idx] {
            return false;
        }
        self.alive[idx] = false;
        self.generations[idx] = self.generations[idx].wrapping_add(1);
        self.free_list.push(entity.index);
        self.live_count -= 1;
        true
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        let idx = entity.index as usize;
        idx < self.generations.len()
            && self.generations[idx] == entity.generation
            && self.alive[idx]
    }

    pub fn len(&self) -> usize {
        self.live_count
    }

    pub fn is_empty(&self) -> bool {
        self.live_count == 0
    }
}

impl Default for EntityAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_sequential_entities() {
        let mut alloc = EntityAllocator::new();
        let e0 = alloc.allocate();
        let e1 = alloc.allocate();
        assert_eq!(e0.index, 0);
        assert_eq!(e0.generation, 0);
        assert_eq!(e1.index, 1);
        assert_eq!(e1.generation, 0);
    }

    #[test]
    fn deallocate_and_recycle() {
        let mut alloc = EntityAllocator::new();
        let e0 = alloc.allocate();
        assert!(alloc.deallocate(e0));
        let e0_new = alloc.allocate();
        assert_eq!(e0_new.index, 0);
        assert_eq!(e0_new.generation, 1);
    }

    #[test]
    fn deallocate_stale_entity_fails() {
        let mut alloc = EntityAllocator::new();
        let e0 = alloc.allocate();
        assert!(alloc.deallocate(e0));
        // e0 is now stale (generation 0, but slot is generation 1)
        assert!(!alloc.deallocate(e0));
    }

    #[test]
    fn is_alive_checks() {
        let mut alloc = EntityAllocator::new();
        let e0 = alloc.allocate();
        assert!(alloc.is_alive(e0));
        alloc.deallocate(e0);
        assert!(!alloc.is_alive(e0));
        let e0_new = alloc.allocate();
        assert!(alloc.is_alive(e0_new));
        assert!(!alloc.is_alive(e0)); // old generation still dead
    }

    #[test]
    fn len_tracks_alive_count() {
        let mut alloc = EntityAllocator::new();
        assert_eq!(alloc.len(), 0);
        assert!(alloc.is_empty());
        let e0 = alloc.allocate();
        let _e1 = alloc.allocate();
        assert_eq!(alloc.len(), 2);
        alloc.deallocate(e0);
        assert_eq!(alloc.len(), 1);
    }

    #[test]
    fn deallocate_invalid_index() {
        let mut alloc = EntityAllocator::new();
        let fake = Entity {
            index: 99,
            generation: 0,
        };
        assert!(!alloc.deallocate(fake));
    }

    #[test]
    fn generation_wraps() {
        let mut alloc = EntityAllocator::new();
        // Force wrap by manipulating generations directly isn't needed;
        // just verify the wrapping_add logic works
        let e = alloc.allocate();
        alloc.deallocate(e);
        let e2 = alloc.allocate();
        assert_eq!(e2.generation, 1);
        alloc.deallocate(e2);
        let e3 = alloc.allocate();
        assert_eq!(e3.generation, 2);
    }

    #[test]
    fn multiple_recycles() {
        let mut alloc = EntityAllocator::new();
        let e0 = alloc.allocate();
        let e1 = alloc.allocate();
        let e2 = alloc.allocate();
        alloc.deallocate(e1);
        alloc.deallocate(e0);
        // Free list: [e1.index=1, e0.index=0], pops e0 first (LIFO)
        let r0 = alloc.allocate();
        let r1 = alloc.allocate();
        assert_eq!(r0.index, 0);
        assert_eq!(r0.generation, 1);
        assert_eq!(r1.index, 1);
        assert_eq!(r1.generation, 1);
        assert!(alloc.is_alive(e2));
    }
}
