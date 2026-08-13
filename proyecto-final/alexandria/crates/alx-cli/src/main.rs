//! alx — binario CLI de ALEXANDRIA.
//!
//! Subcomandos: `run <titulo>` (pipeline end-to-end con gates reales de
//! comandos y critic loop), `status` (fachada alx-lib), `task add/list` y
//! `--version`. El estado del DAG vive en memoria por invocación; la
//! persistencia a disco llega en fases posteriores.

use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};

use alx_cli::{
    check_network, render_build, render_network, render_run, run_pipeline, verify_build, AppState,
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

#[derive(Subcommand)]
enum Command {
    /// Ejecuta el pipeline de demo end-to-end (task → DAG → descomposición → harness).
    Run {
        /// Título de la tarea demo.
        titulo: String,
    },
    /// Estado actual del sistema (fachada alx-lib).
    Status,
    /// Comprueba la red real del governor (headroom→mask→routatic, fallback omniroute).
    Network,
    /// Dogfood: verifica el build del workspace con un gate real (cargo build).
    Build,
    /// Gestiona tareas del DAG (en memoria).
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
}

#[derive(Subcommand)]
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
    match cli.command {
        None => print_help(),
        Some(Command::Run { titulo }) => {
            let result = run_pipeline(&titulo);
            println!("{}", render_run(&result));
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
                app.add_task(task);
                println!("Tarea creada: fase {}", phase_id.as_str());
            }
            TaskCommand::List => {
                if app.task_count() == 0 {
                    println!("(sin tareas)");
                } else {
                    for t in app.tasks() {
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
