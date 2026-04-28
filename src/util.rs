/// Truncate text to a maximum character count, appending "..." if truncated.
pub fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        text.to_string()
    } else {
        format!("{}...", text.chars().take(max).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_shorter_than_max() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_equal_to_max() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_longer_than_max() {
        let result = truncate("hello world", 5);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn test_truncate_empty_string() {
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn test_truncate_max_zero() {
        let result = truncate("hello", 0);
        assert_eq!(result, "...");
    }

    #[test]
    fn test_truncate_multibyte_chars() {
        // "こんにちは" is 5 chars but 15 bytes in UTF-8
        // truncate uses .len() which is bytes, so 5 bytes cuts into multibyte
        let result = truncate("こんにちは世界", 5);
        assert_eq!(result, "こんにちは...");
    }

    #[test]
    fn test_truncate_multibyte_exact() {
        // "こんにちは" is 15 bytes, 5 bytes would truncate
        let result = truncate("こんにちは", 5);
        assert_eq!(result, "こんにちは...");
    }

    #[test]
    fn test_truncate_multibyte_sufficient_max() {
        // 15 bytes in "こんにちは", so max=15 keeps it intact
        let result = truncate("こんにちは", 15);
        assert_eq!(result, "こんにちは");
    }
}
