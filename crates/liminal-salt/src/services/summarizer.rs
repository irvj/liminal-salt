//! Title generation — one-shot LLM call to turn the first exchange into a 2–5
//! word session title. Ports `chat/services/summarizer.py` semantics faithfully.

use crate::services::llm::{ChatLlm, LlmMessage};
use crate::services::session::Role;

const ARTIFACTS: &[&str] = &[
    "<s>",
    "</s>",
    "[INST]",
    "[/INST]",
    "<<SYS>>",
    "<</SYS>>",
    "###",
    "Prompt",
];
const BAD_PATTERNS: &[&str] = &["[", "]", "<", ">", "#", "\n", "Prompt", "INST", "SYS"];

/// Generate a 2–5 word title from the first user + assistant exchange. Falls
/// back to a truncated user prompt if the LLM response is missing, malformed,
/// or contains model artifacts.
pub async fn generate_title<L: ChatLlm>(
    llm: &L,
    user_prompt: &str,
    assistant_response: &str,
) -> String {
    if user_prompt.is_empty() {
        return "New Chat".to_string();
    }

    let use_response = !assistant_response.is_empty()
        && !assistant_response.starts_with("ERROR:")
        && !assistant_response.trim().is_empty();

    let prompt = if use_response {
        format!(
            "Generate a very short, 2-5 word title for a chat session.\n\
            Rules:\n\
            - NO quotes, punctuation, or special characters\n\
            - NO model tokens like [INST], </s>, <s>\n\
            - Just the plain title text\n\
            - Be descriptive but concise\n\n\
            USER ASKED: {}\n\
            ASSISTANT REPLIED: {}\n\n\
            TITLE:",
            truncate(user_prompt, 200),
            truncate(assistant_response, 200),
        )
    } else {
        format!(
            "Generate a very short, 2-5 word title that captures the essence of this question.\n\
            Rules:\n\
            - NO quotes, punctuation, or special characters\n\
            - NO model tokens\n\
            - Just the plain title text\n\n\
            USER QUESTION: {}\n\n\
            TITLE:",
            truncate(user_prompt, 200),
        )
    };

    let messages = [LlmMessage::new(Role::User, prompt)];
    let raw = match llm.complete(&messages).await {
        Ok(t) => t,
        Err(_) => return fallback(user_prompt),
    };

    let cleaned = clean_title(&raw);
    if cleaned.is_empty()
        || cleaned.len() < 3
        || cleaned.len() > 50
        || has_artifacts(&cleaned)
    {
        return fallback(user_prompt);
    }
    cleaned
}

fn truncate(s: &str, limit: usize) -> &str {
    // Byte-index truncation that never splits a multi-byte char.
    if s.len() <= limit {
        return s;
    }
    let mut idx = limit;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    &s[..idx]
}

fn clean_title(title: &str) -> String {
    let mut t = title.to_string();
    for artifact in ARTIFACTS {
        t = t.replace(artifact, "");
    }
    t = t.replace(['"', '\''], "");
    let t = t.trim().trim_end_matches([':', ';', '.', ',', '!', '?']);
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn has_artifacts(title: &str) -> bool {
    BAD_PATTERNS.iter().any(|p| title.contains(p))
}

fn fallback(user_prompt: &str) -> String {
    if user_prompt.len() <= 50 {
        user_prompt.to_string()
    } else {
        format!("{}...", truncate(user_prompt, 50))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_title_strips_artifacts_and_punctuation() {
        assert_eq!(clean_title("<s>Hello World</s>"), "Hello World");
        assert_eq!(clean_title("\"Quoted Title\":"), "Quoted Title");
        assert_eq!(clean_title("  weird   whitespace  "), "weird whitespace");
    }

    #[test]
    fn has_artifacts_detects_brackets_and_tokens() {
        assert!(has_artifacts("[INST] bad"));
        assert!(has_artifacts("has #hash"));
        assert!(has_artifacts("has\nnewline"));
        assert!(!has_artifacts("Clean Title"));
    }

    #[test]
    fn fallback_truncates_long_prompts() {
        let long = "a".repeat(60);
        let f = fallback(&long);
        assert!(f.ends_with("..."));
        assert_eq!(f.len(), 53); // 50 + "..."
    }

    #[test]
    fn truncate_preserves_char_boundaries() {
        // "héllo" — 'é' is 2 bytes; cutting at 2 would split it.
        assert_eq!(truncate("héllo", 2), "h");
    }
}
