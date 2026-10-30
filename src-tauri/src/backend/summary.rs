//! Экспорт summary.json для виджета (#24).

pub const SUMMARY_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_format_is_versioned() {
        assert_eq!(SUMMARY_VERSION, 1);
    }
}
