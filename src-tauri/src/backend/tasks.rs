//! Модель задачи и трейт источника (#16).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Todo,
    InProgress,
    Done,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub project: String,
    pub title: String,
    pub status: TaskStatus,
    pub source: String,
    pub url: Option<String>,
}

/// Источник задач: TODO.md, tasks.json, GitHub Issues (#17, #18).
pub trait TaskSource {
    fn name(&self) -> &str;
    fn fetch(&self, project: &str) -> Result<Vec<Task>, String>;
}

/// Сводка по статусам для дашборда и виджета.
pub fn count_by_status(tasks: &[Task]) -> (usize, usize, usize, usize) {
    tasks.iter().fold((0, 0, 0, 0), |(t, p, d, b), task| match task.status {
        TaskStatus::Todo => (t + 1, p, d, b),
        TaskStatus::InProgress => (t, p + 1, d, b),
        TaskStatus::Done => (t, p, d + 1, b),
        TaskStatus::Blocked => (t, p, d, b + 1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(status: TaskStatus) -> Task {
        Task {
            id: "1".into(),
            project: "demo".into(),
            title: "t".into(),
            status,
            source: "test".into(),
            url: None,
        }
    }

    #[test]
    fn counts_each_status() {
        let tasks = vec![
            task(TaskStatus::Todo),
            task(TaskStatus::InProgress),
            task(TaskStatus::Done),
            task(TaskStatus::Done),
        ];
        assert_eq!(count_by_status(&tasks), (1, 1, 2, 0));
    }
}
