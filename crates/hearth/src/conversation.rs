//! The conversation backend in the game (V2.1 §10.4; H10). Off by default: then nothing here is
//! asked or sent, and the people speak in templated lines and the player with the talk wheel.
//! Set up, two things go to the model on its own thread: a line said to the player (and made out
//! by it) is phrased from the speaker's own state, and shown only once the filter has passed it —
//! the templated line stands meanwhile, and stays if the phrasing is refused or late; and what the
//! player types to someone is read as one of the acts it could have chosen on the wheel, acted on
//! when the reading is sure and plain, else offered to choose from. Nothing that comes back
//! touches the world but that act.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use hearth_ai::{ConversationBackend, Job, Lexicon, Prompts, Rejected, Worker};
use hearth_content::Content;
use hearth_content::schema::ai::Cue;
use hearth_core::options::ConversationOptions;
use hearth_craft::knowledge::Graph;
use hearth_people::converse::{Context, Guard};
use hearth_people::player::Ask;

/// What came of asking the model.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// A heard line, phrased: its id and the line.
    Phrased { line: u64, text: String },
    /// The player's typed words, read plainly as an act: made as if chosen on the wheel.
    Act { person: u64, ask: Ask },
    /// The player's typed words, unclear: the acts they may be, to choose from (none: nothing
    /// could be made of them).
    Unclear {
        person: u64,
        text: String,
        options: Vec<Ask>,
    },
}

/// A question out with the model.
enum Pending {
    Phrase {
        line: u64,
        guard: Box<Guard>,
        key: String,
        until: Instant,
    },
    Parse {
        person: u64,
        text: String,
        offered: Vec<(String, String)>,
        until: Instant,
    },
}

/// How the backend has done (for the options screen and the tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    /// Lines asked for, shown, refused by the filter, late, failed; typed words read.
    pub asked: u32,
    pub shown: u32,
    pub refused: u32,
    pub late: u32,
    pub failed: u32,
    pub read: u32,
}

/// The most phrased lines remembered (a speaker saying the same to the same listener again
/// says it the same way).
const CACHED: usize = 256;
/// The most refusals kept to look at.
const REFUSALS: usize = 32;

/// The backend as the game uses it.
pub struct Conversation {
    options: ConversationOptions,
    worker: Option<Worker>,
    lexicon: Lexicon,
    prompts: Option<Prompts>,
    cues: Vec<Cue>,
    pending: HashMap<u64, Pending>,
    next: u64,
    cache: HashMap<String, String>,
    pub stats: Stats,
    /// The last lines the filter refused, and why.
    pub refusals: Vec<(String, Rejected)>,
    /// Why a backend set up is not asked (no key, no prompts …).
    pub trouble: Option<String>,
}

impl Conversation {
    /// Off, with the content's words, prompts and cues ready.
    pub fn new(content: &Content, graph: &Graph) -> Self {
        Self {
            options: ConversationOptions::default(),
            worker: None,
            lexicon: Lexicon::from_content(content, graph),
            prompts: Prompts::from_content(content),
            cues: content.cues.iter().cloned().collect(),
            pending: HashMap::new(),
            next: 1,
            cache: HashMap::new(),
            stats: Stats::default(),
            refusals: Vec::new(),
            trouble: None,
        }
    }

    /// Set up as the options have it: a backend asked on its own thread, or none.
    pub fn configure(&mut self, o: &ConversationOptions) {
        if *o == self.options && (self.worker.is_some() || !o.on()) {
            return;
        }
        let backend = hearth_ai::backend(o);
        match backend {
            Ok(b) if o.on() => self.start(o.clone(), b),
            Ok(_) => self.stop(o.clone(), None),
            Err(e) => self.stop(o.clone(), Some(e.to_string())),
        }
    }

    /// Set up with a backend given (a test's, or one already made).
    pub fn start(&mut self, o: ConversationOptions, backend: Box<dyn ConversationBackend>) {
        self.options = o;
        self.pending.clear();
        self.cache.clear();
        if self.prompts.is_none() {
            self.worker = None;
            self.trouble = Some("the conversation prompts are missing".to_owned());
            return;
        }
        self.worker = Some(Worker::spawn(backend));
        self.trouble = None;
    }

    fn stop(&mut self, o: ConversationOptions, trouble: Option<String>) {
        self.options = o;
        self.worker = None;
        self.pending.clear();
        self.trouble = trouble;
    }

    /// Whether a backend is asked.
    pub fn on(&self) -> bool {
        self.worker.is_some()
    }

    /// Whether the backend has nothing waiting (an overheard line may be phrased).
    pub fn idle(&self) -> bool {
        self.worker.as_ref().is_some_and(|w| w.waiting() == 0)
    }

    /// Whether the player may type what it says.
    pub fn free_text(&self) -> bool {
        self.on() && self.options.free_text
    }

    /// The words the filter holds lines to.
    pub fn lexicon(&self) -> &Lexicon {
        &self.lexicon
    }

    fn budget(&self) -> Duration {
        Duration::from_millis(u64::from(self.options.budget_ms))
    }

    /// A heard line to phrase from its speaker's state; a line phrased before for the same
    /// speaker, act and listener comes back at once.
    pub fn phrase(&mut self, line: u64, context: &Context, guard: Guard) -> Option<Outcome> {
        let (Some(worker), Some(prompts)) = (self.worker.as_mut(), self.prompts.as_ref()) else {
            return None;
        };
        let key = format!(
            "{}|{}|{}|{}|{:?}",
            context.speaker, context.act, context.meaning, context.listener, context.gesture
        );
        if let Some(text) = self.cache.get(&key) {
            return match self.lexicon.check(text, &guard) {
                Ok(text) => {
                    self.stats.shown += 1;
                    Some(Outcome::Phrased { line, text })
                }
                Err(_) => None,
            };
        }
        let id = self.next;
        self.next += 1;
        let budget = Duration::from_millis(u64::from(self.options.budget_ms));
        let job = Job {
            id,
            request: prompts.phrase(context),
            urgent: false,
            wait: budget,
        };
        if worker.submit(job) {
            self.stats.asked += 1;
            self.pending.insert(
                id,
                Pending::Phrase {
                    line,
                    guard: Box::new(guard),
                    key,
                    until: Instant::now() + budget,
                },
            );
        }
        None
    }

    /// What the player typed to someone, to read as an act: `listener` says whom to (as the
    /// player knows them), `offered` the techniques it may mean (id, name).
    pub fn read(
        &mut self,
        person: u64,
        text: &str,
        listener: &str,
        offered: Vec<(String, String)>,
    ) {
        let budget = self.budget() * 2;
        let (Some(worker), Some(prompts)) = (self.worker.as_mut(), self.prompts.as_ref()) else {
            return;
        };
        let id = self.next;
        self.next += 1;
        let names: Vec<String> = offered.iter().map(|(_, n)| n.clone()).collect();
        let job = Job {
            id,
            request: prompts.parse(text, listener, &names),
            urgent: true,
            wait: budget,
        };
        if worker.submit(job) {
            self.pending.insert(
                id,
                Pending::Parse {
                    person,
                    text: text.to_owned(),
                    offered,
                    until: Instant::now() + budget,
                },
            );
        }
    }

    fn refused(&mut self, line: String, why: Rejected) {
        log::debug!("a phrased line refused ({why}): {line}");
        self.stats.refused += 1;
        self.refusals.push((line, why));
        if self.refusals.len() > REFUSALS {
            self.refusals.remove(0);
        }
    }

    /// What has come back: lines phrased and passed, typed words read; a reading out too long
    /// is offered to choose from by its cues.
    pub fn poll(&mut self) -> Vec<Outcome> {
        let Some(worker) = self.worker.as_mut() else {
            return Vec::new();
        };
        let done = worker.poll();
        let now = Instant::now();
        let mut out = Vec::new();
        for d in done {
            match self.pending.remove(&d.id) {
                Some(Pending::Phrase {
                    line,
                    guard,
                    key,
                    until,
                }) => {
                    let text = match d.reply {
                        Ok(t) => t,
                        Err(e) => {
                            log::debug!("a line could not be phrased: {e}");
                            self.stats.failed += 1;
                            continue;
                        }
                    };
                    if now > until {
                        self.stats.late += 1;
                        continue;
                    }
                    match self.lexicon.check(&text, &guard) {
                        Ok(clean) => {
                            self.stats.shown += 1;
                            if self.cache.len() >= CACHED {
                                self.cache.clear();
                            }
                            self.cache.insert(key, clean.clone());
                            out.push(Outcome::Phrased { line, text: clean });
                        }
                        Err(why) => self.refused(text, why),
                    }
                }
                Some(Pending::Parse {
                    person,
                    text,
                    offered,
                    ..
                }) => {
                    self.stats.read += 1;
                    let parsed = d
                        .reply
                        .ok()
                        .and_then(|r| hearth_ai::read_parse(&r, &offered));
                    out.push(self.settle(person, text, &offered, parsed));
                }
                None => {}
            }
        }
        // Readings out too long: the cues' guesses; lines out too long: let go.
        let late: Vec<u64> = self
            .pending
            .iter()
            .filter(|(_, p)| match p {
                Pending::Phrase { until, .. } | Pending::Parse { until, .. } => now > *until,
            })
            .map(|(id, _)| *id)
            .collect();
        for id in late {
            match self.pending.remove(&id) {
                Some(Pending::Parse {
                    person,
                    text,
                    offered,
                    ..
                }) => out.push(self.settle(person, text, &offered, None)),
                Some(Pending::Phrase { .. }) => self.stats.late += 1,
                None => {}
            }
        }
        out
    }

    /// A reading made an outcome: acted on when sure and plain, else offered to choose from —
    /// its own guesses first, then the cues'.
    fn settle(
        &self,
        person: u64,
        text: String,
        offered: &[(String, String)],
        parsed: Option<hearth_ai::Parsed>,
    ) -> Outcome {
        if let Some(p) = &parsed
            && p.settled()
            && let Some(ask) = &p.ask
        {
            return Outcome::Act {
                person,
                ask: ask.clone(),
            };
        }
        let mut options: Vec<Ask> = parsed
            .map(|p| p.ask.into_iter().chain(p.alternatives).collect())
            .unwrap_or_default();
        for a in hearth_ai::guess(&text, &self.cues, offered) {
            if !options.contains(&a) {
                options.push(a);
            }
        }
        options.truncate(hearth_ai::parse::OFFERED);
        Outcome::Unclear {
            person,
            text,
            options,
        }
    }
}
