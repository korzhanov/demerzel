//! Менеджер процессов: статусы и переходы (#8, #9).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Stopped,
    Starting,
    Running,
    Exited(i32),
    Crashed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Start,
    Started,
    Stop,
    Exit(i32),
    Signal,
}

/// Чистая функция переходов: не запускает процессы, только считает статус.
pub fn next(status: Status, event: Event) -> Status {
    use Event::*;
    use Status::*;
    match (status, event) {
        (Stopped | Exited(_) | Crashed, Start) => Starting,
        (Starting, Started) => Running,
        (Running | Starting, Stop) => Stopped,
        (Running | Starting, Exit(0)) => Exited(0),
        (Running | Starting, Exit(code)) => Exited(code),
        (Running | Starting, Signal) => Crashed,
        (s, _) => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_then_started_is_running() {
        let s = next(Status::Stopped, Event::Start);
        assert_eq!(s, Status::Starting);
        assert_eq!(next(s, Event::Started), Status::Running);
    }

    #[test]
    fn nonzero_exit_is_recorded_with_code() {
        assert_eq!(next(Status::Running, Event::Exit(1)), Status::Exited(1));
    }

    #[test]
    fn stop_from_running_returns_to_stopped() {
        assert_eq!(next(Status::Running, Event::Stop), Status::Stopped);
    }
}
