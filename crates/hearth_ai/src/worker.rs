//! Asking the model on a thread of its own (V2.1 §10.4; H10): the world never waits on it. Jobs
//! wait their turn — a player's typed words before the people's lines — and one waiting longer
//! than it may is let go unasked; replies are picked up when they come.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::{ConversationBackend, Error, Request};

/// The most jobs kept waiting (the oldest of the people's lines let go first).
pub const WAITING: usize = 6;

/// Something to ask.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: u64,
    pub request: Request,
    /// Asked ahead of the rest (a player's typed words).
    pub urgent: bool,
    /// How long it may wait to be asked.
    pub wait: Duration,
}

/// A reply, and how long it took from being handed over.
#[derive(Debug, Clone, PartialEq)]
pub struct Done {
    pub id: u64,
    pub reply: Result<String, Error>,
    pub took: Duration,
}

struct Queue {
    jobs: VecDeque<(Job, Instant)>,
    closed: bool,
}

/// The thread asking the model, and the jobs waiting for it.
pub struct Worker {
    queue: Arc<(Mutex<Queue>, Condvar)>,
    done: Receiver<Done>,
    /// Sent but not yet answered.
    out: usize,
}

impl Worker {
    /// A thread asking `backend`.
    pub fn spawn(mut backend: Box<dyn ConversationBackend>) -> Self {
        let queue = Arc::new((
            Mutex::new(Queue {
                jobs: VecDeque::new(),
                closed: false,
            }),
            Condvar::new(),
        ));
        let (tx, done): (Sender<Done>, Receiver<Done>) = channel();
        let q = Arc::clone(&queue);
        let spawned = std::thread::Builder::new()
            .name("conversation".to_owned())
            .spawn(move || {
                loop {
                    let next = {
                        let (lock, ready) = &*q;
                        let Ok(mut g) = lock.lock() else {
                            return;
                        };
                        loop {
                            if g.closed {
                                return;
                            }
                            if let Some(j) = g.jobs.pop_front() {
                                break j;
                            }
                            g = match ready.wait(g) {
                                Ok(g) => g,
                                Err(_) => return,
                            };
                        }
                    };
                    let (job, since) = next;
                    let reply = if since.elapsed() > job.wait {
                        Err(Error::Unreachable("let go: it waited too long".to_owned()))
                    } else {
                        backend.complete(&job.request)
                    };
                    let d = Done {
                        id: job.id,
                        reply,
                        took: since.elapsed(),
                    };
                    if tx.send(d).is_err() {
                        return;
                    }
                }
            });
        if let Err(e) = spawned {
            log::warn!("the conversation thread could not be started: {e}");
        }
        Self {
            queue,
            done,
            out: 0,
        }
    }

    /// Hands a job over to be asked; false if it was let go at once (too many waiting).
    pub fn submit(&mut self, job: Job) -> bool {
        let (lock, ready) = &*self.queue;
        let Ok(mut g) = lock.lock() else {
            return false;
        };
        if !job.urgent && g.jobs.len() >= WAITING {
            // The oldest of the people's lines waiting is stale by now.
            match g.jobs.iter().position(|(j, _)| !j.urgent) {
                Some(k) => {
                    g.jobs.remove(k);
                    self.out = self.out.saturating_sub(1);
                }
                None => return false,
            }
        }
        if job.urgent {
            g.jobs.push_front((job, Instant::now()));
        } else {
            g.jobs.push_back((job, Instant::now()));
        }
        self.out += 1;
        ready.notify_one();
        true
    }

    /// The replies come since last asked.
    pub fn poll(&mut self) -> Vec<Done> {
        let out: Vec<Done> = self.done.try_iter().collect();
        self.out = self.out.saturating_sub(out.len());
        out
    }

    /// Jobs handed over and not yet answered.
    pub fn waiting(&self) -> usize {
        self.out
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // The thread finishes what it is asking and stops; nothing waits for it.
        let (lock, ready) = &*self.queue;
        if let Ok(mut g) = lock.lock() {
            g.closed = true;
            g.jobs.clear();
        }
        ready.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Scripted;

    #[test]
    fn replies_come_back_and_typed_words_go_first() {
        let mut w = Worker::spawn(Box::new(Scripted(|r: &Request| {
            std::thread::sleep(Duration::from_millis(20));
            Ok(r.user.clone())
        })));
        let job = |id: u64, urgent: bool| Job {
            id,
            request: Request {
                system: String::new(),
                user: format!("job {id}"),
                max_tokens: 10,
                temperature: 0.0,
            },
            urgent,
            wait: Duration::from_secs(5),
        };
        for id in 1..=3 {
            assert!(w.submit(job(id, false)));
        }
        assert!(w.submit(job(9, true)));
        let t0 = Instant::now();
        let mut got = Vec::new();
        while got.len() < 4 && t0.elapsed() < Duration::from_secs(5) {
            got.extend(w.poll());
            std::thread::sleep(Duration::from_millis(5));
        }
        let order: Vec<u64> = got.iter().map(|d| d.id).collect();
        assert_eq!(order.len(), 4, "{order:?}");
        // The urgent job is asked as soon as the one under way is done.
        assert!(order.iter().position(|&i| i == 9) <= Some(1), "{order:?}");
        assert!(got.iter().all(|d| d.reply.is_ok()));
        assert_eq!(w.waiting(), 0);
    }
}
