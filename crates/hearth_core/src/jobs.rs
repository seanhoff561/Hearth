//! The game's worker threads, by priority (E4.1 §4.6): so the player is never starved.
//!
//! - **Interactive** — what the player is waiting on in a menu (the creator's people, a place's
//!   details): threads at the normal priority.
//! - **Near field** — the cubes generated and meshed about the player: rayon's global pool, so
//!   every parallel loop not run elsewhere runs here, at the normal priority.
//! - **LOD** — the distant terrain's tiles, nearest first: below normal.
//! - **Background** — preparation nothing waits on (the in-game globe's map, a planet's places):
//!   the lowest priority there is.
//!
//! The near field takes the machine's cores less those kept free for the main thread, rendering
//! and sound (one, two from eight cores; `PerformanceOptions`), so a frame always finds a core.
//! Each pool's threads tell the profile whom they work for (`prof::caller`). Jobs that may be
//! wanted no longer carry a [`Cancel`] and give up at their next step.

use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// How urgent a job is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Interactive,
    NearField,
    Lod,
    Background,
}

/// The pools' sizes: the cores the near field takes and those kept free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sizes {
    pub cores: usize,
    pub reserved: usize,
    pub near: usize,
    pub interactive: usize,
    pub lod: usize,
    pub background: usize,
}

impl Sizes {
    /// The sizes for a machine of `cores` cores, `workers` and `reserved` as configured (0: from
    /// the machine).
    pub fn for_machine(cores: usize, workers: usize, reserved: usize) -> Self {
        let cores = cores.max(1);
        let reserved = if reserved > 0 {
            reserved.min(cores.saturating_sub(1))
        } else if cores >= 8 {
            2
        } else {
            1
        };
        let near = if workers > 0 {
            workers
        } else {
            cores.saturating_sub(reserved).max(1)
        };
        Self {
            cores,
            reserved,
            near,
            // What the player waits on in a menu, where the world about them is not running:
            // as many as the near field.
            interactive: near.max(2),
            lod: (cores / 4).max(2),
            background: (cores / 4).max(1),
        }
    }
}

struct Pools {
    sizes: Sizes,
    interactive: rayon::ThreadPool,
    lod: rayon::ThreadPool,
    background: rayon::ThreadPool,
}

static POOLS: OnceLock<Pools> = OnceLock::new();

/// Sets the pools up for this machine (`workers` and `reserved` as configured, 0 from the
/// machine): rayon's global pool as the near field's. Call once, early, before anything runs in
/// parallel; later calls (and a global pool already made) keep what is there.
pub fn init(workers: usize, reserved: usize) -> Sizes {
    let (cores, _) = crate::prof::machine();
    let sizes = Sizes::for_machine(cores, workers, reserved);
    let made = rayon::ThreadPoolBuilder::new()
        .num_threads(sizes.near)
        .thread_name(|i| format!("near-{i}"))
        .start_handler(|_| start(Priority::NearField))
        .build_global();
    if made.is_err() {
        log::debug!("the global pool was made already");
    }
    let pools = POOLS.get_or_init(|| make(sizes));
    log::info!(
        "{} cores: {} kept free, {} for the world about the player, {} for distant terrain",
        pools.sizes.cores,
        pools.sizes.reserved,
        pools.sizes.near,
        pools.sizes.lod
    );
    pools.sizes
}

fn make(sizes: Sizes) -> Pools {
    let pool = |p: Priority, n: usize, name: &'static str| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .thread_name(move |i| format!("{name}-{i}"))
            .start_handler(move |_| start(p))
            .build()
            .expect("a pool of threads")
    };
    Pools {
        sizes,
        interactive: pool(Priority::Interactive, sizes.interactive, "interactive"),
        lod: pool(Priority::Lod, sizes.lod, "lod"),
        background: pool(Priority::Background, sizes.background, "background"),
    }
}

fn pools() -> &'static Pools {
    POOLS.get_or_init(|| {
        let (cores, _) = crate::prof::machine();
        make(Sizes::for_machine(cores, 0, 0))
    })
}

/// The pools' sizes.
pub fn sizes() -> Sizes {
    pools().sizes
}

/// A pool's thread starting: its priority with the system, and whom it works for.
fn start(p: Priority) {
    crate::prof::set_thread_caller(match p {
        Priority::Interactive => "interactive",
        Priority::NearField => "near",
        Priority::Lod => "lod",
        Priority::Background => "background",
    });
    sys::lower(p);
}

/// Runs `f` in the pool of `p`, its parallel loops there too (the near field's: here, on the
/// global pool), and waits for it.
pub fn install<R: Send>(p: Priority, f: impl FnOnce() -> R + Send) -> R {
    match p {
        Priority::NearField => f(),
        Priority::Interactive => pools().interactive.install(f),
        Priority::Lod => pools().lod.install(f),
        Priority::Background => pools().background.install(f),
    }
}

/// Runs `f` in the pool of `p` without waiting.
pub fn spawn(p: Priority, f: impl FnOnce() + Send + 'static) {
    match p {
        Priority::NearField => rayon::spawn(f),
        Priority::Interactive => pools().interactive.spawn(f),
        Priority::Lod => pools().lod.spawn(f),
        Priority::Background => pools().background.spawn(f),
    }
}

/// The pool of `p`, for code that keeps one (the distant terrain's streamer).
pub fn pool(p: Priority) -> Option<&'static rayon::ThreadPool> {
    match p {
        Priority::NearField => None,
        Priority::Interactive => Some(&pools().interactive),
        Priority::Lod => Some(&pools().lod),
        Priority::Background => Some(&pools().background),
    }
}

/// Tells jobs that their work is wanted no longer: they give up at their next step.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Threads' priorities with the system.
mod sys {
    use super::Priority;

    /// Lowers the calling thread's priority for jobs of `p` (a thread may always be lowered;
    /// raising one needs rights a game does not have).
    #[cfg(target_os = "linux")]
    pub fn lower(p: Priority) {
        let nice = match p {
            Priority::Interactive | Priority::NearField => return,
            Priority::Lod => 5,
            Priority::Background => 15,
        };
        // SAFETY: the calling thread's id, and setpriority on it alone (Linux's threads each
        // have their own nice value).
        unsafe {
            let tid = libc::syscall(libc::SYS_gettid) as libc::id_t;
            libc::setpriority(libc::PRIO_PROCESS, tid, nice);
        }
    }

    #[cfg(windows)]
    pub fn lower(p: Priority) {
        use windows_sys::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
            THREAD_PRIORITY_LOWEST,
        };
        let prio = match p {
            Priority::Interactive | Priority::NearField => return,
            Priority::Lod => THREAD_PRIORITY_BELOW_NORMAL,
            Priority::Background => THREAD_PRIORITY_LOWEST,
        };
        // SAFETY: the calling thread's pseudo-handle and a valid priority.
        unsafe {
            SetThreadPriority(GetCurrentThread(), prio);
        }
    }

    #[cfg(not(any(target_os = "linux", windows)))]
    pub fn lower(_p: Priority) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cores_are_kept_for_the_main_thread() {
        let s = Sizes::for_machine(4, 0, 0);
        assert_eq!((s.reserved, s.near, s.interactive), (1, 3, 3));
        let s = Sizes::for_machine(8, 0, 0);
        assert_eq!((s.reserved, s.near, s.interactive), (2, 6, 6));
        let s = Sizes::for_machine(16, 0, 0);
        assert_eq!((s.reserved, s.near), (2, 14));
        // One core: it is shared.
        let s = Sizes::for_machine(1, 0, 0);
        assert_eq!(s.near, 1);
        // As configured.
        let s = Sizes::for_machine(8, 3, 0);
        assert_eq!(s.near, 3);
        let s = Sizes::for_machine(8, 0, 4);
        assert_eq!((s.reserved, s.near), (4, 4));
    }

    #[test]
    fn jobs_run_in_their_pools_and_say_whom_they_work_for() {
        let who = install(Priority::Background, || {
            (
                crate::prof::current_caller(),
                std::thread::current().name().map(str::to_owned),
            )
        });
        assert_eq!(who.0, "background");
        assert!(who.1.is_some_and(|n| n.starts_with("background-")));
        let (tx, rx) = std::sync::mpsc::channel();
        spawn(Priority::Lod, move || {
            let _ = tx.send(crate::prof::current_caller());
        });
        assert_eq!(rx.recv().ok(), Some("lod"));
        let c = Cancel::new();
        let d = c.clone();
        assert!(!d.is_cancelled());
        c.cancel();
        assert!(d.is_cancelled());
    }
}
