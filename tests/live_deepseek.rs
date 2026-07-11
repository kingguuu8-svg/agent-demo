use std::sync::Arc;

use async_trait::async_trait;
use mini_coding_agent::{
    Agent, AgentConfig, Approver, DeepSeekClient, Memory, PermissionMode, SessionKey, ToolRegistry,
    tools::PreparedCall,
};

struct Allow;
#[async_trait]
impl Approver for Allow {
    async fn approve(&self, _: &[PreparedCall]) -> bool {
        true
    }
}

#[tokio::test]
#[ignore = "uses the real DeepSeek API"]
async fn live_model_calls_calculator_and_finishes() {
    if std::env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("DEEPSEEK_API_KEY is not set; skipping live smoke test");
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(
        Arc::new(DeepSeekClient::from_env().unwrap()),
        Arc::new(Memory::open(temp.path().join("agent.db")).unwrap()),
        ToolRegistry::standard(),
        Arc::new(Allow),
        PermissionMode::FullAccess,
        temp.path().to_path_buf(),
        AgentConfig::default(),
    );
    let reply = agent
        .run(
            SessionKey::new("live", "calculator"),
            "Use the calculator tool to compute 2468 * 1357, then answer with the result.",
        )
        .await
        .unwrap();
    assert!(!reply.content.trim().is_empty());
    assert!(reply.tool_calls >= 1);
}
