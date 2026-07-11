use std::sync::Arc;

use mini_coding_agent::{Memory, SessionKey, ToolRegistry, model::ToolOutput, tools::ToolContext};
use serde_json::json;

fn context(temp: &tempfile::TempDir) -> ToolContext {
    ToolContext {
        session: SessionKey::new("u", "s"),
        workspace: temp.path().to_path_buf(),
        memory: Arc::new(Memory::open(temp.path().join("agent.db")).unwrap()),
    }
}

fn assert_ok(output: &ToolOutput) {
    assert!(output.ok, "tool failed: {:?}", output.error);
}

#[tokio::test]
async fn calculator_evaluates_expression() {
    let temp = tempfile::tempdir().unwrap();
    let registry = ToolRegistry::standard();
    let output = registry
        .get("calculator")
        .unwrap()
        .execute(&context(&temp), json!({"expression":"2+3*4"}))
        .await;
    assert_ok(&output);
    assert_eq!(output.data.unwrap()["result"], 14.0);
}

#[tokio::test]
async fn edit_create_replace_and_read() {
    let temp = tempfile::tempdir().unwrap();
    let registry = ToolRegistry::standard();
    let ctx = context(&temp);
    let edit = registry.get("edit_file").unwrap();
    assert_ok(
        &edit
            .execute(
                &ctx,
                json!({"path":"a.txt","old_text":"","new_text":"one\ntwo"}),
            )
            .await,
    );
    assert_ok(
        &edit
            .execute(
                &ctx,
                json!({"path":"a.txt","old_text":"two","new_text":"three"}),
            )
            .await,
    );
    let read = registry
        .get("read_file")
        .unwrap()
        .execute(&ctx, json!({"path":"a.txt","offset":1,"limit":20}))
        .await;
    assert_ok(&read);
    assert!(
        read.data.unwrap()["content"]
            .as_str()
            .unwrap()
            .contains("three")
    );
}

#[tokio::test]
async fn edit_rejects_ambiguous_match() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("a.txt"), "x x").unwrap();
    let output = ToolRegistry::standard()
        .get("edit_file")
        .unwrap()
        .execute(
            &context(&temp),
            json!({"path":"a.txt","old_text":"x","new_text":"y"}),
        )
        .await;
    assert!(!output.ok);
    assert_eq!(
        std::fs::read_to_string(temp.path().join("a.txt")).unwrap(),
        "x x"
    );
}

#[tokio::test]
async fn shell_reports_stdout_and_exit_code() {
    let temp = tempfile::tempdir().unwrap();
    let command = if cfg!(windows) {
        "Write-Output hello"
    } else {
        "printf hello"
    };
    let output = ToolRegistry::standard()
        .get("shell")
        .unwrap()
        .execute(
            &context(&temp),
            json!({"command":command,"timeout_ms":5000}),
        )
        .await;
    assert_ok(&output);
    let data = output.data.unwrap();
    assert_eq!(data["exit_code"], 0);
    assert!(data["stdout"].as_str().unwrap().contains("hello"));
}

#[tokio::test]
async fn shell_does_not_inherit_deepseek_key() {
    unsafe { std::env::set_var("DEEPSEEK_API_KEY", "should-not-leak") };
    let temp = tempfile::tempdir().unwrap();
    let command = if cfg!(windows) {
        "$env:DEEPSEEK_API_KEY"
    } else {
        "printf %s \"$DEEPSEEK_API_KEY\""
    };
    let output = ToolRegistry::standard()
        .get("shell")
        .unwrap()
        .execute(
            &context(&temp),
            json!({"command":command,"timeout_ms":5000}),
        )
        .await;
    let stdout = output.data.unwrap()["stdout"].as_str().unwrap().to_string();
    assert!(!stdout.contains("should-not-leak"));
    unsafe { std::env::remove_var("DEEPSEEK_API_KEY") };
}

#[tokio::test]
async fn shell_times_out() {
    let temp = tempfile::tempdir().unwrap();
    let command = if cfg!(windows) {
        "Start-Sleep -Seconds 2"
    } else {
        "sleep 2"
    };
    let output = ToolRegistry::standard()
        .get("shell")
        .unwrap()
        .execute(&context(&temp), json!({"command":command,"timeout_ms":100}))
        .await;
    assert!(!output.ok);
    assert_eq!(output.error.unwrap().code, "timeout");
}

#[tokio::test]
async fn shell_nonzero_exit_is_an_observation() {
    let temp = tempfile::tempdir().unwrap();
    let command = "exit 7";
    let output = ToolRegistry::standard()
        .get("shell")
        .unwrap()
        .execute(
            &context(&temp),
            json!({"command":command,"timeout_ms":5000}),
        )
        .await;
    assert_ok(&output);
    let data = output.data.unwrap();
    assert_eq!(data["exit_code"], 7);
    assert_eq!(data["success"], false);
}

#[tokio::test]
async fn read_file_supports_pagination() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("lines.txt"), "one\ntwo\nthree\n").unwrap();
    let output = ToolRegistry::standard()
        .get("read_file")
        .unwrap()
        .execute(
            &context(&temp),
            json!({"path":"lines.txt","offset":2,"limit":1}),
        )
        .await;
    assert_ok(&output);
    let data = output.data.unwrap();
    assert!(data["content"].as_str().unwrap().contains("two"));
    assert_eq!(data["next_offset"], 3);
}

#[tokio::test]
async fn read_file_rejects_binary_content() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("binary.bin"), [1_u8, 0, 2]).unwrap();
    let output = ToolRegistry::standard()
        .get("read_file")
        .unwrap()
        .execute(
            &context(&temp),
            json!({"path":"binary.bin","offset":1,"limit":10}),
        )
        .await;
    assert!(!output.ok);
    assert_eq!(output.error.unwrap().code, "binary_file");
}

#[tokio::test]
async fn schema_validation_rejects_unknown_fields() {
    let registry = ToolRegistry::standard();
    let error = registry
        .get("calculator")
        .unwrap()
        .validate(&json!({"expression":"1+1","extra":true}))
        .unwrap_err();
    assert!(error.contains("unknown field"));
}
