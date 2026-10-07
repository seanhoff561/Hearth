//! H10 (V2.1 §10.4) through the server, as a client meets it. With no backend (the default)
//! nothing changes: no line is phrased, typed words are not heard, and the player is sent nothing
//! it was not sent before. With one — a model server on this computer, mocked — a greeting said to
//! the player is phrased and sent in the templated sense's place; a phrasing that leaks is refused
//! and never sent; typed words plainly an act are answered as that act is; words that are unclear,
//! or would propose to pair, are offered to choose from.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use common::*;
use hearth_core::options::{ConversationApi, ConversationBackend, ConversationOptions};
use hearth_people::player::Ask;
use hearth_protocol::ToServer;
use serde_json::Value;

/// A model server on this computer answering chat completions with `reply` (given the request's
/// JSON) for as long as the test runs; its address.
fn model_server(reply: impl Fn(&Value) -> String + Send + 'static) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let url = format!("http://{}/v1", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else {
                continue;
            };
            let mut reader = BufReader::new(stream.try_clone().expect("the stream"));
            let mut length = 0usize;
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            loop {
                let mut h = String::new();
                if reader.read_line(&mut h).is_err() {
                    break;
                }
                let h = h.trim_end().to_lowercase();
                if h.is_empty() {
                    break;
                }
                if let Some(v) = h.strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0u8; length];
            if reader.read_exact(&mut body).is_err() {
                continue;
            }
            let req: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let content = reply(&req);
            let out = serde_json::json!({
                "choices": [{"message": {"role": "assistant", "content": content}}]
            })
            .to_string();
            let mut stream = stream;
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{out}",
                    out.len()
                )
                .as_bytes(),
            );
        }
    });
    url
}

/// Runs the world a little at a time until `pred` holds, for at most `secs` of wall time (the
/// model answers on its own thread, and its answer is taken as the world goes on).
fn ticking(w: &mut World, secs: f64, pred: impl Fn(&World) -> bool) -> bool {
    let t0 = std::time::Instant::now();
    while t0.elapsed().as_secs_f64() < secs {
        w.run(4);
        if pred(w) {
            return true;
        }
    }
    false
}

/// Says something on the wheel to a person and waits for their answer.
fn speak(w: &mut World, person: u64, ask: Ask) -> String {
    let n = w.acted.len();
    w.server.send(ToServer::Speak { person, ask });
    w.run(5);
    assert!(w.until(10.0, |w| w.acted.len() > n), "an answer");
    w.acted[n].2.clone()
}

/// Types words to a person; the answer (none: offered acts to choose from instead).
fn say(w: &mut World, person: u64, text: &str) -> Option<String> {
    let n = w.acted.len();
    w.clarify = None;
    w.server.send(ToServer::SayText {
        person,
        text: text.to_owned(),
    });
    assert!(
        ticking(w, 30.0, |w| w.acted.len() > n || w.clarify.is_some()),
        "typed words answered"
    );
    (w.acted.len() > n).then(|| w.acted[n].2.clone())
}

#[test]
fn with_no_backend_nothing_changes_and_with_one_lines_are_phrased_and_words_read() {
    let dir = temp("conversation");
    let mut w = World::start(&dir, hearth_save::KnowledgeMode::default(), 7);
    let t0 = std::time::Instant::now();
    while w.people.is_empty() && t0.elapsed().as_secs() < 90 {
        w.run(40);
    }
    let kin = w
        .people
        .iter()
        .filter(|v| !v.dead)
        .min_by(|a, b| {
            let d = |v: &hearth_people::PersonView| (v.pos - w.mover.pos).length();
            d(a).total_cmp(&d(b))
        })
        .map(|v| (v.id, v.pos))
        .expect("one of the family");
    w.go(kin.1.x + 1.5, kin.1.z);

    // No backend: typed words are not heard; a greeting is answered and heard as before, and
    // nothing else comes.
    let heard = w.heard.len();
    let answer = say(&mut w, kin.0, "hello there").expect("an answer");
    assert!(answer.contains("talk wheel"), "{answer}");
    let greeted = speak(&mut w, kin.0, Ask::Greet);
    println!("greeted: {greeted}");
    assert!(
        ticking(&mut w, 20.0, |w| w.heard[heard..].iter().any(|l| l.to_you)),
        "the greeting heard"
    );
    w.run(40);
    assert!(w.phrased.is_empty(), "no line phrased: {:?}", w.phrased);
    assert!(w.clarify.is_none() && w.conversing.is_none());

    // A backend: a model on this computer, a mock answering as the script has it — a greeting
    // phrased plainly, an insult phrased with a leak, typed words read by what they say.
    let phrasings = Arc::new(AtomicUsize::new(0));
    let asked = Arc::clone(&phrasings);
    let url = model_server(move |req| {
        let system = req["messages"][0]["content"].as_str().unwrap_or_default();
        let user = req["messages"][1]["content"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase();
        if system.contains("single speech act") {
            let act = if user.contains("hello there") {
                r#"{"act": "greet", "technique": null, "sure": 0.95, "or": []}"#
            } else if user.contains("partner") {
                r#"{"act": "pair", "technique": null, "sure": 0.99, "or": ["praise"]}"#
            } else {
                r#"{"act": "none", "technique": null, "sure": 0, "or": []}"#
            };
            return act.to_owned();
        }
        asked.fetch_add(1, Ordering::SeqCst);
        if user.contains("an insult") {
            "You fool, the king of Rome will hear of this!".to_owned()
        } else {
            "Hello, friend. Come and sit by the fire.".to_owned()
        }
    });
    w.server.send(ToServer::Conversation(ConversationOptions {
        backend: ConversationBackend::Local,
        api: ConversationApi::OpenAiCompatible,
        url,
        model: "mock".to_owned(),
        key_env: String::new(),
        budget_ms: 8000,
        free_text: true,
    }));
    assert!(ticking(&mut w, 10.0, |w| w.conversing.is_some()));
    assert_eq!(w.conversing.clone(), Some((true, true, None)));

    // A greeting said to the player: phrased, and sent with the heard line's id.
    let heard = w.heard.len();
    speak(&mut w, kin.0, Ask::Greet);
    assert!(
        ticking(&mut w, 30.0, |w| !w.phrased.is_empty()),
        "the greeting phrased"
    );
    let (id, text) = w.phrased[0].clone();
    println!("phrased #{id}: {text}");
    assert_eq!(text, "Hello, friend. Come and sit by the fire.");
    assert!(
        w.heard[heard..].iter().any(|l| l.id == id && l.to_you),
        "the phrasing is of the greeting heard"
    );

    // An insult phrased with a leak: refused, never sent.
    let heard = w.heard.len();
    let before = phrasings.load(Ordering::SeqCst);
    speak(&mut w, kin.0, Ask::Insult);
    assert!(
        ticking(&mut w, 30.0, |w| w.heard[heard..].iter().any(|l| l.to_you)),
        "the insult heard"
    );
    assert!(
        ticking(&mut w, 30.0, |_| phrasings.load(Ordering::SeqCst) > before),
        "the insult asked to be phrased"
    );
    w.run(80);
    let insult: Vec<u64> = w.heard[heard..].iter().map(|l| l.id).collect();
    assert!(
        !w.phrased.iter().any(|(id, _)| insult.contains(id)),
        "a leaking line sent: {:?}",
        w.phrased
    );

    // Typed words: plainly a greeting, answered as one.
    let answer = say(&mut w, kin.0, "Hello there, how are you?").expect("answered");
    println!("typed a greeting: {answer}");
    assert!(answer.contains("greet"), "{answer}");
    // Asking to pair is offered, never taken.
    assert!(say(&mut w, kin.0, "Will you be my partner?").is_none());
    let (person, _, options) = w.clarify.clone().expect("acts to choose from");
    assert_eq!(person, kin.0);
    assert_eq!(options.first().map(|o| &o.0), Some(&Ask::Pair));
    println!("offered: {options:?}");
    // Words that are no act: nothing made of them, nothing done.
    assert!(say(&mut w, kin.0, "blorp zim zam").is_none());
    let (_, _, options) = w.clarify.clone().expect("told");
    assert!(options.is_empty(), "{options:?}");
}
