//! app-detector CLI - Detect application types, frameworks, and environments
//!
//! A command-line tool for automatically discovering what your application is
//! and how it can be run.

use anyhow::{Context, Result};
use app_detector::{
    DetectionConfig, DetectionEngine, DetectionReport, StrategyRegistry,
    types::{AppTypeCategory, EnvCapabilityCategory, StrategyCategory},
};
use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use std::collections::HashSet;
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// app-detector - Detect application types, frameworks, and deployment environments
#[derive(Parser, Debug)]
#[command(
    name = "app-detector",
    version,
    about = "Automatically detect application types, frameworks, and deployment environments",
    long_about = "A strategy-based detection platform that discovers:\n  \
                  • Languages (Rust, Node.js, Python, etc.)\n  \
                  • Frameworks (React, Django, etc.)\n  \
                  • Deployment environments (Docker, Kubernetes, etc.)\n  \
                  • Available commands and configurations"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format
    #[arg(short, long, global = true, value_enum, default_value = "human")]
    output: OutputFormat,

    /// Enable verbose logging (can be used multiple times: -v, -vv, -vvv)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Detect applications in a directory
    #[command(alias = "d")]
    Detect {
        /// Path to detect (defaults to current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum directory depth to search
        #[arg(short, long, default_value = "3")]
        max_depth: usize,

        /// Additional patterns to ignore
        #[arg(short, long)]
        ignore: Vec<String>,

        /// Filter by category (can specify multiple)
        #[arg(short = 'c', long)]
        category: Vec<CategoryFilter>,

        /// Show only app types (languages, frameworks, services)
        #[arg(long, conflicts_with = "env_only")]
        app_only: bool,

        /// Show only environment capabilities (docker, kubernetes, etc.)
        #[arg(long, conflicts_with = "app_only")]
        env_only: bool,

        /// Minimum confidence level (0.0-1.0)
        #[arg(long, default_value = "0.0")]
        min_confidence: f32,
    },

    /// List all available detection strategies
    #[command(alias = "ls")]
    ListStrategies {
        /// Filter by category
        #[arg(short, long)]
        category: Option<CategoryFilter>,
    },

    /// List all available categories
    ListCategories,

    /// Show information about a specific strategy
    Info {
        /// Strategy ID to show info for
        strategy_id: String,
    },
}

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    /// Human-readable output with colors
    Human,
    /// JSON output
    Json,
    /// JSON output with pretty printing
    JsonPretty,
}

#[derive(Debug, Clone, ValueEnum)]
enum CategoryFilter {
    // App Types
    Language,
    Framework,
    Monorepo,
    BuildTool,
    PackageManager,
    Database,
    Service,
    MessageQueue,
    ReverseProxy,
    Cicd,
    Testing,
    Linting,

    // Environment Capabilities
    Local,
    Docker,
    OrbStack,
    Kubernetes,
    DockerSwarm,
    Nomad,
    CloudPlatform,
    Infrastructure,
}

impl CategoryFilter {
    fn to_strategy_category(&self) -> StrategyCategory {
        match self {
            // App Types
            Self::Language => StrategyCategory::AppType(AppTypeCategory::Language),
            Self::Framework => StrategyCategory::AppType(AppTypeCategory::Framework),
            Self::Monorepo => StrategyCategory::AppType(AppTypeCategory::Monorepo),
            Self::BuildTool => StrategyCategory::AppType(AppTypeCategory::BuildTool),
            Self::PackageManager => StrategyCategory::AppType(AppTypeCategory::PackageManager),
            Self::Database => StrategyCategory::AppType(AppTypeCategory::Database),
            Self::Service => StrategyCategory::AppType(AppTypeCategory::Service),
            Self::MessageQueue => StrategyCategory::AppType(AppTypeCategory::MessageQueue),
            Self::ReverseProxy => StrategyCategory::AppType(AppTypeCategory::ReverseProxy),
            Self::Cicd => StrategyCategory::AppType(AppTypeCategory::Cicd),
            Self::Testing => StrategyCategory::AppType(AppTypeCategory::Testing),
            Self::Linting => StrategyCategory::AppType(AppTypeCategory::Linting),

            // Environment Capabilities
            Self::Local => StrategyCategory::EnvCapability(EnvCapabilityCategory::Local),
            Self::Docker => StrategyCategory::EnvCapability(EnvCapabilityCategory::Docker),
            Self::OrbStack => StrategyCategory::EnvCapability(EnvCapabilityCategory::OrbStack),
            Self::Kubernetes => StrategyCategory::EnvCapability(EnvCapabilityCategory::Kubernetes),
            Self::DockerSwarm => StrategyCategory::EnvCapability(EnvCapabilityCategory::DockerSwarm),
            Self::Nomad => StrategyCategory::EnvCapability(EnvCapabilityCategory::Nomad),
            Self::CloudPlatform => StrategyCategory::EnvCapability(EnvCapabilityCategory::CloudPlatform),
            Self::Infrastructure => StrategyCategory::EnvCapability(EnvCapabilityCategory::Infrastructure),
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging based on verbosity
    init_logging(cli.verbose, cli.no_color);

    // Disable colors if requested
    if cli.no_color {
        colored::control::set_override(false);
    }

    match cli.command {
        Commands::Detect {
            path,
            max_depth,
            ignore,
            category,
            app_only,
            env_only,
            min_confidence,
        } => {
            let config = build_detection_config(
                max_depth,
                ignore,
                category,
                app_only,
                env_only,
            );

            let registry = StrategyRegistry::with_defaults();
            let engine = DetectionEngine::with_config(registry, config);

            let report = engine
                .detect(&path)
                .with_context(|| format!("Failed to detect applications in {}", path.display()))?;

            output_report(&report, &cli.output, min_confidence)?;
        }

        Commands::ListStrategies { category } => {
            let registry = StrategyRegistry::with_defaults();
            output_strategies(&registry, category, &cli.output)?;
        }

        Commands::ListCategories => {
            output_categories(&cli.output)?;
        }

        Commands::Info { strategy_id } => {
            let registry = StrategyRegistry::with_defaults();
            output_strategy_info(&registry, &strategy_id, &cli.output)?;
        }
    }

    Ok(())
}

fn init_logging(verbosity: u8, no_color: bool) {
    let log_level = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("app_detector={}", log_level)));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_ansi(!no_color);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}

fn build_detection_config(
    max_depth: usize,
    ignore: Vec<String>,
    categories: Vec<CategoryFilter>,
    app_only: bool,
    env_only: bool,
) -> DetectionConfig {
    let mut config = DetectionConfig {
        max_depth,
        ignore_patterns: vec![
            ".git".to_string(),
            "node_modules".to_string(),
            "target".to_string(),
        ],
        enabled_categories: None,
        disabled_strategies: HashSet::new(),
        parallel: false,
        enable_workspace_detection: true,
        max_workspace_depth: 2,
    };

    // Add custom ignore patterns
    config.ignore_patterns.extend(ignore);

    // Build category filter
    if !categories.is_empty() {
        let enabled: HashSet<_> = categories
            .into_iter()
            .map(|c| c.to_strategy_category())
            .collect();
        config.enabled_categories = Some(enabled);
    } else if app_only || env_only {
        // Build category set based on app_only/env_only
        let mut enabled = HashSet::new();

        if app_only {
            enabled.insert(StrategyCategory::AppType(AppTypeCategory::Language));
            enabled.insert(StrategyCategory::AppType(AppTypeCategory::Framework));
            enabled.insert(StrategyCategory::AppType(AppTypeCategory::Monorepo));
            enabled.insert(StrategyCategory::AppType(AppTypeCategory::Service));
            enabled.insert(StrategyCategory::AppType(AppTypeCategory::Database));
        }

        if env_only {
            enabled.insert(StrategyCategory::EnvCapability(EnvCapabilityCategory::Local));
            enabled.insert(StrategyCategory::EnvCapability(EnvCapabilityCategory::Docker));
            enabled.insert(StrategyCategory::EnvCapability(EnvCapabilityCategory::OrbStack));
            enabled.insert(StrategyCategory::EnvCapability(EnvCapabilityCategory::Kubernetes));
        }

        config.enabled_categories = Some(enabled);
    }

    config
}

fn output_report(report: &DetectionReport, format: &OutputFormat, min_confidence: f32) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(report)?);
        }
        OutputFormat::JsonPretty => {
            println!("{}", serde_json::to_string_pretty(report)?);
        }
        OutputFormat::Human => {
            output_report_human(report, min_confidence, 0)?;
        }
    }

    Ok(())
}

fn output_report_human(report: &DetectionReport, min_confidence: f32, depth: usize) -> Result<()> {
    let indent = "  ".repeat(depth);

    // Header
    if depth == 0 {
        println!("\n{}", "Detection Report".bold().cyan());
        println!("{} {}\n", "Path:".bold(), report.path.display());
    } else {
        println!("\n{}{}",
            indent,
            format!("Workspace: {}", report.path.display()).bold().yellow()
        );
    }

    let filtered: Vec<_> = report
        .results
        .iter()
        .filter(|r| r.confidence >= min_confidence)
        .collect();

    // App Types Section
    let app_types: Vec<_> = filtered
        .iter()
        .filter(|r| r.category.is_app_type())
        .collect();

    if !app_types.is_empty() {
        println!("{}{}",indent, "App Types:".bold().green());
        for result in app_types {
            print_result_indented(result, &indent);
        }
        println!();
    }

    // Environment Capabilities Section
    let env_caps: Vec<_> = filtered
        .iter()
        .filter(|r| r.category.is_env_capability())
        .collect();

    if !env_caps.is_empty() {
        println!("{}{}",indent, "Environment Capabilities:".bold().blue());
        for result in env_caps {
            print_result_indented(result, &indent);
        }
        println!();
    }

    if depth == 0 {
        // Summary only at root level
        if let Some(app_name) = report.app_name() {
            println!("{} {}", "Detected app name:".bold(), app_name.bright_yellow());
        }

        if let Some(primary) = report.primary_language() {
            println!(
                "{} {} (confidence: {:.0}%)",
                "Primary language:".bold(),
                primary.strategy_id.bright_yellow(),
                primary.confidence * 100.0
            );
        }

        if filtered.is_empty() && report.children.is_empty() {
            println!("{}", "No detections found".yellow());
        }
    }

    // Recursively print workspace children
    if !report.children.is_empty() {
        println!("{}{}",
            indent,
            format!("Workspaces: ({} found)", report.children.len()).bold().cyan()
        );
        for child in &report.children {
            output_report_human(child, min_confidence, depth + 1)?;
        }
    }

    Ok(())
}

fn print_result_indented(result: &app_detector::DetectionResult, indent: &str) {
    let confidence_color = if result.confidence >= 0.9 {
        "green"
    } else if result.confidence >= 0.5 {
        "yellow"
    } else {
        "red"
    };

    let confidence_str = format!("{:.0}%", result.confidence * 100.0);
    let confidence_colored = match confidence_color {
        "green" => confidence_str.green(),
        "yellow" => confidence_str.yellow(),
        _ => confidence_str.red(),
    };

    println!(
        "{}  {} {} {}",
        indent,
        "•".bright_black(),
        result.strategy_id.bright_white(),
        format!("({})", confidence_colored).dimmed()
    );

    // Show data summary based on type
    match &result.data {
        app_detector::DetectionData::Language(info) => {
            println!("{}    {} {}", indent, "Language:".dimmed(), info.name);
            if let Some(ref version) = info.version {
                println!("{}    {} {}", indent, "Version:".dimmed(), version);
            }
        }
        app_detector::DetectionData::Framework(info) => {
            println!("{}    {} {}", indent, "Framework:".dimmed(), info.name);
            if let Some(ref version) = info.version {
                println!("{}    {} {}", indent, "Version:".dimmed(), version);
            }
        }
        app_detector::DetectionData::Service(info) => {
            println!("{}    {} {}", indent, "Service:".dimmed(), info.name);
        }
        app_detector::DetectionData::Monorepo(info) => {
            println!("{}    {} {}", indent, "Tool:".dimmed(), info.tool);
            if !info.workspace_info.is_empty() {
                println!("{}    {} {}", indent, "Workspaces:".dimmed(), info.workspace_info.len());
            }
        }
        app_detector::DetectionData::LocalEnv(info) => {
            if !info.commands.is_empty() {
                println!("{}    {} {}", indent, "Commands:".dimmed(), info.commands.len());
            }
        }
        app_detector::DetectionData::DockerEnv(info) => {
            if !info.dockerfiles.is_empty() {
                println!("{}    {} {}", indent, "Dockerfiles:".dimmed(), info.dockerfiles.len());
            }
            if !info.commands.is_empty() {
                println!("{}    {} {}", indent, "Commands:".dimmed(), info.commands.len());
            }
        }
        app_detector::DetectionData::OrbStackEnv(info) => {
            if !info.commands.is_empty() {
                println!("{}    {} {}", indent, "Commands:".dimmed(), info.commands.len());
            }
        }
        app_detector::DetectionData::KubernetesEnv(info) => {
            if !info.commands.is_empty() {
                println!("{}    {} {}", indent, "Commands:".dimmed(), info.commands.len());
            }
        }
        _ => {}
    }
}

fn output_strategies(
    registry: &StrategyRegistry,
    category: Option<CategoryFilter>,
    format: &OutputFormat,
) -> Result<()> {
    let strategies = if let Some(cat) = category {
        registry.by_category(&cat.to_strategy_category())
    } else {
        registry.all()
    };

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let data: Vec<_> = strategies
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "id": s.id(),
                        "name": s.name(),
                        "category": format!("{:?}", s.category()),
                        "priority": s.priority(),
                        "depends_on": s.depends_on(),
                    })
                })
                .collect();

            if matches!(format, OutputFormat::JsonPretty) {
                println!("{}", serde_json::to_string_pretty(&data)?);
            } else {
                println!("{}", serde_json::to_string(&data)?);
            }
        }
        OutputFormat::Human => {
            println!("\n{}", "Available Detection Strategies".bold().cyan());
            println!("{} strategies found\n", strategies.len());

            for strategy in strategies {
                println!("{} {}", "•".bright_black(), strategy.id().bright_white());
                println!("  {} {}", "Name:".dimmed(), strategy.name());
                println!("  {} {:?}", "Category:".dimmed(), strategy.category());
                println!("  {} {}", "Priority:".dimmed(), strategy.priority());

                if !strategy.depends_on().is_empty() {
                    println!(
                        "  {} {}",
                        "Dependencies:".dimmed(),
                        strategy.depends_on().join(", ")
                    );
                }
                println!();
            }
        }
    }

    Ok(())
}

fn output_categories(format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let data = serde_json::json!({
                "app_types": [
                    "language", "framework", "monorepo", "build_tool", "package_manager",
                    "database", "service", "message_queue", "reverse_proxy", "cicd",
                    "testing", "linting"
                ],
                "env_capabilities": [
                    "local", "docker", "orbstack", "kubernetes", "docker_swarm",
                    "nomad", "cloud_platform", "infrastructure"
                ]
            });

            if matches!(format, OutputFormat::JsonPretty) {
                println!("{}", serde_json::to_string_pretty(&data)?);
            } else {
                println!("{}", serde_json::to_string(&data)?);
            }
        }
        OutputFormat::Human => {
            println!("\n{}", "Available Categories".bold().cyan());

            println!("\n{}", "App Type Categories:".bold().green());
            println!("  {} Language detection", "•".bright_black());
            println!("  {} Framework detection", "•".bright_black());
            println!("  {} Monorepo tools", "•".bright_black());
            println!("  {} Build tools", "•".bright_black());
            println!("  {} Package managers", "•".bright_black());
            println!("  {} Databases", "•".bright_black());
            println!("  {} Services", "•".bright_black());
            println!("  {} Message queues", "•".bright_black());
            println!("  {} Reverse proxies", "•".bright_black());
            println!("  {} CI/CD systems", "•".bright_black());
            println!("  {} Testing frameworks", "•".bright_black());
            println!("  {} Linters/formatters", "•".bright_black());

            println!("\n{}", "Environment Capability Categories:".bold().blue());
            println!("  {} Local development", "•".bright_black());
            println!("  {} Docker containers", "•".bright_black());
            println!("  {} OrbStack", "•".bright_black());
            println!("  {} Kubernetes", "•".bright_black());
            println!("  {} Docker Swarm", "•".bright_black());
            println!("  {} Nomad", "•".bright_black());
            println!("  {} Cloud platforms", "•".bright_black());
            println!("  {} Infrastructure as Code", "•".bright_black());
            println!();
        }
    }

    Ok(())
}

fn output_strategy_info(
    registry: &StrategyRegistry,
    strategy_id: &str,
    format: &OutputFormat,
) -> Result<()> {
    let strategy = registry
        .get(strategy_id)
        .with_context(|| format!("Strategy '{}' not found", strategy_id))?;

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let data = serde_json::json!({
                "id": strategy.id(),
                "name": strategy.name(),
                "category": format!("{:?}", strategy.category()),
                "priority": strategy.priority(),
                "depends_on": strategy.depends_on(),
                "conflicts_with": strategy.conflicts_with(),
            });

            if matches!(format, OutputFormat::JsonPretty) {
                println!("{}", serde_json::to_string_pretty(&data)?);
            } else {
                println!("{}", serde_json::to_string(&data)?);
            }
        }
        OutputFormat::Human => {
            println!("\n{}", "Strategy Information".bold().cyan());
            println!("{} {}", "ID:".bold(), strategy.id());
            println!("{} {}", "Name:".bold(), strategy.name());
            println!("{} {:?}", "Category:".bold(), strategy.category());
            println!("{} {}", "Priority:".bold(), strategy.priority());

            if !strategy.depends_on().is_empty() {
                println!(
                    "{} {}",
                    "Dependencies:".bold(),
                    strategy.depends_on().join(", ")
                );
            }

            if !strategy.conflicts_with().is_empty() {
                println!(
                    "{} {}",
                    "Conflicts with:".bold(),
                    strategy.conflicts_with().join(", ")
                );
            }
            println!();
        }
    }

    Ok(())
}
