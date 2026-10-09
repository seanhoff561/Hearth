//! Measuring where the time, the memory and the threads go (E4.1): named zones timed into one
//! table for the whole process, which the benchmarks read and print; counters; which caller a
//! thread is working for (so the terrain can count the fine tiles each kind of caller builds);
//! and the process's memory and its threads' CPU time. With the `tracy` feature, every zone is
//! also a Tracy zone, for the Tracy profiler's timeline.

use std::cell::Cell;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rustc_hash::FxHashMap;

/// How often a zone ran, for how long in all, and its longest run.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ZoneStats {
    pub count: u64,
    pub total: Duration,
    pub max: Duration,
}

static ZONES: Mutex<Option<FxHashMap<&'static str, ZoneStats>>> = Mutex::new(None);
static COUNTERS: Mutex<Option<FxHashMap<String, u64>>> = Mutex::new(None);
static GAUGES: Mutex<Option<FxHashMap<&'static str, u64>>> = Mutex::new(None);

/// A zone being timed: from its making to its drop.
pub struct Zone {
    name: &'static str,
    start: Instant,
    #[cfg(feature = "tracy")]
    _span: Option<tracy_client::Span>,
}

impl Zone {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            start: Instant::now(),
            #[cfg(feature = "tracy")]
            _span: tracy_client::Client::running()
                .map(|c| c.span_alloc(Some(name), "", file!(), line!(), 0)),
        }
    }
}

impl Drop for Zone {
    fn drop(&mut self) {
        let took = self.start.elapsed();
        if let Ok(mut z) = ZONES.lock() {
            let e = z
                .get_or_insert_with(FxHashMap::default)
                .entry(self.name)
                .or_default();
            e.count += 1;
            e.total += took;
            e.max = e.max.max(took);
        }
    }
}

/// Times the rest of the enclosing block as the named zone.
#[macro_export]
macro_rules! zone {
    ($name:expr) => {
        let _zone = $crate::prof::Zone::new($name);
    };
}

/// Every zone timed so far, longest in all first.
pub fn zones() -> Vec<(&'static str, ZoneStats)> {
    let mut v: Vec<(&'static str, ZoneStats)> = ZONES
        .lock()
        .ok()
        .and_then(|z| {
            z.as_ref()
                .map(|m| m.iter().map(|(k, v)| (*k, *v)).collect())
        })
        .unwrap_or_default();
    v.sort_by_key(|z| std::cmp::Reverse(z.1.total));
    v
}

/// Adds to a counter.
pub fn count(name: &str, n: u64) {
    if let Ok(mut c) = COUNTERS.lock() {
        *c.get_or_insert_with(FxHashMap::default)
            .entry(name.to_owned())
            .or_default() += n;
    }
}

/// Every counter, by name.
pub fn counters() -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = COUNTERS
        .lock()
        .ok()
        .and_then(|c| {
            c.as_ref()
                .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect())
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// A counter's value.
pub fn counter(name: &str) -> u64 {
    COUNTERS
        .lock()
        .ok()
        .and_then(|c| c.as_ref().and_then(|m| m.get(name).copied()))
        .unwrap_or(0)
}

/// Sets a gauge: a figure its owner keeps up to date for others to read (the server's memory
/// by kind, for the client's report).
pub fn gauge(name: &'static str, value: u64) {
    if let Ok(mut g) = GAUGES.lock() {
        g.get_or_insert_with(FxHashMap::default).insert(name, value);
    }
}

/// Every gauge, by name.
pub fn gauges() -> Vec<(&'static str, u64)> {
    let mut v: Vec<(&'static str, u64)> = GAUGES
        .lock()
        .ok()
        .and_then(|g| {
            g.as_ref()
                .map(|m| m.iter().map(|(k, v)| (*k, *v)).collect())
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Clears the zones and counters (a benchmark's next stage).
pub fn reset() {
    if let Ok(mut z) = ZONES.lock() {
        *z = None;
    }
    if let Ok(mut c) = COUNTERS.lock() {
        *c = None;
    }
}

thread_local! {
    static CALLER: Cell<&'static str> = const { Cell::new("other") };
}

/// Marks the thread as working for a caller until dropped (the one before comes back).
pub struct CallerGuard {
    before: &'static str,
}

impl Drop for CallerGuard {
    fn drop(&mut self) {
        CALLER.with(|c| c.set(self.before));
    }
}

/// The thread works for `name` (the globe, the places, the near terrain, the distant
/// terrain…) until the guard drops.
pub fn caller(name: &'static str) -> CallerGuard {
    CallerGuard {
        before: CALLER.with(|c| c.replace(name)),
    }
}

/// The caller the thread works for ("other" when none was named).
pub fn current_caller() -> &'static str {
    CALLER.with(Cell::get)
}

/// Names whom the calling thread works for when nothing else is said (a pool's threads, from
/// their start).
pub fn set_thread_caller(name: &'static str) {
    CALLER.with(|c| c.set(name));
}

/// The process's resident memory now and at its peak (bytes), where the system tells.
pub fn memory() -> Option<(u64, u64)> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let field = |name: &str| {
            status
                .lines()
                .find(|l| l.starts_with(name))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
                .map(|kb| kb * 1024)
        };
        Some((field("VmRSS:")?, field("VmHWM:")?))
    }
    #[cfg(not(target_os = "linux"))]
    {
        sys::memory()
    }
}

/// The heap as the allocator holds it (bytes): in use, and freed but kept from the system
/// (fragments and arenas' slack), where the allocator tells (glibc's).
pub fn heap() -> Option<(u64, u64)> {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        // SAFETY: mallinfo2 only reads the allocator's own counters.
        let m = unsafe { libc::mallinfo2() };
        Some(((m.uordblks + m.hblkhd) as u64, m.fordblks as u64))
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        None
    }
}

/// Gives the heap's freed pages back to the system (glibc's arenas keep them otherwise): after a
/// long journey's terrain has come and gone (E4.1 §4.7). Returns whether any were.
pub fn trim_heap() -> bool {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        // SAFETY: malloc_trim only returns the allocator's free pages to the system.
        unsafe { libc::malloc_trim(0) != 0 }
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        false
    }
}

/// The CPU time (s) each of the process's threads has used, by its name, where the system
/// tells.
pub fn thread_cpu() -> Vec<(String, f64)> {
    #[cfg(target_os = "linux")]
    {
        let ticks = 100.0;
        let mut out = Vec::new();
        let Ok(dir) = std::fs::read_dir("/proc/self/task") else {
            return out;
        };
        for t in dir.flatten() {
            let path = t.path();
            let name = std::fs::read_to_string(path.join("comm"))
                .map(|s| s.trim().to_owned())
                .unwrap_or_default();
            let Ok(stat) = std::fs::read_to_string(path.join("stat")) else {
                continue;
            };
            // Fields after the name in parentheses: utime and stime are the 12th and 13th.
            let rest = stat.rsplit_once(')').map_or("", |(_, r)| r);
            let f: Vec<&str> = rest.split_whitespace().collect();
            if f.len() > 12 {
                let u: f64 = f[11].parse().unwrap_or(0.0);
                let s: f64 = f[12].parse().unwrap_or(0.0);
                out.push((name, (u + s) / ticks));
            }
        }
        out
    }
    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}

/// The machine: its cores and its physical memory (bytes), where the system tells.
pub fn machine() -> (usize, Option<u64>) {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    #[cfg(target_os = "linux")]
    let ram = std::fs::read_to_string("/proc/meminfo").ok().and_then(|m| {
        m.lines()
            .find(|l| l.starts_with("MemTotal:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
            .map(|kb| kb * 1024)
    });
    #[cfg(not(target_os = "linux"))]
    let ram = sys::ram();
    (cores, ram)
}

#[cfg(windows)]
mod sys {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    pub fn memory() -> Option<(u64, u64)> {
        let mut c: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        // SAFETY: the current process's pseudo-handle, and a counters struct of its size.
        let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
        (ok != 0).then_some((c.WorkingSetSize as u64, c.PeakWorkingSetSize as u64))
    }

    pub fn ram() -> Option<u64> {
        let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
        m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        // SAFETY: a status struct with its length set.
        let ok = unsafe { GlobalMemoryStatusEx(&mut m) };
        (ok != 0).then_some(m.ullTotalPhys)
    }
}

#[cfg(all(not(windows), not(target_os = "linux")))]
mod sys {
    pub fn memory() -> Option<(u64, u64)> {
        None
    }

    pub fn ram() -> Option<u64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_counters_and_callers_add_up() {
        {
            zone!("prof.test");
            std::thread::sleep(Duration::from_millis(2));
        }
        let z = zones();
        let (_, s) = z.iter().find(|(n, _)| *n == "prof.test").expect("timed");
        assert!(s.count >= 1 && s.total >= Duration::from_millis(2));
        count("prof.test.count", 3);
        count("prof.test.count", 4);
        assert_eq!(counter("prof.test.count"), 7);
        assert_eq!(current_caller(), "other");
        {
            let _g = caller("globe");
            assert_eq!(current_caller(), "globe");
            {
                let _h = caller("places");
                assert_eq!(current_caller(), "places");
            }
            assert_eq!(current_caller(), "globe");
        }
        assert_eq!(current_caller(), "other");
        let (cores, _) = machine();
        assert!(cores >= 1);
        #[cfg(target_os = "linux")]
        {
            let (now, peak) = memory().expect("memory on Linux");
            assert!(now > 0 && peak >= now);
            assert!(!thread_cpu().is_empty());
        }
    }
}
