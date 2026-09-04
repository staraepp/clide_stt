//! Text refinement through a cloud chat model.
//!
//! Reuses the API key the user already stored for transcription — the same
//! Groq or OpenAI account, a different endpoint. That is deliberate: nobody
//! should have to paste a second key to tidy their own sentences.
//!
//! # Privacy
//!
//! These send the transcript to a third party. That is a materially different
//! decision from Apple Intelligence, which never leaves the Mac, so cloud
//! refiners are **off until the user turns them on** and say plainly in the UI
//! that the text leaves the machine.

use async_trait::async_trait;
use serde::Deserialize;

use super::traits::{RefineError, RefineRequest, Refiner};
use crate::credentials::Credentials;

/// An OpenAI-shaped chat completion endpoint.
pub struct CloudRefiner {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    /// Which stored credential to use — the same id as the STT provider.
    credential_key: &'static str,
    endpoint: &'static str,
    model: &'static str,
    http: reqwest::Client,
    credentials: Credentials,
}

impl CloudRefiner {
    pub fn groq(http: reqwest::Client, credentials: Credentials) -> Self {
        Self {
            id: "groq-rewrite",
            name: "Groq",
            description: "Fast cloud rewriting on your Groq key. The transcript leaves your Mac.",
            credential_key: "groq",
            endpoint: "https://api.groq.com/openai/v1/chat/completions",
            model: "llama-3.3-70b-versatile",
            http,
            credentials,
        }
    }

    pub fn openai(http: reqwest::Client, credentials: Credentials) -> Self {
        Self {
            id: "openai-rewrite",
            name: "OpenAI",
            description: "Rewriting on your OpenAI key. The transcript leaves your Mac.",
            credential_key: "openai",
            endpoint: "https://api.openai.com/v1/chat/completions",
            model: "gpt-4o-mini",
            http,
            credentials,
        }
    }

    fn key(&self) -> Result<String, RefineError> {
        self.credentials
            .read(self.credential_key)
            .ok()
            .flatten()
            .filter(|key| !key.trim().is_empty())
            .ok_or(RefineError::Unavailable { engine: self.id })
    }
}

#[async_trait]
impl Refiner for CloudRefiner {
    fn id(&self) -> &'static str {
        self.id
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn local(&self) -> bool {
        false
    }

    fn availability(&self) -> Result<(), RefineError> {
        self.key().map(|_| ())
    }

    async fn refine(&self, request: RefineRequest) -> Result<String, RefineError> {
        let key = self.key()?;

        // The style instruction is the system message, so the transcript
        // arrives as content. A transcript that reads like a question is then
        // far less likely to be answered rather than tidied.
        let body = serde_json::json!({
            "model": self.model,
            "temperature": 0,
            "messages": [
                { "role": "system", "content": request.style.instruction() },
                { "role": "user", "content": request.text },
            ],
        });

        let response = self
            .http
            .post(self.endpoint)
            .bearer_auth(key)
            .json(&body)
            .send()
            .await
            .map_err(|error| RefineError::Failed {
                engine: self.id,
                detail: error.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(RefineError::Declined {
                engine: self.id,
                detail: format!("the model host returned {status}"),
            });
        }

        let payload: ChatResponse = response.json().await.map_err(|error| RefineError::Failed {
            engine: self.id,
            detail: error.to_string(),
        })?;

        let refined = payload
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content.trim().to_string())
            .unwrap_or_default();

        if refined.is_empty() {
            return Err(RefineError::Declined {
                engine: self.id,
                detail: "the model returned nothing".into(),
            });
        }

        Ok(refined)
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credentials(name: &str) -> Credentials {
        let dir = std::env::temp_dir().join(format!("clide-cloud-refine-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Credentials::new(&dir)
    }

    #[test]
    fn cloud_refiners_declare_themselves_as_leaving_the_mac() {
        let store = credentials("local-flag");
        for refiner in [
            CloudRefiner::groq(reqwest::Client::new(), store.clone()),
            CloudRefiner::openai(reqwest::Client::new(), store.clone()),
        ] {
            assert!(!refiner.local(), "{} claimed to be local", refiner.id());
            assert!(
                refiner.description().contains("leaves your Mac"),
                "{} does not warn that text leaves the machine",
                refiner.id()
            );
        }
    }

    /// Without a key there is nothing to send with, and the UI needs to say so
    /// rather than failing at the end of a dictation.
    #[test]
    fn an_unconfigured_refiner_is_unavailable() {
        let refiner = CloudRefiner::groq(reqwest::Client::new(), credentials("no-key"));
        assert!(refiner.availability().is_err());
        assert!(!refiner.descriptor().available);
        assert!(refiner.descriptor().unavailable_reason.is_some());
    }

    /// It reuses the transcription key rather than asking for a second one.
    #[test]
    fn it_reads_the_same_credential_as_the_stt_provider() {
        let store = credentials("shared-key");
        store.store("groq", "gsk_example").unwrap();

        let groq = CloudRefiner::groq(reqwest::Client::new(), store.clone());
        assert!(groq.availability().is_ok());

        // OpenAI's key is separate and still missing.
        let openai = CloudRefiner::openai(reqwest::Client::new(), store);
        assert!(openai.availability().is_err());
    }
}
