use crate::{memory::StoredMessage, model::ChatMessage};

pub fn estimate_tokens(messages: &[ChatMessage]) -> usize {
    messages
        .iter()
        .map(|message| {
            serde_json::to_string(message)
                .map(|s| s.chars().count())
                .unwrap_or(0)
        })
        .sum()
}

pub fn build_context(
    system_prompt: &str,
    summary: &str,
    stored: &[StoredMessage],
) -> Vec<ChatMessage> {
    let mut messages = vec![ChatMessage::system(system_prompt)];
    if !summary.is_empty() {
        messages.push(ChatMessage::system(format!(
            "Session memory summary:\n{summary}"
        )));
    }
    messages.extend(stored.iter().map(|item| item.message.clone()));
    messages
}

pub fn split_complete_turns(
    stored: &[StoredMessage],
    keep_turns: usize,
) -> Option<(&[StoredMessage], &[StoredMessage])> {
    let user_indices: Vec<usize> = stored
        .iter()
        .enumerate()
        .filter_map(|(index, item)| (item.message.role == "user").then_some(index))
        .collect();
    if user_indices.len() <= keep_turns {
        return None;
    }
    let cut = user_indices[user_indices.len() - keep_turns];
    Some((&stored[..cut], &stored[cut..]))
}
