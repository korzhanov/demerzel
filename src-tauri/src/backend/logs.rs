//! Логи процессов: кольцевой буфер и батчинг для UI (#9).
//!
//! Чтение stdout и stderr идёт в отдельных потоках, иначе дочерний процесс
//! зависнет, когда забьётся pipe. Буфер ограничен, старые строки вытесняются.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const DEFAULT_CAPACITY: usize = 5000;
/// Интервал батча для событий в UI.
pub const BATCH_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Запускает поток, который читает строки из потока и кладёт их в буфер.
pub fn spawn_reader<R: Read + Send + 'static>(reader: R, stream: Stream, log: SharedLog) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            let Ok(text) = line else { break };
            if let Ok(mut buf) = log.lock() {
                buf.push(LogLine { stream, text });
            }
        }
    })
}

/// Решает, пора ли отправить батч в UI.
pub fn batch_due(last_flush: Instant, now: Instant) -> bool {
    now.duration_since(last_flush) >= BATCH_INTERVAL
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

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
    fn reader_collects_lines_from_both_streams() {
        let log = new_shared(10);
        let h1 = spawn_reader(Cursor::new("one\ntwo\n"), Stream::Stdout, log.clone());
        let h2 = spawn_reader(Cursor::new("oops\n"), Stream::Stderr, log.clone());
        h1.join().unwrap();
        h2.join().unwrap();
        let lines = log.lock().unwrap().snapshot();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines.iter().filter(|l| l.stream == Stream::Stderr).count(), 1);
    }

    #[test]
    fn batch_waits_for_interval() {
        let t0 = Instant::now();
        assert!(!batch_due(t0, t0 + Duration::from_millis(50)));
        assert!(batch_due(t0, t0 + Duration::from_millis(150)));
    }
}
