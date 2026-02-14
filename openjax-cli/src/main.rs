use clap::Parser;
use openjax_core::{Agent, Config};
use openjax_protocol::{Event, Op};
use std::io::{self, Write};
use std::path::PathBuf;

const DEFAULT_CONFIG: &str = r#"# OpenJax Configuration
# Edit this file to configure your API keys

[model]
# Backend: minimax | openai | echo
backend = "echo"
# API key (env vars take priority if set)
# For OpenAI: OPENAI_API_KEY
# For MiniMax: OPENJAX_MINIMAX_API_KEY
# api_key = "your-api-key"
# base_url = "https://api.example.com"
# model = "gpt-4.1-mini"

[sandbox]
mode = "workspace_write"
approval_policy = "on_request"

[agent]
max_agents = 4
max_depth = 1
"#;

fn ensure_local_config() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let config_path = cwd.join(".openjax.toml");
    
    if config_path.exists() {
        return Some(config_path);
    }
    
    match std::fs::write(&config_path, DEFAULT_CONFIG) {
        Ok(()) => {
            println!("[config] created default config: {}", config_path.display());
            Some(config_path)
        }
        Err(e) => {
            eprintln!("[config] failed to create default config: {}", e);
            None
        }
    }
}

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

    /// 配置文件路径 (默认自动查找 ./.openjax.toml 或 ~/.openjax/config.toml)
    #[arg(long)]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
        if let Some(path) = ensure_local_config() {
            match Config::from_file(&path) {
                Ok(c) => {
                    println!("[config] loaded from: {}", path.display());
                    c
                }
                Err(e) => {
                    eprintln!("[config] failed to load {}: {}", path.display(), e);
                    Config::load()
                }
            }
        } else {
            let config = Config::load();
            if let Some(path) = Config::find_config_file() {
                println!("[config] loaded from: {}", path.display());
            } else {
                println!("[config] no config file found, using defaults");
            }
            config
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
