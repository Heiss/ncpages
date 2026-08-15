//! ncpages — watch a Nextcloud folder, build a static site when it changes,
//! publish it atomically, and serve it.
//!
//! One binary, several roles. `run` is the homelab default: watcher, scheduler
//! and HTTP server in one process. `watch` and `serve` split them into separate
//! containers for deployments that want the site to survive a watcher crash.
//! `build-agent` is the builder side, which holds the build tools and neither
//! credentials nor network egress.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::info;

use ncpages::config::Config;
use ncpages::serve::{Health, SharedHealth};
use ncpages::{agent, doctor, pipeline, publish, scheduler, serve, source, state};

#[derive(Parser)]
#[command(name = "ncpages", version, about, long_about = None)]
struct Cli {
    /// Path to ncpages.toml. Lives in the read-only config directory, never in
    /// the watched folder.
    #[arg(
        long,
        short,
        global = true,
        default_value = "/etc/ncpages/ncpages.toml"
    )]
    config: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Watch, build, publish and serve in one process.
    Run,
    /// Watch, build and publish, without serving.
    Watch,
    /// Serve the current release only.
    Serve,
    /// Run the build endpoint for the isolated builder container.
    BuildAgent {
        #[arg(long, default_value = "0.0.0.0:8080")]
        listen: String,
    },
    /// Run one build now, regardless of whether anything changed.
    Build,
    /// Check this deployment and report what is wrong with it.
    Doctor,
    /// Parse and validate the configuration, then exit.
    Check,
}

/// `NCPAGES_LOG` takes a level name. A regex-based directive filter would be
/// nicer and costs about a megabyte of binary for a service with nine modules.
fn log_level() -> tracing::Level {
    match std::env::var("NCPAGES_LOG")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(log_level())
        .with_target(false)
        .init();

    let cli = Cli::parse();
    let config = Arc::new(
        Config::load(&cli.config).with_context(|| format!("loading {}", cli.config.display()))?,
    );

    match cli.command {
        Command::Check => {
            println!(
                "{}: valid (schema_version {})",
                cli.config.display(),
                config.schema_version
            );
            Ok(())
        }

        Command::Doctor => {
            let checks = doctor::run(config).await?;
            let worst = doctor::print(&checks);
            if worst == doctor::Level::Fail {
                std::process::exit(1);
            }
            Ok(())
        }

        Command::Build => {
            let source = source::Source::from_config(&config)?;
            let mut state = state::State::load(&config.paths.state)?;
            publish::ensure_bootstrap(&config.publish.root)?;
            let outcome = pipeline::run_once(config.clone(), &source, &mut state, "manual").await?;
            state.last_result = Some(outcome.summary());
            state.save(&config.paths.state)?;
            println!("{}", outcome.summary());
            for warning in &outcome.warnings {
                println!("warning: {warning}");
            }
            for conflict in &outcome.conflict_copies {
                println!("conflict copy excluded: {conflict}");
            }
            if !outcome.published && !outcome.skipped {
                std::process::exit(1);
            }
            Ok(())
        }

        Command::BuildAgent { listen } => agent::run(config, listen).await,

        Command::Serve => serve::serve_site(config).await,

        Command::Watch => {
            let health: SharedHealth = Arc::new(tokio::sync::RwLock::new(Health::default()));
            let shutdown = scheduler::ShutdownSignal::install();
            let health_server = tokio::spawn(serve::serve_health(config.clone(), health.clone()));
            let result = scheduler::run(config, health, shutdown).await;
            health_server.abort();
            result
        }

        Command::Run => {
            let health: SharedHealth = Arc::new(tokio::sync::RwLock::new(Health::default()));
            let shutdown = scheduler::ShutdownSignal::install();

            // The site is served from the moment the process starts, including
            // during the first sync — hence the bootstrap holding page.
            publish::ensure_bootstrap(&config.publish.root)?;

            let site = config
                .serve
                .enabled
                .then(|| tokio::spawn(serve::serve_site(config.clone())));
            let health_server = tokio::spawn(serve::serve_health(config.clone(), health.clone()));

            info!("ncpages running: watcher and server in one process");
            let result = scheduler::run(config, health, shutdown).await;

            if let Some(site) = site {
                site.abort();
            }
            health_server.abort();
            result
        }
    }
}
