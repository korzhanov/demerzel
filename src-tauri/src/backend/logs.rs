//! Логи процессов: кольцевой буфер и батчинг для UI (#9).
//!
//! stdout и stderr процесса пишутся в файлы, а буфер заполняется, дочитывая эти файлы.
//! Так процесс не зависает и не падает от EPIPE, когда приложение закрыто, а логи
//! остаются доступны после перезапуска. Буфер ограничен, старые строки вытесняются.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const DEFAULT_CAPACITY: usize = 5000;
/// Интервал батча для событий в UI.
pub const BATCH_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LogLine {
    pub stream: Stream,
    pub text: String,
}

/// Кольцевой буфер фиксированной ёмкости.
#[derive(Debug)]
pub struct RingBuffer<T> {
    capacity: usize,
    items: VecDeque<T>,
}

impl<T: Clone> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self { capacity: capacity.max(1), items: VecDeque::new() }
    }

    pub fn push(&mut self, item: T) {
        if self.items.len() == self.capacity {
            self.items.pop_front();
        }
        self.items.push_back(item);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn snapshot(&self) -> Vec<T> {
        self.items.iter().cloned().collect()
    }
}

/// Общий буфер логов процесса.
pub type SharedLog = Arc<Mutex<RingBuffer<LogLine>>>;

pub fn new_shared(capacity: usize) -> SharedLog {
    Arc::new(Mutex::new(RingBuffer::new(capacity)))
}

/// Интервал опроса файла лога.
pub const TAIL_INTERVAL: Duration = Duration::from_millis(250);
/// Сколько байт с конца файла читать при присоединении к уже работающему процессу.
pub const RESTORE_TAIL_BYTES: u64 = 256 * 1024;

/// Берёт из буфера только полные строки. Возвращает, сколько байт съедено, и строки.
/// `skip_head`: начало буфера может быть серединой строки — её отбрасываем.
pub fn take_complete_lines(buf: &[u8], skip_head: bool) -> (usize, Vec<String>) {
    let Some(last) = buf.iter().rposition(|b| *b == b'\n') else {
        return (0, Vec::new());
    };
    let consumed = last + 1;
    let mut text = &buf[..consumed];
    if skip_head {
        match text.iter().position(|b| *b == b'\n') {
            Some(p) => text = &text[p + 1..],
            None => return (consumed, Vec::new()),
        }
    }
    let lines = String::from_utf8_lossy(text).lines().map(str::to_string).collect();
    (consumed, lines)
}

/// Читает байты файла начиная с `offset`. Если файл стал меньше (его перезаписали), читает с начала.
fn read_from(path: &Path, offset: u64) -> io::Result<(u64, Vec<u8>)> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let from = if len < offset { 0 } else { offset };
    file.seek(SeekFrom::Start(from))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok((from, buf))
}

/// Поток, который дочитывает файл лога процесса в буфер. Процесс пишет в файл, а не в pipe,
/// поэтому переживает закрытие приложения, а после перезапуска логи можно читать снова.
/// Работает, пока `done` не выставлен; последний проход делает уже после этого.
pub fn spawn_file_tailer(
    path: PathBuf,
    stream: Stream,
    log: SharedLog,
    done: Arc<AtomicBool>,
    start: u64,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut offset = start;
        let mut skip_head = start > 0;
        loop {
            let finished = done.load(Ordering::SeqCst);
            if let Ok((from, buf)) = read_from(&path, offset) {
                let (consumed, lines) = take_complete_lines(&buf, skip_head);
                if consumed > 0 {
                    skip_head = false;
                }
                offset = from + consumed as u64;
                if !lines.is_empty()
                    && let Ok(mut b) = log.lock()
                {
                    for text in lines {
                        b.push(LogLine { stream, text });
                    }
                }
            }
            if finished {
                break;
            }
            thread::sleep(TAIL_INTERVAL);
        }
    })
}

/// Смещение, с которого начинать чтение файла при присоединении: последние `RESTORE_TAIL_BYTES`.
pub fn tail_start(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len().saturating_sub(RESTORE_TAIL_BYTES)).unwrap_or(0)
}

/// Решает, пора ли отправить батч в UI.
pub fn batch_due(last_flush: Instant, now: Instant) -> bool {
    now.duration_since(last_flush) >= BATCH_INTERVAL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_drops_oldest_when_full() {
        let mut r = RingBuffer::new(3);
        for i in 0..5 {
            r.push(i);
        }
        assert_eq!(r.snapshot(), vec![2, 3, 4]);
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn zero_capacity_is_clamped_to_one() {
        let mut r = RingBuffer::new(0);
        r.push("a");
        r.push("b");
        assert_eq!(r.snapshot(), vec!["b"]);
    }

    #[test]
    fn takes_only_complete_lines_and_keeps_remainder() {
        let (consumed, lines) = take_complete_lines(b"one\ntwo\npart", false);
        assert_eq!(consumed, 8);
        assert_eq!(lines, vec!["one", "two"]);
    }

    #[test]
    fn skips_partial_head_when_reading_from_middle() {
        let (consumed, lines) = take_complete_lines(b"tail of line\nnext\n", true);
        assert_eq!(consumed, 18);
        assert_eq!(lines, vec!["next"]);
    }

    #[test]
    fn no_newline_consumes_nothing() {
        assert_eq!(take_complete_lines(b"still writing", false), (0, Vec::new()));
    }

    #[test]
    fn tailer_reads_lines_written_to_file() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("demerzel-tail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("out.log");
        std::fs::write(&path, "first\nsecond\n").unwrap();
        let log = new_shared(10);
        let done = Arc::new(AtomicBool::new(false));
        let h = spawn_file_tailer(path.clone(), Stream::Stdout, log.clone(), done.clone(), 0);
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(f, "third").unwrap();
        drop(f);
        done.store(true, Ordering::SeqCst);
        h.join().unwrap();
        let texts: Vec<String> = log.lock().unwrap().snapshot().into_iter().map(|l| l.text).collect();
        assert_eq!(texts, vec!["first", "second", "third"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn batch_waits_for_interval() {
        let t0 = Instant::now();
        assert!(!batch_due(t0, t0 + Duration::from_millis(50)));
        assert!(batch_due(t0, t0 + Duration::from_millis(150)));
    }
}
