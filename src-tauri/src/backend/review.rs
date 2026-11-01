//! Ревью агентом (#153/самосовершенствование): другой агент читает diff и пишет замечания (#review).
//! Ревьюер ничего не меняет в worktree: в промпте прямой запрет, результат — только текст.

use super::agents::AgentRunner;

/// Порядок предпочтения ревьюера: другой CLI, не тот, что написал код.
const REVIEWER_ORDER: &[AgentRunner] = &[AgentRunner::Codex, AgentRunner::Opencode, AgentRunner::Claude, AgentRunner::Gemini];

/// Выбирает ревьюера: первый установленный из порядка предпочтения, не равный автору.
pub fn pick_reviewer(author: AgentRunner, installed: &[AgentRunner]) -> Option<AgentRunner> {
    REVIEWER_ORDER
        .iter()
        .copied()
        .find(|r| *r != author && installed.contains(r))
}

/// Обрезает diff по символам, чтобы промпт не разрастался. Возвращает признак обрезки.
pub fn clip_diff(diff: &str, max_chars: usize) -> (String, bool) {
    if diff.chars().count() <= max_chars {
        return (diff.to_string(), false);
    }
    (diff.chars().take(max_chars).collect(), true)
}

/// Промпт ревьюера на русском. Формат ответа фиксирован, чтобы его было легко читать.
pub fn review_prompt(diff: &str, truncated: bool) -> String {
    let note = if truncated { "\n(diff обрезан: показана только начальная часть)\n" } else { "" };
    format!(
        "Ты ревьюер. Не меняй файлы и не запускай команды, которые что-то меняют.\n\
         Прочитай изменения ниже и ответь строго в формате:\n\
         ## Замечания\n- ... (важность: высокая/средняя/низкая, файл и строка, если видно)\n\
         ## Риски\n- ...\n\
         ## Вердикт\nпринять | доработать — и одна фраза почему.\n{note}\n```diff\n{diff}\n```\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewer_is_never_the_author_and_prefers_codex() {
        let installed = vec![AgentRunner::Claude, AgentRunner::Codex];
        assert_eq!(pick_reviewer(AgentRunner::Claude, &installed), Some(AgentRunner::Codex));
        assert_eq!(pick_reviewer(AgentRunner::Codex, &installed), Some(AgentRunner::Claude));
        assert_eq!(pick_reviewer(AgentRunner::Claude, &[AgentRunner::Claude]), None);
    }

    #[test]
    fn clip_keeps_short_and_marks_long() {
        assert_eq!(clip_diff("abc", 10), ("abc".to_string(), false));
        assert_eq!(clip_diff("абвгд", 3), ("абв".to_string(), true));
    }

    #[test]
    fn prompt_forbids_changes_and_fixes_format() {
        let p = review_prompt("+x", false);
        assert!(p.contains("Не меняй файлы"));
        assert!(p.contains("## Вердикт"));
        assert!(!p.contains("обрезан"));
        assert!(review_prompt("+x", true).contains("обрезан"));
    }
}
