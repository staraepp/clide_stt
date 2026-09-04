//! The refinement backends this build knows about.

use std::sync::Arc;

use super::apple_intelligence::AppleIntelligenceRefiner;
use super::cloud::CloudRefiner;
use super::formatting::FormattingRefiner;
use super::traits::{Refiner, RefinerDescriptor};
use crate::credentials::Credentials;

pub struct RefinerRegistry {
    refiners: Vec<Arc<dyn Refiner>>,
}

impl RefinerRegistry {
    pub fn new(http: reqwest::Client, credentials: Credentials) -> Self {
        Self {
            // Apple Intelligence first: it is the only one that never sends
            // the transcript anywhere, so it is the one to reach for by
            // default. The cloud engines are off until switched on.
            refiners: vec![
                // Deterministic and instant, so it runs before anything that
                // has to load a model or reach a network.
                Arc::new(FormattingRefiner::new()),
                Arc::new(AppleIntelligenceRefiner::new()),
                Arc::new(CloudRefiner::groq(http.clone(), credentials.clone())),
                Arc::new(CloudRefiner::openai(http, credentials)),
            ],
        }
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn Refiner>> {
        self.refiners.iter().find(|r| r.id() == id).cloned()
    }

    /// The first backend that is both switched on and able to run.
    ///
    /// `enabled` is the user's explicit list. A refiner absent from it is
    /// never used, however available it happens to be — which is what makes
    /// "the transcript leaves your Mac" a decision rather than a side effect.
    pub fn first_enabled(&self, enabled: &[String]) -> Option<Arc<dyn Refiner>> {
        self.refiners
            .iter()
            .find(|refiner| {
                enabled.iter().any(|id| id == refiner.id()) && refiner.availability().is_ok()
            })
            .cloned()
    }

    pub fn descriptors(&self) -> Vec<RefinerDescriptor> {
        self.refiners.iter().map(|r| r.descriptor()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> RefinerRegistry {
        let dir = std::env::temp_dir().join("clide-refiner-registry");
        std::fs::create_dir_all(&dir).unwrap();
        RefinerRegistry::new(reqwest::Client::new(), Credentials::new(&dir))
    }

    #[test]
    fn every_refiner_has_a_unique_id() {
        let registry = registry();
        let mut ids: Vec<_> = registry.descriptors().into_iter().map(|d| d.id).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(count, ids.len());
    }

    #[test]
    fn a_refiner_can_be_looked_up_by_id() {
        let registry = registry();
        assert!(registry.get("apple-intelligence").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    /// The privacy rule, as a test: an engine the user has not switched on is
    /// never used, no matter how available it is.
    #[test]
    fn a_refiner_that_is_not_enabled_is_never_chosen() {
        let registry = registry();
        assert!(
            registry.first_enabled(&[]).is_none(),
            "a refiner ran with nothing enabled"
        );

        // Naming only one engine must never reach for a different one.
        let only_cloud = vec!["groq-rewrite".to_string()];
        if let Some(chosen) = registry.first_enabled(&only_cloud) {
            assert_eq!(chosen.id(), "groq-rewrite");
        }
    }

    /// Local refinement is preferred when several are enabled, because it is
    /// the only one that does not send the transcript anywhere.
    #[test]
    fn the_local_engine_is_preferred_over_cloud_ones() {
        let registry = registry();
        let ids: Vec<String> = registry.descriptors().into_iter().map(|d| d.id).collect();

        let local_at = ids.iter().position(|id| id == "apple-intelligence");
        let cloud_at = ids.iter().position(|id| id == "groq-rewrite");

        if let (Some(local), Some(cloud)) = (local_at, cloud_at) {
            assert!(local < cloud, "a cloud refiner outranked the local one");
        }
    }
}
