//! H10: the HTTP adapters against a mock server speaking each API (no real service is ever
//! asked): the chat-completions API and the Messages API — the request each sends (its path, its
//! headers, the key, the model and the messages) and the reply each reads; the models a server
//! lists; a refusal, a malformed reply, a server too slow and one not there; the options making
//! the backend, the key read from its variable.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use hearth_ai::http::{Anthropic, OpenAiCompatible};
use hearth_ai::{ConversationBackend, Error, Request};
use hearth_core::options::{ConversationApi, ConversationBackend as Kind, ConversationOptions};
use serde_json::Value;

/// A request as the mock server took it.
#[derive(Debug, Clone)]
struct Taken {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Taken {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    fn json(&self) -> Value {
        serde_json::from_str(&self.body).expect("a JSON body")
    }
}

/// A server on this computer answering `replies` requests in turn — each with a status, a body
/// and a delay — and telling what it was asked.
fn mock(replies: Vec<(u16, String, Duration)>) -> (String, Receiver<Taken>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let url = format!("http://{}/v1", listener.local_addr().expect("an address"));
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for (status, body, delay) in replies {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream.try_clone().expect("the stream"));
            let mut line = String::new();
            reader.read_line(&mut line).expect("the request line");
            let mut parts = line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_owned();
            let path = parts.next().unwrap_or_default().to_owned();
            let mut headers = Vec::new();
            let mut length = 0usize;
            loop {
                let mut h = String::new();
                reader.read_line(&mut h).expect("a header");
                let h = h.trim_end();
                if h.is_empty() {
                    break;
                }
                if let Some((k, v)) = h.split_once(':') {
                    let (k, v) = (k.trim().to_lowercase(), v.trim().to_owned());
                    if k == "content-length" {
                        length = v.parse().unwrap_or(0);
                    }
                    headers.push((k, v));
                }
            }
            let mut got = vec![0u8; length];
            reader.read_exact(&mut got).expect("the body");
            let _ = tx.send(Taken {
                method,
                path,
                headers,
                body: String::from_utf8_lossy(&got).into_owned(),
            });
            std::thread::sleep(delay);
            let mut stream = stream;
            let reply = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    (url, rx)
}

fn ok(body: &str) -> (u16, String, Duration) {
    (200, body.to_owned(), Duration::ZERO)
}

fn request() -> Request {
    Request {
        system: "You give voice to one person.".to_owned(),
        user: "The speech act: a greeting.".to_owned(),
        max_tokens: 40,
        temperature: 0.7,
    }
}

const WAIT: Duration = Duration::from_secs(5);

#[test]
fn the_chat_completions_adapter_sends_and_reads_as_that_api_does() {
    let (url, taken) = mock(vec![
        ok(
            r#"{"id":"x","choices":[{"index":0,"message":{"role":"assistant","content":"Hello, friend."},"finish_reason":"stop"}]}"#,
        ),
        ok(
            r#"{"object":"list","data":[{"id":"small-model","object":"model"},{"id":"large-model","object":"model"}]}"#,
        ),
    ]);
    let mut b = OpenAiCompatible::new(&url, "small-model", Some("k-123".to_owned()), WAIT);
    assert_eq!(b.complete(&request()), Ok("Hello, friend.".to_owned()));
    let t = taken.recv().expect("asked");
    assert_eq!(
        (t.method.as_str(), t.path.as_str()),
        ("POST", "/v1/chat/completions")
    );
    assert_eq!(t.header("authorization"), Some("Bearer k-123"));
    let j = t.json();
    assert_eq!(j["model"], "small-model");
    assert_eq!(j["messages"][0]["role"], "system");
    assert_eq!(j["messages"][0]["content"], "You give voice to one person.");
    assert_eq!(j["messages"][1]["role"], "user");
    assert_eq!(j["max_tokens"], 40);
    assert_eq!(j["stream"], false);
    assert_eq!(
        b.models(),
        Ok(vec!["small-model".to_owned(), "large-model".to_owned()])
    );
    let t = taken.recv().expect("asked");
    assert_eq!((t.method.as_str(), t.path.as_str()), ("GET", "/v1/models"));
}

#[test]
fn the_messages_adapter_sends_and_reads_as_that_api_does() {
    let (url, taken) = mock(vec![
        ok(
            r#"{"id":"m","type":"message","role":"assistant","content":[{"type":"text","text":"Welcome, "},{"type":"text","text":"stranger."}],"stop_reason":"end_turn"}"#,
        ),
        ok(
            r#"{"data":[{"id":"some-model","type":"model","display_name":"Some model"}],"has_more":false}"#,
        ),
    ]);
    let mut b = Anthropic::new(&url, "some-model", Some("sk-test".to_owned()), WAIT);
    assert_eq!(b.complete(&request()), Ok("Welcome, stranger.".to_owned()));
    let t = taken.recv().expect("asked");
    assert_eq!(
        (t.method.as_str(), t.path.as_str()),
        ("POST", "/v1/messages")
    );
    assert_eq!(t.header("x-api-key"), Some("sk-test"));
    assert_eq!(t.header("anthropic-version"), Some("2023-06-01"));
    let j = t.json();
    assert_eq!(j["model"], "some-model");
    assert_eq!(j["system"], "You give voice to one person.");
    assert_eq!(j["messages"][0]["role"], "user");
    assert_eq!(j["messages"].as_array().map(Vec::len), Some(1));
    assert_eq!(j["max_tokens"], 40);
    assert_eq!(b.models(), Ok(vec!["some-model".to_owned()]));
}

#[test]
fn refusals_malformed_replies_slow_and_missing_servers_are_told_apart() {
    let (url, _taken) = mock(vec![
        (
            401,
            r#"{"error":{"message":"the key is not valid","type":"auth"}}"#.to_owned(),
            Duration::ZERO,
        ),
        ok("this is not JSON"),
        ok(r#"{"choices":[]}"#),
        (200, r#"{"choices":[]}"#.to_owned(), Duration::from_secs(3)),
    ]);
    let mut b = OpenAiCompatible::new(&url, "m", None, WAIT);
    assert_eq!(
        b.complete(&request()),
        Err(Error::Refused(401, "the key is not valid".to_owned()))
    );
    assert!(matches!(b.complete(&request()), Err(Error::Malformed(_))));
    assert!(matches!(b.complete(&request()), Err(Error::Malformed(_))));
    let mut slow = OpenAiCompatible::new(&url, "m", None, Duration::from_millis(400));
    assert!(matches!(
        slow.complete(&request()),
        Err(Error::Unreachable(_))
    ));
    // No server there.
    let free = TcpListener::bind("127.0.0.1:0").expect("a port");
    let gone = format!("http://{}/v1", free.local_addr().expect("an address"));
    drop(free);
    let mut b = OpenAiCompatible::new(&gone, "m", None, WAIT);
    assert!(matches!(b.complete(&request()), Err(Error::Unreachable(_))));
    // No model chosen: nothing is sent.
    let mut b = OpenAiCompatible::new(&gone, "", None, WAIT);
    assert_eq!(b.complete(&request()), Err(Error::NoModel));
}

#[test]
fn the_options_make_the_backend_with_the_key_from_its_variable() {
    // Off: never asked.
    let off = ConversationOptions::default();
    assert!(!off.on());
    let mut b = hearth_ai::backend(&off).expect("none");
    assert_eq!(b.complete(&request()), Err(Error::Off));
    // A provider's: its key from the variable named, and none without it.
    let remote = ConversationOptions {
        backend: Kind::Remote,
        api: ConversationApi::Anthropic,
        url: "https://api.example.invalid/v1".to_owned(),
        model: "m".to_owned(),
        key_env: "HEARTH_SURELY_UNSET_KEY_VARIABLE".to_owned(),
        ..ConversationOptions::default()
    };
    assert!(matches!(hearth_ai::backend(&remote), Err(Error::NoKey(_))));
    let unnamed = ConversationOptions {
        key_env: String::new(),
        ..remote.clone()
    };
    assert!(matches!(hearth_ai::backend(&unnamed), Err(Error::NoKey(_))));
    // A local server: asked through the chat-completions adapter, with the key the variable
    // holds (a variable every test run has stands in for one).
    let path = std::env::var("PATH").expect("a PATH");
    let (url, taken) = mock(vec![ok(r#"{"choices":[{"message":{"content":"Hi."}}]}"#)]);
    let local = ConversationOptions {
        backend: Kind::Local,
        api: ConversationApi::OpenAiCompatible,
        url,
        model: "tiny".to_owned(),
        key_env: "PATH".to_owned(),
        ..ConversationOptions::default()
    };
    let (reply, _) = hearth_ai::test(&local).expect("an answer");
    assert_eq!(reply, "Hi.");
    let t = taken.recv().expect("asked");
    assert_eq!(
        t.header("authorization"),
        Some(format!("Bearer {path}").as_str())
    );
    assert_eq!(t.json()["model"], "tiny");
}
