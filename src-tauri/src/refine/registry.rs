//! The refinement backends this build knows about.

use std::sync::Arc;

use super::apple_intelligence::AppleIntelligenceRefiner;
use super::traits::{Refiner, RefinerDescriptor};

pub struct RefinerRegistry {
    refiners: Vec<Arc<dyn Refiner>>,
}

impl RefinerRegistry {
    pub fn new() -> Self {
        Self {
            // Apple Intelligence is the only backend today. A bundled small
            // model would join this list without the pipeline changing.
            refiners: vec![Arc::new(AppleIntelligenceRefiner::new())],
        }
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn Refiner>> {
        self.refiners.iter().find(|r| r.id() == id).cloned()
    }

    /// The first backend that can actually run right now.
    ///
    /// Used when the user has asked for refinement without naming an engine.
    pub fn first_available(&self) -> Option<Arc<dyn Refiner>> {
        self.refiners
            .iter()
            .find(|refiner| refiner.availability().is_ok())
            .cloned()
    }

    pub fn descriptors(&self) -> Vec<RefinerDescriptor> {
        self.refiners.iter().map(|r| r.descriptor()).collect()
    }
}

impl Default for RefinerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refiner_has_a_unique_id() {
        let registry = RefinerRegistry::new();
        let mut ids: Vec<_> = registry.descriptors().into_iter().map(|d| d.id).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(count, ids.len());
    }

    #[test]
    fn a_refiner_can_be_looked_up_by_id() {
        let registry = RefinerRegistry::new();
        assert!(registry.get("apple-intelligence").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    /// On a Mac without Apple Intelligence there is simply no refiner, and the
    /// pipeline must treat that as "use the transcript as spoken".
    #[test]
    fn no_available_refiner_is_a_valid_answer() {
        let registry = RefinerRegistry::new();
        // Either outcome is correct depending on the machine; what matters is
        // that asking does not panic.
        let _ = registry.first_available();
    }
}
