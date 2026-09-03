//! Choosing a backup engine when the selected one cannot run.
//!
//! # Why this exists at all, given blueprint §12
//!
//! `blueprint.md` §12 is blunt: **no automatic cloud roulette.** Silently
//! moving a recording of someone's voice to a vendor they did not pick is a
//! privacy decision Clide has no business making on its own.
//!
//! The user asked for a fallback anyway, and they are right that hard-failing
//! is bad when the cause is mundane — a local model that was deleted, or a
//! provider with no key. This module reconciles the two:
//!
//! 1. **Local engines are always safe to fall back to.** The audio never
//!    leaves the machine, so no privacy boundary is crossed.
//! 2. **Falling back to a different cloud vendor requires explicit opt-in**
//!    (`FallbackPolicy::AnyConfigured`). It is off by default.
//! 3. **A fallback is never silent.** Whatever runs is named in the result and
//!    surfaced to the HUD, so "why does this transcript look different" always
//!    has an answer.
//!
//! That keeps the promise that matters — no secret provider switching — while
//! letting Clide recover from the boring failures.

use serde::{Deserialize, Serialize};

use crate::credentials::Credentials;
use crate::providers::{ProviderRegistry, TranscriptionProvider};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FallbackPolicy {
    /// Never substitute. A failure is reported and the user decides.
    Off,
    /// Fall back only to a local engine. The audio stays on this machine.
    #[default]
    LocalOnly,
    /// Also allow another configured cloud provider. Opt-in: this sends the
    /// recording to a vendor the user did not choose for it.
    AnyConfigured,
}

/// A backend Clide could actually use right now, and why it was chosen.
pub struct Candidate {
    pub provider: Arc<dyn TranscriptionProvider>,
    pub model: String,
}

/// Whether a provider is usable at this moment.
///
/// "Configured" means different things per backend, which is exactly what the
/// capability system is for: a local engine needs an installed model, a cloud
/// one needs a credential.
fn is_usable(provider: &Arc<dyn TranscriptionProvider>, credentials: &Credentials) -> bool {
    if provider.models().is_empty() {
        // A local engine with nothing downloaded, most often.
        return false;
    }

    if provider.capabilities().local {
        return true;
    }

    credentials.is_configured(provider.id())
}

/// Candidates to try after `failed_provider`, in order.
///
/// Local engines come first even under `AnyConfigured`: they are faster to
/// reach, cost nothing, and keep the audio here. Within that, the registry's
/// own order decides, so there is no hidden ranking to reason about.
pub fn candidates(
    registry: &ProviderRegistry,
    credentials: &Credentials,
    policy: FallbackPolicy,
    failed_provider: &str,
) -> Vec<Candidate> {
    if policy == FallbackPolicy::Off {
        return Vec::new();
    }

    let mut local = Vec::new();
    let mut cloud = Vec::new();

    for descriptor in registry.descriptors() {
        if descriptor.id == failed_provider {
            continue;
        }
        let Some(provider) = registry.get(&descriptor.id) else {
            continue;
        };
        if !is_usable(&provider, credentials) {
            continue;
        }

        // Prefer the provider's own default when it offers it, otherwise the
        // first model it actually has.
        let offered = provider.models();
        let model = offered
            .iter()
            .find(|model| model.id == provider.default_model())
            .or_else(|| offered.first())
            .map(|model| model.id.clone());

        let Some(model) = model else { continue };

        if provider.capabilities().local {
            local.push(Candidate { provider, model });
        } else if policy == FallbackPolicy::AnyConfigured {
            cloud.push(Candidate { provider, model });
        }
    }

    local.extend(cloud);
    local
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ModelStore;

    fn registry() -> ProviderRegistry {
        ProviderRegistry::new(
            reqwest::Client::new(),
            ModelStore::new(&std::env::temp_dir().join("clide-fallback-none")),
        )
    }

    fn credentials(name: &str) -> Credentials {
        let dir = std::env::temp_dir().join(format!("clide-fallback-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Credentials::new(&dir)
    }

    #[test]
    fn off_never_substitutes_anything() {
        let found = candidates(
            &registry(),
            &credentials("off"),
            FallbackPolicy::Off,
            "groq",
        );
        assert!(found.is_empty());
    }

    /// The privacy rule, as a test: with the default policy, a configured
    /// cloud provider must never be offered as a substitute.
    #[test]
    fn local_only_never_reaches_for_a_cloud_provider() {
        let credentials = credentials("local-only");
        credentials.store("openai", "sk-test").unwrap();
        credentials.store("deepgram", "dg-test").unwrap();

        let found = candidates(
            &registry(),
            &credentials,
            FallbackPolicy::LocalOnly,
            "groq",
        );

        for candidate in &found {
            assert!(
                candidate.provider.capabilities().local,
                "{} was offered under LocalOnly",
                candidate.provider.id()
            );
        }
    }

    #[test]
    fn an_unconfigured_cloud_provider_is_never_a_candidate() {
        // No credentials stored at all.
        let found = candidates(
            &registry(),
            &credentials("unconfigured"),
            FallbackPolicy::AnyConfigured,
            "groq",
        );
        assert!(
            found.iter().all(|c| c.provider.capabilities().local),
            "a cloud provider with no key was offered"
        );
    }

    #[test]
    fn the_failed_provider_is_never_its_own_fallback() {
        let credentials = credentials("self");
        credentials.store("openai", "sk-test").unwrap();

        let found = candidates(
            &registry(),
            &credentials,
            FallbackPolicy::AnyConfigured,
            "openai",
        );
        assert!(found.iter().all(|c| c.provider.id() != "openai"));
    }

    /// A local engine with nothing downloaded cannot serve a transcription, so
    /// it must not be offered as a rescue.
    ///
    /// Apple Speech is the exception and the reason this is worth having: it
    /// ships with macOS, always has a model, and is therefore the one engine
    /// that can rescue a dictation on a machine where nothing was downloaded.
    #[test]
    fn only_local_engines_with_a_usable_model_are_candidates() {
        let found = candidates(
            &registry(),
            &credentials("no-models"),
            FallbackPolicy::LocalOnly,
            "groq",
        );

        for candidate in &found {
            assert!(
                !candidate.provider.models().is_empty(),
                "{} was offered with no models installed",
                candidate.provider.id()
            );
        }

        assert!(
            found.iter().any(|c| c.provider.id() == "apple"),
            "Apple Speech ships with macOS and should always be able to rescue"
        );
    }

    #[test]
    fn cloud_candidates_come_after_local_ones() {
        let credentials = credentials("order");
        credentials.store("openai", "sk-test").unwrap();

        let found = candidates(
            &registry(),
            &credentials,
            FallbackPolicy::AnyConfigured,
            "groq",
        );

        let first_cloud = found.iter().position(|c| !c.provider.capabilities().local);
        let last_local = found.iter().rposition(|c| c.provider.capabilities().local);

        if let (Some(cloud), Some(local)) = (first_cloud, last_local) {
            assert!(local < cloud, "a cloud provider outranked a local one");
        }
    }
}
