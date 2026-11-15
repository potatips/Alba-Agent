use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::domain::{ChatMessage, HistoryLimits, MessageRole};
use crate::models::{ModelClient, ModelRequest};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionTrace {
    pub start_index: usize,
    pub end_index: usize,
    pub message_count: usize,
    pub tool_calls_count: usize,
    pub summary_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageGroup {
    pub messages: Vec<ChatMessage>,
    pub bytes: usize,
    pub is_tool_family: bool,
}

pub fn group_messages(messages: &[ChatMessage]) -> Vec<MessageGroup> {
    let mut groups = Vec::new();
    let mut pos = 0usize;

    while pos < messages.len() {
        let msg = &messages[pos];
        if msg.role == MessageRole::Assistant {
            if let Some(ref calls) = msg.tool_calls {
                if !calls.is_empty() {
                    let expected_ids: Vec<&str> = calls.iter().map(|c| c.id.as_str()).collect();
                    let mut family_msgs = vec![msg.clone()];
                    let mut bytes = msg.estimated_bytes();
                    pos += 1;
                    while pos < messages.len() && messages[pos].role == MessageRole::Tool {
                        let belongs = messages[pos]
                            .tool_call_id
                            .as_deref()
                            .is_some_and(|id| expected_ids.contains(&id));
                        if belongs {
                            bytes += messages[pos].estimated_bytes();
                            family_msgs.push(messages[pos].clone());
                            pos += 1;
                        } else {
                            break;
                        }
                    }
                    groups.push(MessageGroup {
                        messages: family_msgs,
                        bytes,
                        is_tool_family: true,
                    });
                    continue;
                }
            }
        }

        groups.push(MessageGroup {
            messages: vec![msg.clone()],
            bytes: msg.estimated_bytes(),
            is_tool_family: false,
        });
        pos += 1;
    }

    groups
}

pub fn format_deterministic_summary(
    start_index: usize,
    end_index: usize,
    messages: &[ChatMessage],
) -> (String, CompactionTrace) {
    let mut user_turns = 0usize;
    let mut assistant_turns = 0usize;
    let mut tool_turns = 0usize;
    let mut tool_names = Vec::new();
    let mut highlights = Vec::new();

    for msg in messages {
        match msg.role {
            MessageRole::System => {}
            MessageRole::User => {
                user_turns += 1;
                if highlights.len() < 3 && !msg.content.trim().is_empty() {
                    highlights.push(format!("User requested: {}", msg.content.trim()));
                }
            }
            MessageRole::Assistant => {
                assistant_turns += 1;
                if let Some(ref calls) = msg.tool_calls {
                    for call in calls {
                        if !tool_names.contains(&call.name) {
                            tool_names.push(call.name.clone());
                        }
                    }
                }
            }
            MessageRole::Tool => {
                tool_turns += 1;
            }
        }
    }

    let summary_body = format!(
        "Prior interaction summary ({} messages: {} user, {} assistant, {} tool calls [{}]); Key points: {}",
        messages.len(),
        user_turns,
        assistant_turns,
        tool_turns,
        tool_names.join(", "),
        if highlights.is_empty() {
            "routine task execution".to_string()
        } else {
            highlights.join("; ")
        }
    );

    let mut hasher = Sha256::new();
    hasher.update(summary_body.as_bytes());
    let summary_hash = format!("{:x}", hasher.finalize());

    let header = format!(
        "[COMPACTED_SUMMARY range={}..{} count={} hash={}]\n{}",
        start_index,
        end_index,
        messages.len(),
        &summary_hash[..8],
        summary_body
    );

    let trace = CompactionTrace {
        start_index,
        end_index,
        message_count: messages.len(),
        tool_calls_count: tool_turns,
        summary_hash,
    };

    (header, trace)
}

pub async fn compact_conversation_history<M: ModelClient>(
    messages: &[ChatMessage],
    limits: &HistoryLimits,
    recent_messages_to_keep: usize,
    model_client: Option<&M>,
) -> (Vec<ChatMessage>, Option<CompactionTrace>) {
    if messages.is_empty() {
        return (Vec::new(), None);
    }

    let mut leading_system = Vec::new();
    let mut start_idx = 0usize;
    while start_idx < messages.len() && messages[start_idx].role == MessageRole::System {
        leading_system.push(messages[start_idx].clone());
        start_idx += 1;
    }

    let rest = &messages[start_idx..];
    if rest.is_empty() {
        return (leading_system, None);
    }

    let total_messages = messages.len();
    let total_bytes: usize = messages.iter().map(ChatMessage::estimated_bytes).sum();

    if total_messages <= limits.max_messages && total_bytes <= limits.max_bytes {
        return (messages.to_vec(), None);
    }

    let groups = group_messages(rest);
    if groups.is_empty() {
        return (leading_system, None);
    }

    let min_keep_count = recent_messages_to_keep.max(1);
    let mut tail_groups = Vec::new();
    let mut tail_message_count = 0usize;

    let mut split_group_idx = groups.len();
    while split_group_idx > 0 {
        let next_group = &groups[split_group_idx - 1];
        if tail_message_count < min_keep_count || split_group_idx == groups.len() {
            tail_message_count += next_group.messages.len();
            tail_groups.push(next_group.clone());
            split_group_idx -= 1;
        } else {
            break;
        }
    }
    tail_groups.reverse();

    if split_group_idx == 0 {
        return (messages.to_vec(), None);
    }

    let mut to_compact_msgs = Vec::new();
    for g in &groups[..split_group_idx] {
        to_compact_msgs.extend(g.messages.clone());
    }

    if to_compact_msgs.is_empty() {
        return (messages.to_vec(), None);
    }

    let start_source_index = start_idx;
    let end_source_index = start_idx + to_compact_msgs.len();

    let (summary_content, trace) = if let Some(client) = model_client {
        let prompt = format!(
            "Summarize the following {} conversation messages into a concise summary:\n{:?}",
            to_compact_msgs.len(),
            to_compact_msgs
        );
        let req = ModelRequest {
            messages: vec![ChatMessage::user(prompt)],
            tools: Vec::new(),
            temperature: Some(0.0),
            max_tokens: Some(512),
            model_override: None,
        };
        match client.complete(&req).await {
            Ok(resp) => {
                let text = resp
                    .content
                    .unwrap_or_else(|| "task in progress".to_string());
                let mut hasher = Sha256::new();
                hasher.update(text.as_bytes());
                let summary_hash = format!("{:x}", hasher.finalize());
                let header = format!(
                    "[COMPACTED_SUMMARY range={}..{} count={} hash={}]\n{}",
                    start_source_index,
                    end_source_index,
                    to_compact_msgs.len(),
                    &summary_hash[..8],
                    text
                );
                let tr = CompactionTrace {
                    start_index: start_source_index,
                    end_index: end_source_index,
                    message_count: to_compact_msgs.len(),
                    tool_calls_count: to_compact_msgs
                        .iter()
                        .filter(|m| m.role == MessageRole::Tool)
                        .count(),
                    summary_hash,
                };
                (header, tr)
            }
            Err(_) => {
                format_deterministic_summary(start_source_index, end_source_index, &to_compact_msgs)
            }
        }
    } else {
        format_deterministic_summary(start_source_index, end_source_index, &to_compact_msgs)
    };

    let summary_message = ChatMessage {
        role: MessageRole::System,
        content: crate::memory::firewall::RedactionEngine::default().redact(&summary_content, &[]),
        tool_calls: None,
        tool_call_id: None,
    };

    let mut compacted_result = leading_system;
    compacted_result.push(summary_message);
    for group in tail_groups {
        compacted_result.extend(group.messages);
    }

    (compacted_result, Some(trace))
}

pub fn verify_tool_call_integrity(messages: &[ChatMessage]) -> Result<(), String> {
    let mut declared_calls = std::collections::HashMap::<String, String>::new();
    let mut answered_calls = std::collections::HashSet::<String>::new();

    for (idx, msg) in messages.iter().enumerate() {
        if msg.role == MessageRole::Assistant {
            if let Some(ref calls) = msg.tool_calls {
                for c in calls {
                    declared_calls.insert(c.id.clone(), format!("msg_{}", idx));
                }
            }
        } else if msg.role == MessageRole::Tool {
            if let Some(ref tid) = msg.tool_call_id {
                if !declared_calls.contains_key(tid) {
                    return Err(format!(
                        "Orphaned tool message at index {} referencing undeclared tool_call_id: {}",
                        idx, tid
                    ));
                }
                answered_calls.insert(tid.clone());
            } else {
                return Err(format!(
                    "Tool message at index {} missing tool_call_id",
                    idx
                ));
            }
        }
    }

    Ok(())
}
