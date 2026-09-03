//! The set of transcription backends this build knows about.

use std::sync::Arc;

use crate::models::ModelStore;

use super::apple::AppleSpeechProvider;
use super::assemblyai::AssemblyAiProvider;
use super::deepgram::DeepgramProvider;
use super::elevenlabs::ElevenLabsProvider;
use super::groq::GroqProvider;
use super::local::{LocalParakeetProvider, LocalWhisperProvider};
use super::openai::OpenAiProvider;
use super::traits::{ProviderDescriptor, TranscriptionProvider};

pub struct ProviderRegistry {
    providers: Vec<Arc<dyn TranscriptionProvider>>,
}

impl ProviderRegistry {
    pub fn new(http: reqwest::Client, models: ModelStore) -> Self {
        Self {
            // Order matters only for `default_provider`; everything else
            // looks providers up by id. Groq stays first because it is the
            // fastest of these for dictation.
            //
            // Apple Speech and local engines join this list without the
            // dictation pipeline changing — they differ by capability, not by
            // special-casing.
            providers: vec![
                Arc::new(GroqProvider::new(http.clone())),
                // Ships with macOS: usable on a fresh install with no key and
                // no download, which also makes it the safest fallback.
                Arc::new(AppleSpeechProvider::new()),
                Arc::new(OpenAiProvider::new(http.clone())),
                Arc::new(DeepgramProvider::new(http.clone())),
                Arc::new(ElevenLabsProvider::new(http.clone())),
                Arc::new(AssemblyAiProvider::new(http)),
                // Local runs last in the list but is not a lesser citizen: it
                // differs by capability, not by rank.
                Arc::new(LocalWhisperProvider::new(models.clone())),
                Arc::new(LocalParakeetProvider::new(models)),
            ],
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

    /// A provider that offers models must default to one of them.
    ///
    /// Local engines are exempt from *having* models: `models()` reports what
    /// is installed, and nothing is installed on a fresh machine. That is the
    /// correct answer, so the invariant is conditional on offering any.
    #[test]
    fn every_provider_that_offers_models_defaults_to_one_of_them() {
        let registry = ProviderRegistry::new(
            reqwest::Client::new(),
            ModelStore::new(&std::env::temp_dir()),
        );

        for descriptor in registry.descriptors() {
            if descriptor.models.is_empty() {
                assert!(
                    descriptor.capabilities.local,
                    "{} offers no models but is not a local engine",
                    descriptor.id
                );
                continue;
            }

            assert!(
                descriptor
                    .models
                    .iter()
                    .any(|m| m.id == descriptor.default_model),
                "{} defaults to a model it does not list",
                descriptor.id
            );
        }
    }

    /// Every cloud backend must be usable the moment a key is entered.
    #[test]
    fn every_cloud_provider_ships_a_model_catalogue() {
        let registry = ProviderRegistry::new(
            reqwest::Client::new(),
            ModelStore::new(&std::env::temp_dir()),
        );

        for descriptor in registry.descriptors() {
            if descriptor.capabilities.local {
                continue;
            }
            assert!(
                !descriptor.models.is_empty(),
                "{} is a cloud provider with no models",
                descriptor.id
            );
        }
    }

    #[test]
    fn providers_are_looked_up_by_id_not_by_position() {
        let registry = ProviderRegistry::new(reqwest::Client::new(), ModelStore::new(&std::env::temp_dir()));
        assert!(registry.get("groq").is_some());
        assert!(registry.get("not-a-provider").is_none());
    }
}
