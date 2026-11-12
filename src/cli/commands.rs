use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};
use serde::Deserialize;
use uuid::Uuid;

use crate::agent::engine::AgentEngine;
use crate::agent::state::EngineOutcome;
use crate::checkpoints::sqlite::SqliteCheckpointStore;
use crate::checkpoints::store::CheckpointStore;
use crate::checkpoints::types::ForkRunRequest;
use crate::domain::{ApprovalDecision, ExecutionScope, HistoryLimits};
use crate::events::sink::{ConfiguredEventSink, JsonLinesEventSink, NullEventSink};
use crate::governance::policy::DefaultApprovalPolicy;
use crate::models::adapter::OpenAiCompatibleClient;
use crate::models::types::ModelConfig;
use crate::schema::WorkflowSpec;
use crate::tools::register_workspace_tools;
use crate::tools::types::ToolRegistry;
use crate::workflow::engine::{WorkflowExecutor, WorkflowOutcome};

pub const DEFAULT_TENANT_ID: &str = "00000000-0000-0000-0000-000000000001";
pub const DEFAULT_WORKSPACE_ID: &str = "00000000-0000-0000-0000-000000000002";

#[derive(Parser, Debug)]
#[command(name = "albaai", version = "0.1.0", about = "Alba Agent Core Engine")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Run(RunArgs),
    Status(StatusArgs),
    Approve(ApproveArgs),
    Deny(DenyArgs),
    Resume(ResumeArgs),
    Cancel(CancelArgs),
    Fork(ForkArgs),
}

#[derive(Args, Debug)]
pub struct RunArgs {
    #[arg(short, long)]
    pub prompt: Option<String>,

    #[arg(long)]
    pub workflow: Option<PathBuf>,

    #[arg(long)]
    pub tenant_id: Option<String>,

    #[arg(long)]
    pub workspace_id: Option<String>,

    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(long)]
    pub run_id: Option<String>,

    #[arg(long, default_value = ".")]
    pub workspace_root: PathBuf,

    #[arg(long, default_value = "albaai.db")]
    pub db_path: PathBuf,

    #[arg(long)]
    pub api_base: Option<String>,

    #[arg(long)]
    pub api_key_stdin: bool,

    #[arg(long)]
    pub model: Option<String>,

    #[arg(long)]
    pub require_approval: bool,

    #[arg(long)]
    pub approval_prefix: Option<String>,

    #[arg(long)]
    pub max_history_messages: Option<usize>,

    #[arg(long)]
    pub max_history_bytes: Option<usize>,

    #[arg(long)]
    pub event_log: Option<String>,
}

#[derive(Args, Debug)]
pub struct StatusArgs {
    #[arg(short, long)]
    pub run_id: String,

    #[arg(long)]
    pub tenant_id: Option<String>,

    #[arg(long)]
    pub workspace_id: Option<String>,

    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(long, default_value = "albaai.db")]
    pub db_path: PathBuf,
}

#[derive(Args, Debug)]
pub struct ApproveArgs {
    #[arg(short, long)]
    pub run_id: String,

    #[arg(short, long)]
    pub approval_id: String,

    #[arg(short, long, default_value = "allow_once")]
    pub decision: String,

    #[arg(long)]
    pub actor: Option<String>,

    #[arg(long)]
    pub tenant_id: Option<String>,

    #[arg(long)]
    pub workspace_id: Option<String>,

    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(long, default_value = "albaai.db")]
    pub db_path: PathBuf,
}

#[derive(Args, Debug)]
pub struct DenyArgs {
    #[arg(short, long)]
    pub run_id: String,

    #[arg(short, long)]
    pub approval_id: String,

    #[arg(long)]
    pub actor: Option<String>,

    #[arg(long)]
    pub tenant_id: Option<String>,

    #[arg(long)]
    pub workspace_id: Option<String>,

    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(long, default_value = "albaai.db")]
    pub db_path: PathBuf,
}

#[derive(Args, Debug)]
pub struct ResumeArgs {
    #[arg(short, long)]
    pub run_id: String,

    #[arg(long)]
    pub workflow: Option<PathBuf>,

    #[arg(long)]
    pub tenant_id: Option<String>,

    #[arg(long)]
    pub workspace_id: Option<String>,

    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(long, default_value = ".")]
    pub workspace_root: PathBuf,

    #[arg(long, default_value = "albaai.db")]
    pub db_path: PathBuf,

    #[arg(long)]
    pub api_base: Option<String>,

    #[arg(long)]
    pub api_key_stdin: bool,

    #[arg(long)]
    pub model: Option<String>,

    #[arg(long)]
    pub max_history_messages: Option<usize>,

    #[arg(long)]
    pub max_history_bytes: Option<usize>,

    #[arg(long)]
    pub event_log: Option<String>,
}

#[derive(Args, Debug)]
pub struct ForkArgs {
    #[arg(short, long)]
    pub run_id: String,

    #[arg(short, long)]
    pub sequence: u32,

    #[arg(long, default_value = "fork")]
    pub branch_label: String,

    #[arg(long)]
    pub new_run_id: Option<String>,

    #[arg(long)]
    pub prompt_override: Option<String>,

    #[arg(long)]
    pub model_override: Option<String>,

    #[arg(long)]
    pub tenant_id: Option<String>,

    #[arg(long)]
    pub workspace_id: Option<String>,

    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(long, default_value = "albaai.db")]
    pub db_path: PathBuf,
}

#[derive(Args, Debug)]
pub struct CancelArgs {
    #[arg(short, long)]
    pub run_id: String,

    #[arg(long)]
    pub tenant_id: Option<String>,

    #[arg(long)]
    pub workspace_id: Option<String>,

    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(long, default_value = "albaai.db")]
    pub db_path: PathBuf,
}

#[derive(Debug, Default, Deserialize)]
pub struct TrustedConfig {
    pub tenant_id: Option<String>,
    pub workspace_id: Option<String>,
    pub actor_id: Option<String>,
}

pub fn load_trusted_config(cli_config: Option<&Path>) -> Result<Option<TrustedConfig>, String> {
    let env_path = match std::env::var("NEXUS_CONFIG_FILE") {
        Ok(val) => {
            let trimmed = val.trim();
            if trimmed.is_empty() {
                return Err("NEXUS_CONFIG_FILE is set but empty".to_string());
            }
            Some(PathBuf::from(trimmed))
        }
        Err(_) => None,
    };

    let path = match (cli_config, env_path) {
        (Some(c), Some(e)) => {
            if c != e.as_path() {
                return Err("Conflicting --config and NEXUS_CONFIG_FILE paths".to_string());
            }
            Some(c.to_path_buf())
        }
        (Some(c), None) => Some(c.to_path_buf()),
        (None, Some(e)) => Some(e),
        (None, None) => None,
    };

    match path {
        None => Ok(None),
        Some(p) => {
            let content = std::fs::read_to_string(&p)
                .map_err(|e| format!("Failed to read config file {}: {}", p.display(), e))?;
            let cfg: TrustedConfig = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse config file {}: {}", p.display(), e))?;
            Ok(Some(cfg))
        }
    }
}

fn resolve_scope_dimension(
    label: &str,
    env_var: &str,
    config_val: Option<&str>,
    cli_val: Option<&str>,
    default_val: &str,
) -> Result<String, String> {
    let env_trusted = match std::env::var(env_var) {
        Ok(v) => {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                return Err(format!("{} is set but empty", env_var));
            }
            Some(trimmed.to_string())
        }
        Err(_) => None,
    };

    let cfg_trusted = match config_val {
        Some(v) => {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                return Err(format!("Trusted config {} is empty", label));
            }
            Some(trimmed.to_string())
        }
        None => None,
    };

    let trusted = env_trusted.or(cfg_trusted);

    if let Some(cli) = cli_val {
        let trimmed = cli.trim();
        if trimmed.is_empty() {
            return Err(format!("CLI --{} is empty", label.replace('_', "-")));
        }
        if let Some(ref pinned) = trusted {
            if trimmed != pinned {
                return Err(format!(
                    "Scope mismatch: CLI --{} '{}' conflicts with trusted {} '{}'",
                    label.replace('_', "-"),
                    trimmed,
                    label,
                    pinned
                ));
            }
        }
        return Ok(trimmed.to_string());
    }

    Ok(trusted.unwrap_or_else(|| default_val.to_string()))
}

pub fn resolve_scope(
    config: Option<&TrustedConfig>,
    cli_tenant: Option<&str>,
    cli_workspace: Option<&str>,
    run_id: &str,
) -> Result<ExecutionScope, String> {
    let tenant_str = resolve_scope_dimension(
        "tenant_id",
        "NEXUS_TENANT_ID",
        config.and_then(|c| c.tenant_id.as_deref()),
        cli_tenant,
        DEFAULT_TENANT_ID,
    )?;
    let workspace_str = resolve_scope_dimension(
        "workspace_id",
        "NEXUS_WORKSPACE_ID",
        config.and_then(|c| c.workspace_id.as_deref()),
        cli_workspace,
        DEFAULT_WORKSPACE_ID,
    )?;
    parse_scope(&tenant_str, &workspace_str, run_id)
}

pub fn resolve_trusted_actor(config: Option<&TrustedConfig>) -> Result<String, String> {
    if let Ok(env_actor) = std::env::var("NEXUS_ACTOR_ID") {
        let trimmed = env_actor.trim();
        if trimmed.is_empty() {
            return Err("NEXUS_ACTOR_ID is set but empty".to_string());
        }
        return Ok(trimmed.to_string());
    }

    if let Some(cfg) = config {
        if let Some(ref cfg_actor) = cfg.actor_id {
            let trimmed = cfg_actor.trim();
            if trimmed.is_empty() {
                return Err("Trusted config actor_id is empty".to_string());
            }
            return Ok(trimmed.to_string());
        }
    }

    Err(
        "Missing trusted actor identity: set NEXUS_ACTOR_ID or configure actor_id in NEXUS_CONFIG_FILE"
            .to_string(),
    )
}

fn read_api_key_from_stdin() -> Result<String, String> {
    let stdin = std::io::stdin();
    let handle = stdin.lock();
    let mut buf = String::new();
    handle
        .take(8192)
        .read_to_string(&mut buf)
        .map_err(|e| format!("Failed reading API key from stdin: {}", e))?;
    let key = buf.lines().next().unwrap_or("").trim().to_string();
    if key.is_empty() {
        return Err("API key read from stdin (--api-key-stdin) was empty".to_string());
    }
    Ok(key)
}

pub fn resolve_model_config(
    cli_base: Option<String>,
    api_key_stdin: bool,
    cli_model: Option<String>,
) -> Result<ModelConfig, String> {
    let api_base = cli_base
        .or_else(|| std::env::var("NEXUS_MODEL_BASE_URL").ok())
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

    let api_key = if api_key_stdin {
        read_api_key_from_stdin()?
    } else {
        std::env::var("NEXUS_API_KEY").unwrap_or_default()
    };

    let model = cli_model
        .or_else(|| std::env::var("NEXUS_MODEL").ok())
        .unwrap_or_else(|| "gpt-4o-mini".to_string());

    Ok(ModelConfig::new(api_base, api_key, model))
}

pub fn resolve_event_sink(cli_event_log: Option<String>) -> (ConfiguredEventSink, bool) {
    let target = cli_event_log.or_else(|| std::env::var("NEXUS_EVENT_LOG").ok());
    match target {
        Some(t) if t == "stdout" => (
            ConfiguredEventSink::JsonLines(JsonLinesEventSink::stdout()),
            true,
        ),
        Some(path) if !path.trim().is_empty() => (
            ConfiguredEventSink::JsonLines(JsonLinesEventSink::to_file(path)),
            false,
        ),
        _ => (ConfiguredEventSink::Null(NullEventSink), false),
    }
}

fn print_human(to_stderr: bool, msg: &str) {
    if to_stderr {
        eprintln!("{}", msg);
    } else {
        println!("{}", msg);
    }
}

pub fn parse_scope(
    tenant_str: &str,
    workspace_str: &str,
    run_str: &str,
) -> Result<ExecutionScope, String> {
    let tenant_id = Uuid::parse_str(tenant_str).map_err(|e| format!("Invalid tenant_id: {}", e))?;
    let workspace_id =
        Uuid::parse_str(workspace_str).map_err(|e| format!("Invalid workspace_id: {}", e))?;
    let run_id = Uuid::parse_str(run_str).map_err(|e| format!("Invalid run_id: {}", e))?;
    ExecutionScope::new(tenant_id, workspace_id, run_id).map_err(|e| e.to_string())
}

pub async fn handle_run(args: RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    let run_id = match args.run_id {
        Some(r) => r,
        None => Uuid::new_v4().to_string(),
    };

    let config = load_trusted_config(args.config.as_deref())?;
    let scope = resolve_scope(
        config.as_ref(),
        args.tenant_id.as_deref(),
        args.workspace_id.as_deref(),
        &run_id,
    )?;
    let model_config = resolve_model_config(args.api_base, args.api_key_stdin, args.model)?;

    let checkpoint_store = Arc::new(SqliteCheckpointStore::new(&args.db_path)?);
    let (sink, human_to_stderr) = resolve_event_sink(args.event_log);
    let event_sink = Arc::new(sink);

    let mut policy = DefaultApprovalPolicy::new().with_workspace_root(&args.workspace_root);
    if args.require_approval {
        policy = policy.with_require_all(true);
    }
    if let Some(prefix) = args.approval_prefix {
        policy = policy.with_sensitive_path_prefix(prefix);
    }

    let mut registry = ToolRegistry::new();
    register_workspace_tools(
        &mut registry,
        &args.workspace_root,
        Some(Arc::clone(&event_sink) as Arc<dyn crate::events::sink::EventSink>),
    );

    let client = Arc::new(OpenAiCompatibleClient::new(model_config)?);

    let mut limits = HistoryLimits::default();
    if let Some(m) = args.max_history_messages {
        limits.max_messages = m;
    }
    if let Some(b) = args.max_history_bytes {
        limits.max_bytes = b;
    }

    if let Some(wf_path) = args.workflow {
        let raw = std::fs::read_to_string(&wf_path)?;
        let spec: WorkflowSpec = serde_json::from_str(&raw)
            .map_err(|e| format!("Failed to parse workflow JSON: {}", e))?;

        let mut executor = WorkflowExecutor::new(
            client,
            checkpoint_store,
            event_sink,
            Arc::new(policy),
            registry,
            &args.workspace_root,
        );

        if let Ok(actor) = resolve_trusted_actor(config.as_ref()) {
            executor = executor.with_author_id(actor);
        }

        print_human(
            human_to_stderr,
            &format!("Starting workflow run {}", scope.run_id),
        );
        let outcome = executor.start_workflow(&spec, scope, None).await?;

        match outcome {
            WorkflowOutcome::Completed(resp) => {
                print_human(
                    human_to_stderr,
                    &format!("Workflow run completed successfully:\n{}", resp),
                );
            }
            WorkflowOutcome::AwaitingApproval {
                approval_id,
                action_description,
                node_id,
            } => {
                print_human(human_to_stderr, "Workflow run paused for human approval.");
                print_human(human_to_stderr, &format!("Node: {}", node_id));
                print_human(human_to_stderr, &format!("Approval ID: {}", approval_id));
                print_human(human_to_stderr, &format!("Action: {}", action_description));
                print_human(
                    human_to_stderr,
                    &format!(
                        "Use `nexus-albaai approve --run-id {} --approval-id {} --decision allow_once` to approve.",
                        scope.run_id, approval_id
                    ),
                );
            }
            WorkflowOutcome::Failed(err) => {
                eprintln!("Workflow run failed: {}", err);
            }
            WorkflowOutcome::Cancelled => {
                print_human(human_to_stderr, "Workflow run was cancelled.");
            }
            WorkflowOutcome::Paused { node_id } => {
                print_human(
                    human_to_stderr,
                    &format!("Workflow paused at node {}.", node_id),
                );
            }
        }

        return Ok(());
    }

    let prompt = args
        .prompt
        .ok_or_else(|| "Either --prompt or --workflow must be specified".to_string())?;

    let mut engine = AgentEngine::new(
        client,
        checkpoint_store,
        event_sink,
        Arc::new(policy),
        registry,
    )
    .with_history_limits(limits);

    if let Ok(actor) = resolve_trusted_actor(config.as_ref()) {
        engine = engine.with_author_id(actor);
    }

    print_human(human_to_stderr, &format!("Starting run {}", scope.run_id));
    let outcome = engine.start_run(scope, &prompt).await?;

    match outcome {
        EngineOutcome::Completed(resp) => {
            print_human(
                human_to_stderr,
                &format!("Run completed successfully:\n{}", resp),
            );
        }
        EngineOutcome::Paused {
            approval_id,
            tool_name,
            action_description,
        } => {
            print_human(human_to_stderr, "Run paused for human approval.");
            print_human(human_to_stderr, &format!("Approval ID: {}", approval_id));
            print_human(human_to_stderr, &format!("Tool: {}", tool_name));
            print_human(human_to_stderr, &format!("Action: {}", action_description));
            print_human(
                human_to_stderr,
                &format!(
                    "Use `nexus-albaai approve --run-id {} --approval-id {} --decision allow_once` to approve.",
                    scope.run_id, approval_id
                ),
            );
        }
        EngineOutcome::Failed(err) => {
            eprintln!("Run failed: {}", err);
        }
        EngineOutcome::Cancelled => {
            print_human(human_to_stderr, "Run was cancelled.");
        }
    }

    Ok(())
}

pub async fn handle_status(args: StatusArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_trusted_config(args.config.as_deref())?;
    let scope = resolve_scope(
        config.as_ref(),
        args.tenant_id.as_deref(),
        args.workspace_id.as_deref(),
        &args.run_id,
    )?;
    let checkpoint_store = SqliteCheckpointStore::new(&args.db_path)?;

    let latest_wf = checkpoint_store.get_latest_workflow_step(&scope).await?;
    if let Some(wf_step) = latest_wf {
        let run_state_str = match serde_json::from_value::<
            crate::workflow::state::WorkflowExecutionContext,
        >(wf_step.state_json.clone())
        {
            Ok(ctx) => ctx.run_state.as_str().to_string(),
            Err(_) => wf_step.status.as_str().to_string(),
        };
        println!("Run ID: {}", wf_step.scope.run_id);
        println!("Tenant ID: {}", wf_step.scope.tenant_id);
        println!("Workspace ID: {}", wf_step.scope.workspace_id);
        println!("Status: {}", run_state_str);
        println!("Node: {}", wf_step.node_id);
        println!("Sequence: {}", wf_step.sequence);
        if let Some(app_id) = wf_step.pending_approval_id {
            println!("Pending approval ID: {}", app_id);
            if let Some(app) = checkpoint_store.get_approval(&scope, app_id).await? {
                println!("Approval tool: {}", app.tool_name);
                println!("Approval action: {}", app.action_description);
                println!("Approval status: {}", app.status.as_str());
            }
        }
        return Ok(());
    }

    let latest = checkpoint_store.get_latest_checkpoint(&scope).await?;
    let cancelled = checkpoint_store.is_cancelled(&scope).await?;
    match latest {
        None => {
            println!("No checkpoints found for run {}", scope.run_id);
        }
        Some(cp) => {
            println!("Run ID: {}", cp.scope.run_id);
            println!("Tenant ID: {}", cp.scope.tenant_id);
            println!("Workspace ID: {}", cp.scope.workspace_id);
            println!("Status: {}", cp.status.as_str());
            println!("Cancellation requested: {}", cancelled);
            println!("Sequence: {}", cp.sequence);
            println!("History message count: {}", cp.history.len());
            if let Some(app_id) = cp.pending_approval_id {
                println!("Pending approval ID: {}", app_id);
                if let Some(app) = checkpoint_store.get_approval(&scope, app_id).await? {
                    println!("Approval tool: {}", app.tool_name);
                    println!("Approval action: {}", app.action_description);
                    println!("Approval status: {}", app.status.as_str());
                }
            }
        }
    }
    Ok(())
}

pub async fn handle_approve(args: ApproveArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_trusted_config(args.config.as_deref())?;
    let actor = match args.actor {
        Some(a) => a,
        None => resolve_trusted_actor(config.as_ref())?,
    };
    let scope = resolve_scope(
        config.as_ref(),
        args.tenant_id.as_deref(),
        args.workspace_id.as_deref(),
        &args.run_id,
    )?;
    let approval_id =
        Uuid::parse_str(&args.approval_id).map_err(|e| format!("Invalid approval_id: {}", e))?;

    let decision = ApprovalDecision::from_str(&args.decision).map_err(|e| {
        format!(
            "Invalid decision '{}', must be 'allow_once' or 'deny': {}",
            args.decision, e
        )
    })?;

    let checkpoint_store = SqliteCheckpointStore::new(&args.db_path)?;
    checkpoint_store
        .resolve_approval(&scope, approval_id, decision, &actor)
        .await?;

    println!(
        "Approval {} resolved with decision '{}' by '{}'.",
        approval_id,
        decision.as_str(),
        actor
    );
    println!(
        "You can now resume the run using `nexus-albaai resume --run-id {}`.",
        scope.run_id
    );

    Ok(())
}

pub async fn handle_deny(args: DenyArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_trusted_config(args.config.as_deref())?;
    let actor = match args.actor {
        Some(a) => a,
        None => resolve_trusted_actor(config.as_ref())?,
    };
    let scope = resolve_scope(
        config.as_ref(),
        args.tenant_id.as_deref(),
        args.workspace_id.as_deref(),
        &args.run_id,
    )?;
    let approval_id =
        Uuid::parse_str(&args.approval_id).map_err(|e| format!("Invalid approval_id: {}", e))?;

    let checkpoint_store = SqliteCheckpointStore::new(&args.db_path)?;
    checkpoint_store
        .resolve_approval(&scope, approval_id, ApprovalDecision::Deny, &actor)
        .await?;

    println!("Approval {} denied by '{}'.", approval_id, actor);
    println!(
        "You can finalize the run state using `nexus-albaai resume --run-id {}`.",
        scope.run_id
    );

    Ok(())
}

pub async fn handle_resume(args: ResumeArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_trusted_config(args.config.as_deref())?;
    let scope = resolve_scope(
        config.as_ref(),
        args.tenant_id.as_deref(),
        args.workspace_id.as_deref(),
        &args.run_id,
    )?;
    let model_config = resolve_model_config(args.api_base, args.api_key_stdin, args.model)?;

    let checkpoint_store = Arc::new(SqliteCheckpointStore::new(&args.db_path)?);
    let (sink, human_to_stderr) = resolve_event_sink(args.event_log);
    let event_sink = Arc::new(sink);
    let policy = Arc::new(DefaultApprovalPolicy::new().with_workspace_root(&args.workspace_root));

    let mut registry = ToolRegistry::new();
    register_workspace_tools(
        &mut registry,
        &args.workspace_root,
        Some(Arc::clone(&event_sink) as Arc<dyn crate::events::sink::EventSink>),
    );

    let client = Arc::new(OpenAiCompatibleClient::new(model_config)?);

    if let Some(wf_path) = args.workflow {
        let raw = std::fs::read_to_string(&wf_path)?;
        let spec: WorkflowSpec = serde_json::from_str(&raw)
            .map_err(|e| format!("Failed to parse workflow JSON: {}", e))?;

        let mut executor = WorkflowExecutor::new(
            client,
            checkpoint_store,
            event_sink,
            policy,
            registry,
            &args.workspace_root,
        );

        if let Ok(actor) = resolve_trusted_actor(config.as_ref()) {
            executor = executor.with_author_id(actor);
        }

        print_human(
            human_to_stderr,
            &format!("Resuming workflow run {}", scope.run_id),
        );
        let outcome = executor.resume_workflow(&spec, scope).await?;

        match outcome {
            WorkflowOutcome::Completed(resp) => {
                print_human(
                    human_to_stderr,
                    &format!("Workflow run resumed and completed successfully:\n{}", resp),
                );
            }
            WorkflowOutcome::AwaitingApproval {
                approval_id,
                action_description,
                node_id,
            } => {
                print_human(
                    human_to_stderr,
                    "Workflow run is still paused waiting for approval.",
                );
                print_human(human_to_stderr, &format!("Node: {}", node_id));
                print_human(human_to_stderr, &format!("Approval ID: {}", approval_id));
                print_human(human_to_stderr, &format!("Action: {}", action_description));
            }
            WorkflowOutcome::Failed(err) => {
                eprintln!("Workflow run failed after resume: {}", err);
            }
            WorkflowOutcome::Cancelled => {
                print_human(human_to_stderr, "Workflow run was cancelled.");
            }
            WorkflowOutcome::Paused { node_id } => {
                print_human(
                    human_to_stderr,
                    &format!("Workflow paused at node {}.", node_id),
                );
            }
        }

        return Ok(());
    }

    let mut limits = HistoryLimits::default();
    if let Some(m) = args.max_history_messages {
        limits.max_messages = m;
    }
    if let Some(b) = args.max_history_bytes {
        limits.max_bytes = b;
    }

    let engine = AgentEngine::new(client, checkpoint_store, event_sink, policy, registry)
        .with_history_limits(limits);

    print_human(human_to_stderr, &format!("Resuming run {}", scope.run_id));
    let outcome = engine.resume_run(scope).await?;

    match outcome {
        EngineOutcome::Completed(resp) => {
            print_human(
                human_to_stderr,
                &format!("Run resumed and completed successfully:\n{}", resp),
            );
        }
        EngineOutcome::Paused {
            approval_id,
            tool_name,
            action_description,
        } => {
            print_human(human_to_stderr, "Run is still paused waiting for approval.");
            print_human(human_to_stderr, &format!("Approval ID: {}", approval_id));
            print_human(human_to_stderr, &format!("Tool: {}", tool_name));
            print_human(human_to_stderr, &format!("Action: {}", action_description));
        }
        EngineOutcome::Failed(err) => {
            eprintln!("Run failed after resume: {}", err);
        }
        EngineOutcome::Cancelled => {
            print_human(human_to_stderr, "Run was cancelled.");
        }
    }

    Ok(())
}

pub async fn handle_cancel(args: CancelArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_trusted_config(args.config.as_deref())?;
    let scope = resolve_scope(
        config.as_ref(),
        args.tenant_id.as_deref(),
        args.workspace_id.as_deref(),
        &args.run_id,
    )?;
    let checkpoint_store = SqliteCheckpointStore::new(&args.db_path)?;

    if checkpoint_store
        .get_latest_checkpoint(&scope)
        .await?
        .is_none()
        && checkpoint_store
            .get_latest_workflow_step(&scope)
            .await?
            .is_none()
    {
        return Err(format!(
            "No checkpoints found for run {} in current scope",
            scope.run_id
        )
        .into());
    }

    checkpoint_store.request_cancellation(&scope).await?;
    println!("Cancellation requested for run {}", scope.run_id);
    Ok(())
}

pub async fn handle_fork(args: ForkArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_trusted_config(args.config.as_deref())?;
    let scope = resolve_scope(
        config.as_ref(),
        args.tenant_id.as_deref(),
        args.workspace_id.as_deref(),
        &args.run_id,
    )?;

    let checkpoint_store = SqliteCheckpointStore::new(&args.db_path)?;

    let wf_steps = checkpoint_store.list_workflow_steps(&scope).await?;
    if !wf_steps.is_empty() {
        let (forked_scope, branch) = checkpoint_store
            .fork_workflow_run(&scope, args.sequence, &args.branch_label)
            .await?;
        println!(
            "Forked workflow run {} from parent {} at sequence {} (branch '{}', branch_id {})",
            forked_scope.run_id,
            branch.parent_run_id,
            branch.fork_sequence,
            branch.branch_label,
            branch.branch_id
        );
        return Ok(());
    }

    let new_run_id = match args.new_run_id {
        Some(raw) => Some(Uuid::parse_str(&raw).map_err(|e| format!("Invalid new_run_id: {}", e))?),
        None => None,
    };
    let author_id = resolve_trusted_actor(config.as_ref()).ok();
    let (cp, branch) = checkpoint_store
        .fork_run(&ForkRunRequest {
            source_scope: scope,
            target_sequence: args.sequence,
            new_run_id,
            branch_label: args.branch_label,
            model_override: args.model_override,
            prompt_override: args.prompt_override,
            author_id,
        })
        .await?;

    println!(
        "Forked run {} from parent {} at sequence {} (branch '{}', branch_id {})",
        cp.scope.run_id,
        branch.parent_run_id,
        branch.fork_sequence,
        branch.branch_label,
        branch.branch_id
    );
    Ok(())
}
