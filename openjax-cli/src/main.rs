use clap::Parser;
use openjax_core::Agent;
use openjax_protocol::{Event, Op};
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "openjax")]
#[command(version = "0.1.0")]
#[command(about = "OpenJax - A CLI agent framework", long_about = None)]
struct Cli {
    /// 模型后端: minimax | openai | echo (also via OPENJAX_MODEL env var)
    #[arg(long)]
    model: Option<String>,

    /// 审批策略: always_ask | on_request | never (also via OPENJAX_APPROVAL_POLICY env var)
    #[arg(long)]
    approval: Option<String>,

    /// 沙箱模式: workspace_write | danger_full_access (also via OPENJAX_SANDBOX_MODE env var)
    #[arg(long)]
    sandbox: Option<String>,

    /// 配置文件路径
    #[arg(long)]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Show config info (CLI args will be applied via environment variables by clap)
    if cli.model.is_some() {
        println!("[cli] model: {:?}", cli.model);
    }
    if cli.approval.is_some() {
        println!("[cli] approval: {:?}", cli.approval);
    }
    if cli.sandbox.is_some() {
        println!("[cli] sandbox: {:?}", cli.sandbox);
    }

    if let Some(config_path) = &cli.config {
        println!("[cli] config: {}", config_path.display());
        println!("[config] Note: config file loading will be implemented in Phase 2");
    }

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
