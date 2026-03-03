use clap::Parser;
use openjax_core::{Agent, Config, init_logger};
use openjax_protocol::{Event, Op};
use rustyline::Editor;
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
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

    /// 配置文件路径 (默认自动查找 ./.openjax/config/config.toml 或 ~/.openjax/config.toml)
    #[arg(long)]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logger();

    let cli = Cli::parse();

    let config = if let Some(config_path) = &cli.config {
        match Config::from_file(config_path) {
            Ok(c) => {
                println!("[config] loaded from: {}", config_path.display());
                c
            }
            Err(e) => {
                eprintln!("[config] failed to load {}: {}", config_path.display(), e);
                std::process::exit(1);
            }
        }
    } else {
        if let Some(path) = Config::find_or_create_config_file() {
            println!("[config] loaded from: {}", path.display());
            Config::load()
        } else {
            eprintln!("[config] failed to discover or create default config file");
            Config::load()
        }
    };

    if cli.model.is_some() {
        println!("[cli] model: {:?}", cli.model);
    }
    if cli.approval.is_some() {
        println!("[cli] approval: {:?}", cli.approval);
    }
    if cli.sandbox.is_some() {
        println!("[cli] sandbox: {:?}", cli.sandbox);
    }

    let mut agent = Agent::with_config(config);

    println!(
        "OpenJax CLI 已启动（model backend: {}，approval: {}，sandbox: {}）。输入内容开始对话，输入 /exit 退出。",
        agent.model_backend_name(),
        agent.approval_policy_name(),
        agent.sandbox_mode_name(),
    );
    println!(
        "可用工具示例: tool:list_dir dir_path=. | tool:read_file file_path=docs/todo.md | tool:grep_files pattern=OpenJax path=."
    );
    println!(
        "命令执行示例: tool:shell cmd='ls -la' | tool:shell cmd='curl https://example.com' require_escalated=true"
    );
    println!(
        "补丁示例: tool:apply_patch patch='*** Begin Patch\\n*** Add File: hello.txt\\n+hello\\n*** End Patch'"
    );

    let mut rl = Editor::<(), DefaultHistory>::new()?;
    let history_path = std::env::current_dir()?
        .join(".openjax")
        .join("history.txt");

    if let Some(dir) = history_path.parent() {
        if !dir.exists() {
            std::fs::create_dir_all(dir)?;
        }
    }

    if let Err(e) = rl.load_history(&history_path) {
        if history_path.exists() {
            eprintln!("[cli] failed to load history: {}", e);
        }
    }

    loop {
        let readline = rl.readline("> ");
        match readline {
            Ok(input) => {
                let input = input.trim().to_string();
                if input.is_empty() {
                    continue;
                }

                rl.add_history_entry(input.as_str())?;

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
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("exit");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    if let Err(e) = rl.save_history(&history_path) {
        eprintln!("[cli] failed to save history: {}", e);
    }

    Ok(())
}

fn print_event(event: &Event) {
    match event {
        Event::TurnStarted { turn_id } => println!("[turn:{turn_id}] started"),
        Event::ToolCallStarted {
            turn_id, tool_name, ..
        } => {
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
        Event::AssistantDelta {
            turn_id,
            content_delta,
        } => {
            println!("[turn:{turn_id}] assistant delta: {content_delta}");
        }
        Event::ApprovalRequested {
            turn_id,
            request_id,
            target,
            reason,
            tool_name,
            command_preview,
            risk_tags,
            sandbox_backend,
            degrade_reason,
        } => {
            println!(
                "[turn:{turn_id}] approval requested: id={request_id} target={target} reason={reason} tool={tool_name:?} cmd={command_preview:?} risks={risk_tags:?} backend={sandbox_backend:?} degrade={degrade_reason:?}"
            );
        }
        Event::ApprovalResolved {
            turn_id,
            request_id,
            approved,
        } => {
            println!("[turn:{turn_id}] approval resolved: id={request_id} approved={approved}");
        }
        Event::TurnCompleted { turn_id } => println!("[turn:{turn_id}] completed"),
        Event::ShutdownComplete => println!("shutdown complete"),
        // Multi-agent events (预留扩展)
        Event::AgentSpawned {
            parent_thread_id,
            new_thread_id,
        } => {
            println!("[agent] spawned: parent={parent_thread_id:?} new={new_thread_id:?}")
        }
        Event::AgentStatusChanged { thread_id, status } => {
            println!("[agent:{thread_id:?}] status: {status:?}")
        }
    }
}
