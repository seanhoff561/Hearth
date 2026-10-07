//! The HTTP adapters (V2.1 §10.4; H10): the chat-completions API most model servers speak — on
//! one's own computer Ollama, LM Studio, a llama.cpp server or vLLM, and many providers over the
//! internet — and the Anthropic Messages API. Each is given the server's address, the model the
//! player chose from the server's own list and, for a provider, the key from the environment;
//! none of them names a model of its own.

use std::time::Duration;

use serde_json::{Value, json};

use crate::{ConversationBackend, Error, Request};

/// The Messages API's version header.
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Whether an address is this computer's own (asked directly, never through a proxy).
pub fn is_local(url: &str) -> bool {
    let rest = url
        .split_once("://")
        .map_or(url, |(_, r)| r)
        .split('/')
        .next()
        .unwrap_or("");
    let host = if let Some(v6) = rest.strip_prefix('[') {
        v6.split(']').next().unwrap_or("")
    } else {
        rest.split(':').next().unwrap_or("")
    };
    host == "localhost" || host == "::1" || host.starts_with("127.")
}

fn agent(url: &str, timeout: Duration) -> ureq::Agent {
    let mut config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false);
    if is_local(url) {
        config = config.proxy(None);
    }
    config.build().into()
}

/// A reply read: its JSON, or why not.
fn read(reply: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Value, Error> {
    let mut reply = reply.map_err(|e| Error::Unreachable(e.to_string()))?;
    let status = reply.status().as_u16();
    let text = reply
        .body_mut()
        .read_to_string()
        .map_err(|e| Error::Unreachable(e.to_string()))?;
    if !(200..300).contains(&status) {
        let why = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
            .unwrap_or_else(|| text.chars().take(200).collect());
        return Err(Error::Refused(status, why));
    }
    serde_json::from_str(&text).map_err(|e| Error::Malformed(e.to_string()))
}

/// The ids of a model list (`{"data": [{"id": …}, …]}`, as both APIs list them).
fn model_ids(v: &Value) -> Result<Vec<String>, Error> {
    let list = v["data"]
        .as_array()
        .ok_or_else(|| Error::Malformed("no model list".to_owned()))?;
    Ok(list
        .iter()
        .filter_map(|m| m["id"].as_str().map(str::to_owned))
        .collect())
}

/// A server speaking the chat-completions API.
pub struct OpenAiCompatible {
    url: String,
    model: String,
    key: Option<String>,
    agent: ureq::Agent,
}

impl OpenAiCompatible {
    pub fn new(url: &str, model: &str, key: Option<String>, timeout: Duration) -> Self {
        let url = url.trim_end_matches('/').to_owned();
        Self {
            agent: agent(&url, timeout),
            url,
            model: model.to_owned(),
            key,
        }
    }
}

impl ConversationBackend for OpenAiCompatible {
    fn complete(&mut self, r: &Request) -> Result<String, Error> {
        if self.model.is_empty() {
            return Err(Error::NoModel);
        }
        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": r.system},
                {"role": "user", "content": r.user},
            ],
            "max_tokens": r.max_tokens,
            "temperature": r.temperature,
            "stream": false,
        });
        let mut req = self
            .agent
            .post(format!("{}/chat/completions", self.url))
            .header("content-type", "application/json");
        if let Some(k) = &self.key {
            req = req.header("authorization", format!("Bearer {k}"));
        }
        let v = read(req.send(body.to_string()))?;
        v["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Error::Malformed("no message in the reply".to_owned()))
    }

    fn models(&mut self) -> Result<Vec<String>, Error> {
        let mut req = self.agent.get(format!("{}/models", self.url));
        if let Some(k) = &self.key {
            req = req.header("authorization", format!("Bearer {k}"));
        }
        model_ids(&read(req.call())?)
    }
}

/// The Anthropic Messages API.
pub struct Anthropic {
    url: String,
    model: String,
    key: Option<String>,
    agent: ureq::Agent,
}

impl Anthropic {
    pub fn new(url: &str, model: &str, key: Option<String>, timeout: Duration) -> Self {
        let url = url.trim_end_matches('/').to_owned();
        Self {
            agent: agent(&url, timeout),
            url,
            model: model.to_owned(),
            key,
        }
    }
}

impl ConversationBackend for Anthropic {
    fn complete(&mut self, r: &Request) -> Result<String, Error> {
        if self.model.is_empty() {
            return Err(Error::NoModel);
        }
        let body = json!({
            "model": self.model,
            "max_tokens": r.max_tokens,
            "temperature": r.temperature,
            "system": r.system,
            "messages": [{"role": "user", "content": r.user}],
        });
        let mut req = self
            .agent
            .post(format!("{}/messages", self.url))
            .header("content-type", "application/json")
            .header("anthropic-version", ANTHROPIC_VERSION);
        if let Some(k) = &self.key {
            req = req.header("x-api-key", k);
        }
        let v = read(req.send(body.to_string()))?;
        let text: String = v["content"]
            .as_array()
            .ok_or_else(|| Error::Malformed("no content in the reply".to_owned()))?
            .iter()
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .collect();
        if text.is_empty() {
            return Err(Error::Malformed("no text in the reply".to_owned()));
        }
        Ok(text)
    }

    fn models(&mut self) -> Result<Vec<String>, Error> {
        let mut req = self
            .agent
            .get(format!("{}/models", self.url))
            .header("anthropic-version", ANTHROPIC_VERSION);
        if let Some(k) = &self.key {
            req = req.header("x-api-key", k);
        }
        model_ids(&read(req.call())?)
    }
}

#[cfg(test)]
mod tests {
    use super::is_local;

    #[test]
    fn loopback_addresses_are_local() {
        assert!(is_local("http://127.0.0.1:11434/v1"));
        assert!(is_local("http://localhost:1234/v1"));
        assert!(is_local("http://[::1]:8080/v1"));
        assert!(!is_local("https://api.example.org/v1"));
        assert!(!is_local("http://10.0.0.5:11434/v1"));
    }
}
