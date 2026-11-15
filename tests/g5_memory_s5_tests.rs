use nexus_albaai::domain::{ChatMessage, HistoryLimits, MessageRole, ToolCall};
use nexus_albaai::memory::compaction::{compact_conversation_history, verify_tool_call_integrity};
use nexus_albaai::models::{ModelClient, ModelError, ModelRequest, ModelResponse};

struct MockSummarizerClient {
    expected_summary: String,
}

impl ModelClient for MockSummarizerClient {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        Ok(ModelResponse {
            content: Some(self.expected_summary.clone()),
            tool_calls: Vec::new(),
            usage: None,
        })
    }
}

#[tokio::test]
async fn test_conversation_compaction_with_mock_model_client() {
    let mock_client = MockSummarizerClient {
        expected_summary: "User checked disk status and reviewed server logs.".to_string(),
    };

    let messages = vec![
        ChatMessage::system("You are an autonomous engineering assistant."),
        ChatMessage::user("Check disk space on /var/log"),
        ChatMessage {
            role: MessageRole::Assistant,
            content: "Running df -h".to_string(),
            tool_calls: Some(vec![ToolCall {
                id: "call_df_1".to_string(),
                name: "run_command".to_string(),
                arguments: serde_json::json!({"command": "df -h /var/log"}),
            }]),
            tool_call_id: None,
        },
        ChatMessage {
            role: MessageRole::Tool,
            content:
                "Filesystem Size Used Avail Use% Mounted on\n/dev/sda1 50G 45G 5G 90% /var/log"
                    .to_string(),
            tool_calls: None,
            tool_call_id: Some("call_df_1".to_string()),
        },
        ChatMessage::assistant("Disk usage is at 90%."),
        ChatMessage::user("Check syslog error count"),
        ChatMessage::assistant("Found 3 critical errors in syslog."),
    ];

    let limits = HistoryLimits {
        max_messages: 5,
        max_bytes: 100_000,
    };

    let (compacted, trace) =
        compact_conversation_history(&messages, &limits, 2, Some(&mock_client)).await;

    assert!(trace.is_some());
    let tr = trace.unwrap();
    assert_eq!(tr.start_index, 1);
    assert!(tr.end_index > tr.start_index);

    assert_eq!(compacted[0].role, MessageRole::System);
    assert_eq!(
        compacted[0].content,
        "You are an autonomous engineering assistant."
    );

    assert_eq!(compacted[1].role, MessageRole::System);
    assert!(compacted[1].content.contains("[COMPACTED_SUMMARY"));
    assert!(compacted[1]
        .content
        .contains("User checked disk status and reviewed server logs."));

    let tail_msg_count = compacted.len() - 2;
    assert!(tail_msg_count >= 2);

    assert!(verify_tool_call_integrity(&compacted).is_ok());
}

#[tokio::test]
async fn test_tool_call_family_is_never_split() {
    let messages = vec![
        ChatMessage::system("System instructions"),
        ChatMessage::user("Step 1"),
        ChatMessage::assistant("Step 1 done"),
        ChatMessage::user("Step 2"),
        ChatMessage {
            role: MessageRole::Assistant,
            content: "Calling dual tools".to_string(),
            tool_calls: Some(vec![
                ToolCall {
                    id: "call_a".to_string(),
                    name: "tool_a".to_string(),
                    arguments: serde_json::json!({}),
                },
                ToolCall {
                    id: "call_b".to_string(),
                    name: "tool_b".to_string(),
                    arguments: serde_json::json!({}),
                },
            ]),
            tool_call_id: None,
        },
        ChatMessage {
            role: MessageRole::Tool,
            content: "result a".to_string(),
            tool_calls: None,
            tool_call_id: Some("call_a".to_string()),
        },
        ChatMessage {
            role: MessageRole::Tool,
            content: "result b".to_string(),
            tool_calls: None,
            tool_call_id: Some("call_b".to_string()),
        },
        ChatMessage::assistant("All tools completed successfully"),
    ];

    let limits = HistoryLimits {
        max_messages: 5,
        max_bytes: 100_000,
    };

    let (compacted, trace) =
        compact_conversation_history::<MockSummarizerClient>(&messages, &limits, 4, None).await;

    assert!(trace.is_some());
    assert!(verify_tool_call_integrity(&compacted).is_ok());

    let has_call_a = compacted.iter().any(|m| {
        m.tool_calls
            .as_ref()
            .is_some_and(|calls| calls.iter().any(|c| c.id == "call_a"))
    });
    let has_res_a = compacted
        .iter()
        .any(|m| m.tool_call_id.as_deref() == Some("call_a"));
    let has_res_b = compacted
        .iter()
        .any(|m| m.tool_call_id.as_deref() == Some("call_b"));

    assert_eq!(has_call_a, has_res_a);
    assert_eq!(has_call_a, has_res_b);
}

#[tokio::test]
async fn test_property_1000_random_histories_zero_orphaned_messages_and_traceability() {
    let mut rng_seed = 0x12345678_u64;
    let mut next_u32 = || -> u32 {
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (rng_seed >> 32) as u32
    };

    for iteration in 0..1000 {
        let system_count = (next_u32() % 3) as usize;
        let mut history = Vec::new();
        for s in 0..system_count {
            history.push(ChatMessage::system(format!("System policy {}", s)));
        }

        let num_turns = 3 + (next_u32() % 12) as usize;
        let mut call_counter = 0usize;

        for turn in 0..num_turns {
            let roll = next_u32() % 10;
            if roll < 3 {
                history.push(ChatMessage::user(format!(
                    "User query iter {} turn {}",
                    iteration, turn
                )));
            } else if roll < 6 {
                history.push(ChatMessage::assistant(format!(
                    "Assistant direct answer {}",
                    turn
                )));
            } else {
                call_counter += 1;
                let id1 = format!("call_{}_{}_1", iteration, call_counter);
                let id2 = format!("call_{}_{}_2", iteration, call_counter);

                let calls = if roll == 6 {
                    vec![ToolCall {
                        id: id1.clone(),
                        name: "exec_probe".to_string(),
                        arguments: serde_json::json!({"turn": turn}),
                    }]
                } else {
                    vec![
                        ToolCall {
                            id: id1.clone(),
                            name: "read_probe".to_string(),
                            arguments: serde_json::json!({"p": 1}),
                        },
                        ToolCall {
                            id: id2.clone(),
                            name: "write_probe".to_string(),
                            arguments: serde_json::json!({"p": 2}),
                        },
                    ]
                };

                history.push(ChatMessage {
                    role: MessageRole::Assistant,
                    content: format!("Invoking {} tool(s)", calls.len()),
                    tool_calls: Some(calls),
                    tool_call_id: None,
                });

                history.push(ChatMessage {
                    role: MessageRole::Tool,
                    content: format!("output of {}", id1),
                    tool_calls: None,
                    tool_call_id: Some(id1),
                });

                if roll != 6 {
                    history.push(ChatMessage {
                        role: MessageRole::Tool,
                        content: format!("output of {}", id2),
                        tool_calls: None,
                        tool_call_id: Some(id2),
                    });
                }
            }
        }

        let max_messages = 4 + (next_u32() % 8) as usize;
        let limits = HistoryLimits {
            max_messages,
            max_bytes: 200_000,
        };
        let recent_n = 1 + (next_u32() % 4) as usize;

        let (compacted, trace) =
            compact_conversation_history::<MockSummarizerClient>(&history, &limits, recent_n, None)
                .await;

        let integrity_check = verify_tool_call_integrity(&compacted);
        assert!(
            integrity_check.is_ok(),
            "Iteration {} failed tool call integrity: {:?}",
            iteration,
            integrity_check.err()
        );

        let out_system_count = compacted
            .iter()
            .take_while(|m| {
                m.role == MessageRole::System && !m.content.starts_with("[COMPACTED_SUMMARY")
            })
            .count();
        assert_eq!(
            out_system_count, system_count,
            "Iteration {}: Leading system count changed from {} to {}",
            iteration, system_count, out_system_count
        );

        if let Some(tr) = trace {
            assert!(tr.start_index >= system_count);
            assert!(tr.end_index > tr.start_index);
            assert_eq!(tr.message_count, tr.end_index - tr.start_index);
        }

        let (idempotent_result, idempotent_trace) = compact_conversation_history::<
            MockSummarizerClient,
        >(&compacted, &limits, recent_n, None)
        .await;

        assert_eq!(
            compacted.len(),
            idempotent_result.len(),
            "Iteration {}: Compaction must be idempotent, length changed on rerun",
            iteration
        );
        assert!(
            idempotent_trace.is_none(),
            "Iteration {}: Re-compaction should be a no-op when already within limits",
            iteration
        );
    }
}
