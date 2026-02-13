use openjax_core::Agent;
use openjax_protocol::{Event, Op};
use std::io::{self, Write};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut agent = Agent::new();

    println!(
        "OpenJax CLI 已启动（model backend: {}，approval: {}，sandbox: {}）。输入内容开始对话，输入 /exit 退出。",
        agent.model_backend_name(),
        agent.approval_policy_name(),
        agent.sandbox_mode_name(),
    );
    println!(
        "可用工具示例: tool:list_dir path=. | tool:read_file path=docs/todo.md | tool:grep_files pattern=OpenJax path=."
    );
    println!(
        "命令执行示例: tool:exec_command cmd='ls -la' | tool:exec_command cmd='curl https://example.com' require_escalated=true"
    );
    println!(
        "补丁示例: tool:apply_patch patch='*** Begin Patch\\n*** Add File: hello.txt\\n+hello\\n*** End Patch'"
    );

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        let bytes = io::stdin().read_line(&mut input)?;
        if bytes == 0 {
            break;
        }

        let input = input.trim().to_string();
        if input.is_empty() {
            continue;
        }

        if input == "/exit" {
            for event in agent.submit(Op::Shutdown).await {
                print_event(&event);
            }
            break;
        }

        for event in agent.submit(Op::UserTurn { input }).await {
            print_event(&event);
        }
    }

    Ok(())
}

fn print_event(event: &Event) {
    match event {
        Event::TurnStarted { turn_id } => println!("[turn:{turn_id}] started"),
        Event::ToolCallStarted { turn_id, tool_name } => {
            println!("[turn:{turn_id}] tool start: {tool_name}")
        }
        Event::ToolCallCompleted {
            turn_id,
            tool_name,
            ok,
            output,
        } => {
            println!("[turn:{turn_id}] tool done: {tool_name} ok={ok}");
            println!("{output}");
        }
        Event::AssistantMessage { turn_id, content } => {
            println!("[turn:{turn_id}] assistant: {content}")
        }
        Event::TurnCompleted { turn_id } => println!("[turn:{turn_id}] completed"),
        Event::ShutdownComplete => println!("shutdown complete"),
        // Multi-agent events (预留扩展)
        Event::AgentSpawned { parent_thread_id, new_thread_id } => {
            println!("[agent] spawned: parent={parent_thread_id:?} new={new_thread_id:?}")
        }
        Event::AgentStatusChanged { thread_id, status } => {
            println!("[agent:{thread_id:?}] status: {status:?}")
        }
    }
}
