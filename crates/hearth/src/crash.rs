//! What is left behind when the game goes wrong (E4.1 §3): everything logged is written to a log
//! file in the game directory's `logs` folder as well as to the console (the session before's
//! kept beside it as `…-previous.log`); a panic is logged there with its thread and backtrace; an
//! allocation the system refuses is written there, without allocating, before the process
//! aborts; and a session that ended without saying so (killed by the system for its memory, a
//! stack overflow, a driver's fault) is reported when the next one starts, with the memory it
//! last had.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// The session's log file, once opened.
static LOG: OnceLock<File> = OnceLock::new();
/// Where it is.
static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
/// The resident memory (MiB) last noted (`note_memory`), for a session that ends without a word.
static LAST_MIB: AtomicU64 = AtomicU64::new(0);

/// The last line of a session that ended as it should.
const CLEAN_END: &str = "the session ended cleanly";
/// What a session writes as it begins.
const BEGUN: &str = "the session began";
/// What a session ending on an error writes first.
const FATAL: &str = "FATAL:";

/// Writes every record to the console and, once it is open, the log file.
struct Tee;

impl Write for Tee {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(mut f) = LOG.get() {
            let _ = f.write_all(buf);
        }
        // A console may be missing (a window of its own): the file is what counts.
        let _ = std::io::stderr().write_all(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(mut f) = LOG.get() {
            let _ = f.flush();
        }
        Ok(())
    }
}

/// Starts logging (filtered by `RUST_LOG` as usual) to the console, and to the log file from
/// when [`open`] opens it.
pub fn init_logging() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"),
    )
    .write_style(env_logger::WriteStyle::Never)
    .target(env_logger::Target::Pipe(Box::new(Tee)))
    .try_init()
    .ok();
}

/// Opens the session's log file, `dir/<name>.log` (the one before kept as
/// `<name>-previous.log`), says there whether that session ended without a word, and logs
/// panics there from now on.
pub fn open(dir: &Path, name: &str) {
    let path = dir.join(format!("{name}.log"));
    let previous = dir.join(format!("{name}-previous.log"));
    if let Err(e) = std::fs::create_dir_all(dir) {
        log::warn!("no log file: {e}");
        return;
    }
    let before = std::fs::read_to_string(&path).ok();
    if before.is_some() {
        let _ = std::fs::rename(&path, &previous);
    }
    match File::create(&path) {
        Ok(f) => {
            let _ = LOG.set(f);
            let _ = LOG_PATH.set(path.clone());
        }
        Err(e) => {
            log::warn!("no log file at {}: {e}", path.display());
            return;
        }
    }
    install_panic_hook();
    log::info!(
        "{BEGUN}: {} {}; logging to {}",
        hearth_core::GAME_NAME,
        hearth_core::GAME_VERSION,
        path.display()
    );
    if let Some(text) = before
        && let Some(why) = ended_without_a_word(&text)
    {
        log::warn!(
            "the session before ended without a word: {why} (see {})",
            previous.display()
        );
    }
}

/// Whether a session's log shows it ended without a word, and what its last lines suggest.
fn ended_without_a_word(text: &str) -> Option<String> {
    // Ended as it should, or said why it did not; or no session's log at all.
    if text.contains(CLEAN_END) || text.contains(FATAL) || !text.contains(BEGUN) {
        return None;
    }
    if text.contains("PANIC") {
        return Some("it panicked".into());
    }
    if text.contains("OUT OF MEMORY") {
        return Some("an allocation was refused".into());
    }
    if text.contains("GPU DEVICE LOST") {
        return Some("the GPU device was lost".into());
    }
    if text.contains("GPU OUT OF MEMORY") {
        return Some("the GPU ran out of memory".into());
    }
    let mib = text
        .lines()
        .rev()
        .find_map(|l| l.split("resident ").nth(1))
        .and_then(|r| r.split_whitespace().next())
        .unwrap_or("?");
    Some(format!(
        "no panic or error was logged: killed by the system (out of memory, last seen at \
         {mib} MiB resident), a stack overflow or a driver's fault"
    ))
}

/// Logs panics with their thread and a backtrace (then the default hook prints them too).
fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unnamed");
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!("PANIC on thread {name:?}: {info}\n{backtrace}");
        default(info);
    }));
}

/// Notes the resident memory now (the main loop, every few seconds), written to the log when it
/// grows by a quarter: a session the system kills leaves its last size behind.
pub fn note_memory() {
    let Some((now, _)) = hearth_core::prof::memory() else {
        return;
    };
    let mib = now >> 20;
    let last = LAST_MIB.load(Ordering::Relaxed);
    if mib > last + last / 4 + 64 {
        LAST_MIB.store(mib, Ordering::Relaxed);
        log::info!("resident {mib} MiB");
    }
}

/// Says the session ended as it should.
pub fn clean_exit() {
    log::info!("{CLEAN_END}");
}

/// Says the session ends on an error (to the log, and to the console without a log line's
/// dressing).
pub fn fatal(message: &str) {
    log::error!("{FATAL} {message}");
    eprintln!("{message}");
}

/// The log file's path, if one is open.
pub fn log_path() -> Option<&'static Path> {
    LOG_PATH.get().map(PathBuf::as_path)
}

/// Called by the allocator when the system refuses `size` bytes: writes so to the log file with
/// nothing allocated (the process aborts after).
pub fn allocation_refused(size: usize) {
    let Some(mut f) = LOG.get() else {
        return;
    };
    let head = b"OUT OF MEMORY: an allocation of ";
    let tail = b" bytes was refused\n";
    let mut buf = [0u8; 96];
    buf[..head.len()].copy_from_slice(head);
    let mut n = head.len();
    n += write_u64(&mut buf[n..], size as u64);
    buf[n..n + tail.len()].copy_from_slice(tail);
    n += tail.len();
    let _ = f.write_all(&buf[..n]);
}

/// Writes `v` in decimal at the start of `out`; returns the digits written.
fn write_u64(out: &mut [u8], mut v: u64) -> usize {
    let mut digits = [0u8; 20];
    let mut k = 0;
    loop {
        digits[k] = b'0' + (v % 10) as u8;
        k += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    for (o, d) in out.iter_mut().zip(digits[..k].iter().rev()) {
        *o = *d;
    }
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_that_ended_without_a_word_is_told_why() {
        let begun = format!("[INFO] {BEGUN}: Hearth\n");
        assert_eq!(
            ended_without_a_word(&format!("{begun}[INFO] {CLEAN_END}\n")),
            None
        );
        assert!(
            ended_without_a_word(&format!("{begun}[ERROR] PANIC on thread \"main\": boom\n"))
                .is_some_and(|w| w.contains("panicked"))
        );
        let killed = format!("{begun}[INFO] resident 9000 MiB\n[INFO] streaming\n");
        assert!(ended_without_a_word(&killed).is_some_and(|w| w.contains("9000 MiB")));
        // A log of no session (another tool's) says nothing.
        assert_eq!(ended_without_a_word("[INFO] something\n"), None);
    }

    #[test]
    fn a_panic_is_logged_with_its_thread() {
        let dir = std::env::temp_dir().join(format!("hearth-crash-{}", std::process::id()));
        init_logging();
        open(&dir, "test");
        let joined = std::thread::Builder::new()
            .name("boom".into())
            .spawn(|| panic!("a test's panic"))
            .expect("thread")
            .join();
        assert!(joined.is_err());
        let text = std::fs::read_to_string(dir.join("test.log")).expect("the log");
        assert!(text.contains(BEGUN), "{text}");
        assert!(
            text.contains("PANIC on thread \"boom\"") && text.contains("a test's panic"),
            "{text}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn numbers_are_written_without_allocating() {
        let mut buf = [0u8; 20];
        let n = write_u64(&mut buf, 18_446_744_073_709_551_615);
        assert_eq!(&buf[..n], b"18446744073709551615");
        let n = write_u64(&mut buf, 0);
        assert_eq!(&buf[..n], b"0");
    }
}
