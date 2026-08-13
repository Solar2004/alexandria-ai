//! alx — binario CLI de ALEXANDRIA.
//!
//! Subcomandos: `run <titulo>` (pipeline end-to-end con gates reales de
//! comandos y critic loop), `status` (fachada alx-lib), `task add/list` y
//! `--version`. El estado del DAG vive en memoria por invocación; la
//! persistencia a disco llega en fases posteriores.

use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};

use alx_cli::{
    agents_run_parallel, agents_show, check_network, feature_run, iterate_next, log_command,
    load_tasks_from_jsonl, persist_task_to_jsonl, render_agents, render_build, render_cost_report,
    render_doctor,
    render_iterate_state, render_metrics, render_network, render_night_report,
    render_phalanx_status, render_real_run, render_report, render_run, render_tui, render_weekly,
    run_evolve_cycle, run_pipeline, run_pipeline_real, serve_mcp_stdio, spawn_agent, verify_build,
    AppState,
};
use alx_core::types::{now_ms, PhaseId, Task};
use alx_lib::Alexandria;

/// ALEXANDRIA — motor de desarrollo IA autónomo.
#[derive(Parser)]
#[command(
    name = "alexandria",
    version,
    about = "Motor de desarrollo IA autónomo"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Ejecuta el pipeline de demo end-to-end (task → DAG → descomposición → harness).
    Run {
        /// Título de la tarea demo.
        titulo: String,
        /// Llama a la cadena real (headroom→mask→routatic) con ledger de coste.
        #[arg(long)]
        real: bool,
    },
    /// Estado actual del sistema (fachada alx-lib).
    Status,
    /// Comprueba la red real del governor (headroom→mask→routatic, fallback omniroute).
    Network,
    /// Dogfood: verifica el build del workspace con un gate real (cargo build).
    Build,
    /// Informe nocturno desde el DAG.
    Night,
    /// Sirve el protocolo MCP JSON-RPC por stdio.
    Mcp,
    /// Estado del plugin PHALANX (config + hooks).
    Phalanx,
    /// Dogfood: ejecuta el pipeline y escribe el artefacto real en docs/features/.
    Feature {
        /// Título de la feature.
        titulo: String,
        /// Usa la cadena real (critic + ledger + must-checks).
        #[arg(long)]
        real: bool,
    },
    /// Ciclo watcher de harnesses evolutivos con persistencia.
    Evolve,
    /// Doctor del ecosistema ALEXANDRIA (crates, hooks, harnesses).
    Doctor,
    /// Cost-report del governor desde el ledger persistido.
    Cost,
    /// Agentes del registry + envelope de spawn (alx-agents).
    Agents,
    /// Muestra un agente real del ecosistema por nombre.
    AgentsShow {
        /// Nombre del agente.
        name: String,
    },
    /// TUI dashboard del motor (estado, red, coste, comandos).
    Tui,
    /// Reporte completo del motor (markdown): TUI + coste + doctor + agentes.
    Report,
    /// Spawn de agentes headless en paralelo sobre una tarea.
    AgentsRun {
        /// Tarea que ejecutan los agentes en paralelo.
        task: String,
    },
    /// Métricas por crate (líneas de código).
    Metrics,
    /// Resumen semanal (coste, telemetría, harnesses, métricas).
    Weekly,
    /// Loop de iteración gestionado por el motor (sin auto-continue bash).
    Iterate {
        /// Avanza una iteración en state.toml.
        #[arg(long)]
        next: bool,
    },
    /// Spawn REAL de un agente contra la cadena (headless).
    Spawn {
        /// Nombre del agente (general-purpose, code-reviewer, test-engineer).
        name: String,
        /// Tarea que ejecuta el agente.
        task: String,
    },
    /// Gestiona tareas del DAG (en memoria).
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
}

#[derive(Debug, Subcommand)]
enum TaskCommand {
    /// Crea una tarea nueva.
    Add {
        /// Título de la tarea.
        title: String,
        /// Fase del pipeline (Ingest, Spec, Plan, Build, Test, Review, Docs, Ship).
        #[arg(long, default_value = "Build")]
        phase: String,
    },
    /// Lista las tareas registradas.
    List,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> ExitCode {
    let mut app = AppState::new();
    let cmd_name = format!("{:?}", cli.command)
        .split_whitespace()
        .next()
        .unwrap_or("?")
        .to_string();
    log_command(&cmd_name);
    match cli.command {
        None => print_help(),
        Some(Command::Run { titulo, real }) => {
            if real {
                let result = run_pipeline_real(&titulo);
                println!("{}", render_real_run(&result));
            } else {
                let result = run_pipeline(&titulo);
                println!("{}", render_run(&result));
            }
        }
        Some(Command::Status) => {
            let alex = Alexandria::new();
            println!("{}", alex.status());
        }
        Some(Command::Network) => {
            let statuses = check_network();
            println!("{}", render_network(&statuses));
        }
        Some(Command::Build) => {
            let evidence = verify_build();
            println!("{}", render_build(&evidence));
            if !evidence.passed {
                return ExitCode::from(1);
            }
        }
        Some(Command::Night) => {
            println!("{}", render_night_report());
        }
        Some(Command::Mcp) => {
            return ExitCode::from(serve_mcp_stdio() as u8);
        }
        Some(Command::Phalanx) => {
            println!("{}", render_phalanx_status());
        }
        Some(Command::Feature { titulo, real }) => {
            println!("{}", feature_run(&titulo, real, "docs/features"));
        }
        Some(Command::Evolve) => {
            println!("{}", run_evolve_cycle());
        }
        Some(Command::Doctor) => {
            println!("{}", render_doctor());
        }
        Some(Command::Cost) => {
            println!("{}", render_cost_report());
        }
        Some(Command::Agents) => {
            println!("{}", render_agents());
        }
        Some(Command::AgentsShow { name }) => {
            println!("{}", agents_show(&name));
        }
        Some(Command::Spawn { name, task }) => {
            println!("{}", spawn_agent(&name, &task));
        }
        Some(Command::Tui) => {
            println!("{}", render_tui());
        }
        Some(Command::Report) => {
            println!("{}", render_report());
        }
        Some(Command::AgentsRun { task }) => {
            println!("{}", agents_run_parallel(&task));
        }
        Some(Command::Metrics) => {
            println!("{}", render_metrics());
        }
        Some(Command::Weekly) => {
            println!("{}", render_weekly());
        }
        Some(Command::Iterate { next }) => {
            if next {
                println!("{}", iterate_next());
            } else {
                println!("{}", render_iterate_state());
            }
        }
        Some(Command::Task { command }) => match command {
            TaskCommand::Add { title, phase } => {
                let Some(phase_id) = parse_phase(&phase) else {
                    eprintln!(
                        "fase inválida: {phase} — usa una de: {}",
                        PhaseId::ALL.map(|p| p.as_str()).join(", ")
                    );
                    return ExitCode::from(2);
                };
                let id = format!("t-{}", app.task_count() + 1);
                let task = Task::new(id, title, phase_id, 15_000, now_ms());
                app.add_task(task.clone());
                let _ = persist_task_to_jsonl(&task);
                println!("Tarea creada: fase {} (persistida)", phase_id.as_str());
            }
            TaskCommand::List => {
                let tasks = load_tasks_from_jsonl();
                if tasks.is_empty() {
                    println!("(sin tareas persistidas)");
                } else {
                    for t in &tasks {
                        println!(
                            "{} | {} | {:?} | {}",
                            t.id,
                            t.title,
                            t.status,
                            t.phase.as_str()
                        );
                    }
                }
            }
        },
    }
    ExitCode::SUCCESS
}

fn parse_phase(s: &str) -> Option<PhaseId> {
    PhaseId::ALL
        .into_iter()
        .find(|p| p.as_str().eq_ignore_ascii_case(s))
}

fn print_help() {
    let mut cmd = Cli::command();
    let _ = cmd.print_help();
}
