# 终端录屏脚本

长 Prompt 可以直接粘贴。如果录屏终端会拆分多行，先输入 `/paste`，粘贴任务，再用单独一行 `.` 提交。执行期间按 `Esc` 可展示取消能力，session 不会丢失。

1. 安装并验证：

   ```text
   cargo install --path .
   cargo test
   ```

2. 运行 `agent-demo config`。展示配置向导，但不要暴露 API Key；说明 Key 进入操作系统凭据管理器。
3. 无参数启动 `agent-demo`。
4. 输入任务：读取 `Cargo.toml`、计算 `23*19`、记录复核 todo，然后运行测试。批准工具批次并展示 trace。
5. 运行 `/status`、`/sessions` 和 `/trace on`。
6. 使用 `/permission full-access` 在不重启的情况下切换权限。
7. 要求 Agent 创建 `demo.txt`、读取并使用 shell 验证内容，展示完全授权模式不再询问。
8. `/exit` 后重新启动，通过 `/resume` 选择带标题的旧 session。
9. 追问修改内容和未完成 todo，展示对话与结构化状态恢复。
10. 展示单次 JSON 自动化：

    ```text
    agent-demo run --json --permission full-access "使用 calculator 计算 2468*1357"
    ```

录屏中不得展示 API Key、凭据管理器内容、完整环境变量或真实 `.env` 文件。
