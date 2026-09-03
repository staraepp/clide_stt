//! The set of transcription backends this build knows about.

use std::sync::Arc;

use super::groq::GroqProvider;
use super::traits::{ProviderDescriptor, TranscriptionProvider};

pub struct ProviderRegistry {
    providers: Vec<Arc<dyn TranscriptionProvider>>,
}

impl ProviderRegistry {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            // v0.1 ships one cloud provider. Apple Speech and local Whisper
            // join this list without the pipeline changing.
            providers: vec![Arc::new(GroqProvider::new(http))],
        }
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn TranscriptionProvider>> {
        self.providers.iter().find(|p| p.id() == id).cloned()
    }

    /// The provider used when nothing has been chosen yet.
    pub fn default_provider(&self) -> Arc<dyn TranscriptionProvider> {
        Arc::clone(&self.providers[0])
    }

    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        self.providers
            .iter()
            .map(|p| ProviderDescriptor::of(p.as_ref()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_provider_defaults_to_a_model_it_offers() {
        let registry = ProviderRegistry::new(reqwest::Client::new());
        for descriptor in registry.descriptors() {
            assert!(
                descriptor
                    .models
                    .iter()
                    .any(|m| m.id == descriptor.default_model),
                "{} defaults to a model it does not list",
                descriptor.id
            );
            assert!(!descriptor.models.is_empty());
        }
    }

    #[test]
    fn providers_are_looked_up_by_id_not_by_position() {
        let registry = ProviderRegistry::new(reqwest::Client::new());
        assert!(registry.get("groq").is_some());
        assert!(registry.get("not-a-provider").is_none());
    }
}
