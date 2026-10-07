//! The optional conversation backend (V2.1 §10.4; H10). Off by default, and the game is whole
//! without it: the people speak in speech acts rendered from templates, and the player speaks
//! with the talk wheel. Set up — a model served on the player's own computer or a provider's over
//! the internet — it does two things:
//!
//! - **phrases** the people's speech acts as natural lines: the model is told the speaker's own
//!   state only ([`hearth_people::converse::Context`]), and what it says is held to the
//!   [`Lexicon`] — no word outside what the speaker may say, no technique it does not know, no
//!   name it does not know, nothing of later ages — before anyone sees it;
//! - **reads** what the player types to someone as one of the speech acts a player may make,
//!   offering the likeliest acts to choose from when it is unsure.
//!
//! Nothing it returns changes the world but through a speech act the game validates and the
//! player could have chosen on the wheel, and the people's decisions never wait on it: it is
//! asked on a thread of its own ([`Worker`]), and a reply too late is let go.

pub mod http;
pub mod lexicon;
pub mod parse;
pub mod prompt;
pub mod worker;

use std::time::Duration;

use hearth_core::options::{ConversationApi, ConversationBackend as Kind, ConversationOptions};

pub use lexicon::{Lexicon, Rejected};
pub use parse::{Parsed, guess, read_parse};
pub use prompt::Prompts;
pub use worker::{Done, Job, Worker};

/// What a model is asked: its instructions, the request, how long a reply may be and how free.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
    /// 0 (the likeliest words) … 1 (freer).
    pub temperature: f32,
}

/// Why a model gave no reply.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The backend is off.
    Off,
    /// No model chosen.
    NoModel,
    /// The API key's variable is unset (its name).
    NoKey(String),
    /// The server could not be reached, or did not answer in time.
    Unreachable(String),
    /// The server refused (its status and what it said).
    Refused(u16, String),
    /// The reply was not what the API promises.
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Off => write!(f, "the conversation backend is off"),
            Error::NoModel => write!(f, "no model is chosen"),
            Error::NoKey(var) => write!(f, "the API key's variable {var} is not set"),
            Error::Unreachable(why) => write!(f, "the server could not be reached: {why}"),
            Error::Refused(status, why) => write!(f, "the server refused ({status}): {why}"),
            Error::Malformed(why) => write!(f, "the reply could not be read: {why}"),
        }
    }
}

impl std::error::Error for Error {}

/// A language model the game may ask (V2.1 §10.4): none, one served locally, or a remote API.
pub trait ConversationBackend: Send {
    /// Asks the model; the text of its reply.
    fn complete(&mut self, request: &Request) -> Result<String, Error>;

    /// The models the server offers, where it lists them (none are assumed).
    fn models(&mut self) -> Result<Vec<String>, Error>;
}

/// No backend: never asked (the default).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoBackend;

impl ConversationBackend for NoBackend {
    fn complete(&mut self, _: &Request) -> Result<String, Error> {
        Err(Error::Off)
    }

    fn models(&mut self) -> Result<Vec<String>, Error> {
        Err(Error::Off)
    }
}

/// A backend that answers from a function: for tests, and for replaying recorded replies.
pub struct Scripted<F: FnMut(&Request) -> Result<String, Error> + Send>(pub F);

impl<F: FnMut(&Request) -> Result<String, Error> + Send> ConversationBackend for Scripted<F> {
    fn complete(&mut self, request: &Request) -> Result<String, Error> {
        (self.0)(request)
    }

    fn models(&mut self) -> Result<Vec<String>, Error> {
        Ok(vec!["scripted".to_owned()])
    }
}

/// The backend the options set up: none when off; an HTTP adapter for the API asked, with the
/// key read from its environment variable (a local server needs none).
pub fn backend(o: &ConversationOptions) -> Result<Box<dyn ConversationBackend>, Error> {
    if !o.on() {
        return Ok(Box::new(NoBackend));
    }
    let key = if o.key_env.is_empty() {
        None
    } else {
        match std::env::var(&o.key_env) {
            Ok(k) if !k.trim().is_empty() => Some(k.trim().to_owned()),
            _ => return Err(Error::NoKey(o.key_env.clone())),
        }
    };
    if o.backend == Kind::Remote && key.is_none() {
        return Err(Error::NoKey(if o.key_env.is_empty() {
            "(none named)".to_owned()
        } else {
            o.key_env.clone()
        }));
    }
    // Slower than the budget is too late; the reply is still read to keep the server's count.
    let timeout = Duration::from_millis(u64::from(o.budget_ms) * 2 + 2_000);
    Ok(match o.api {
        ConversationApi::OpenAiCompatible => {
            Box::new(http::OpenAiCompatible::new(&o.url, &o.model, key, timeout))
        }
        ConversationApi::Anthropic => {
            Box::new(http::Anthropic::new(&o.url, &o.model, key, timeout))
        }
    })
}

/// Tries a backend with a harmless request (the options screen's Test): its reply and how long
/// it took.
pub fn test(o: &ConversationOptions) -> Result<(String, Duration), Error> {
    let mut b = backend(o)?;
    if !o.on() {
        return Err(Error::Off);
    }
    if o.model.is_empty() {
        return Err(Error::NoModel);
    }
    let t0 = std::time::Instant::now();
    let reply = b.complete(&Request {
        system: "Reply with one short, friendly greeting of a few words.".to_owned(),
        user: "Greet a traveller.".to_owned(),
        max_tokens: 30,
        temperature: 0.5,
    })?;
    Ok((reply.trim().to_owned(), t0.elapsed()))
}
