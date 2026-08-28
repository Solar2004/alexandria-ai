//! alx — binario CLI de ALEXANDRIA.
//!
//! Subcomandos: `run <titulo>` (pipeline end-to-end con gates reales de
//! comandos y critic loop), `status` (fachada alx-lib), `task add/list` y
//! `--version`. El estado del DAG vive en memoria por invocación; la
//! persistencia a disco llega en fases posteriores.

use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};

use alx_cli::{
    agents_run_parallel, agents_show, check_network, classify_from_stdin, docmin_check,
    evolve_detect_from_stdin, feature_run, harness_list, harness_new, harness_rm, harness_update,
    harness_use, iterate_next, log_command, load_tasks_from_jsonl,
    memory_capture_from_stdin, mission_print, persist_task_to_jsonl, render_agents,
    render_bench_all, render_benchmark, render_bench_bigcode, render_bench_codecontests,
    render_bench_humaneval, render_build, render_cost_report, render_doctor, render_quality,
    run_lsp_doctor, render_iterate_state, render_metrics, render_network, render_night_report,
    render_phalanx_status, render_real_run, render_report, render_run, render_tui,
    render_weekly, run_evolve_cycle, run_lsp_check, run_phalanx_event, run_pipeline,
    run_pipeline_real, run_setup, run_update, serve_mcp_stdio, skills_sync,
    spawn_agent, verify_build, AppState,
};
use alx_core::types::{now_ms, PhaseId, Task};
use alx_lib::Alexandria;

/// Subcomandos del buzón A2A.
#[derive(Debug, Subcommand)]
enum MailCmd {
    /// Envía un mensaje al buzón de otra sesión.
    Send {
        /// Sesión destino (ALX_SESSION_ID del receptor, ej: "agent-build-1").
        to: String,
        /// Mensaje (resultado, aviso, bloqueo).
        msg: String,
    },
    /// Lee el buzón de ESTA sesión (ALX_SESSION_ID).
    Read {
        /// Vacía el buzón tras leer.
        #[arg(long)]
        clear: bool,
    },
}

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
    /// Dogfood: ejecuta el pipeline y escribe el artefacto real en artifacts/features/.
    Feature {
        /// Título de la feature.
        titulo: String,
        /// Usa la cadena real (critic + ledger + must-checks).
        #[arg(long)]
        real: bool,
    },
    /// Ciclo watcher de harnesses evolutivos con persistencia.
    Evolve,
    /// Alexadriza el proyecto actual: crea .alexandria/ (registry, rúbricas,
    /// skills y diario de lecciones propios de ESTE proyecto).
    Init,
    /// Pule un fichero contra la rúbrica del proyecto; el sistema DECIDE
    /// cuántas rondas según la mejora vista (meseta → parada).
    Polish {
        /// Fichero a pulir.
        path: String,
        /// Rúbrica en .alexandria/rubrics/<nombre>.json (default si falta).
        #[arg(long, default_value = "default")]
        rubric: String,
    },
    /// Detecta problemas recurrentes en las métricas y propone harnesses.
    Patterns {
        /// Crear los harnesses propuestos directamente.
        #[arg(long)]
        apply: bool,
    },
    /// Abre un proyecto de investigación profunda (protocolo plan/17):
    /// fundamentos → iceberg → simulaciones guardadas → frenos → evidencia.
    Research {
        /// La pregunta a investigar.
        pregunta: String,
    },
    /// Descarga un repo de skills/reglas al proyecto (.alexandria/skills/).
    /// Sin argumento muestra el catálogo curado.
    SkillsFetch {
        /// Repo GitHub "owner/repo".
        repo: Option<String>,
        /// Buscar en GitHub ordenado por estrellas en vez de instalar.
        #[arg(long)]
        search: Option<String>,
        /// Tras descargar, la IA escribe su propia versión del skill en frío
        /// y se compara: el DELTA mide el valor real (lo que la IA no genera sola).
        #[arg(long)]
        challenge: bool,
    },
    /// Challenge de skill ya descargada: baseline IA en frío vs skill externa.
    SkillsChallenge {
        /// Directorio de la skill (con SKILL.md) o nombre dentro de .alexandria/skills/.
        path: String,
    },
    /// Analiza la calidad FUNCIONAL de las skills bajo un directorio
    /// (scripts, comandos, librerías, gates) sin llamar al LLM.
    SkillsScore {
        /// Directorio raíz a analizar (recursivo, profundidad 3).
        path: String,
    },
    /// Comprueba que la investigación abierta cumple el protocolo (plan 17):
    /// 7 pasos rellenos, ≥2 simulaciones, tabla de evidencia. Exit 1 si no.
    ResearchCheck {
        /// Dir concreto del research (por defecto: el más reciente).
        dir: Option<String>,
    },
    /// Crea un harness (paso CREAR del ciclo R20-R23; la IA lo usa en pleno trabajo).
    HarnessNew {
        /// Nombre corto (sin prefijo hx-): "sin-todos-pendientes".
        name: String,
        /// Objetivo verificable. Temporal: su cumplimiento autodestruye el harness.
        #[arg(long)]
        objective: String,
        /// Documentación mínima obligatoria (>=20 chars): qué, por qué, cuándo.
        #[arg(long)]
        doc: String,
        /// temporal | permanent (default temporal).
        #[arg(long, default_value = "temporal")]
        kind: String,
        /// manual | phase:<Fase> | event:<Evento> (default manual).
        #[arg(long, default_value = "manual")]
        trigger: String,
    },
    /// Lista los harnesses del registry (estado, usos, trigger).
    HarnessList,
    /// Registra un uso del harness (alimenta la decisión promover/retirar).
    HarnessUse {
        /// Id del harness (hx-<slug>) o nombre.
        id: String,
    },
    /// Refine del Continual Harness: actualiza objetivo/doc/trigger/kind.
    HarnessUpdate {
        /// Id del harness (hx-<slug>) o nombre.
        id: String,
        /// Nuevo objetivo verificable.
        #[arg(long)]
        objective: Option<String>,
        /// Nueva doc-min (>=20 chars).
        #[arg(long)]
        doc: Option<String>,
        /// temporal | permanent.
        #[arg(long)]
        kind: Option<String>,
        /// manual | phase:<Fase> | event:<Evento>.
        #[arg(long)]
        trigger: Option<String>,
    },
    /// Elimina un harness del registry explícitamente.
    HarnessRm {
        /// Id del harness (hx-<slug>) o nombre.
        id: String,
    },
    /// Regenera skill-rules.json desde TODAS las fuentes de SKILL.md (preserva manuales).
    SkillsSync,
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
    /// Dashboard vivo de actividad: estados de Claude/agentes en tiempo real.
    Watch {
        /// Un solo snapshot sin loop (para scripts).
        #[arg(long)]
        once: bool,
    },
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
    /// Benchmark de desempeño del sistema contra expectativas.
    Quality,
    /// Benchmark de tareas complicadas de IA (5x mejor que directa).
    Benchmark,
    /// Benchmark REAL: problemas BigCodeBench (unittest) — directa vs harness.
    BenchBigcode,
    /// Benchmark HumanEval (164, familia 2 para generalidad).
    BenchHumaneval,
    /// Benchmark CodeContests (30, familia 3 I/O-based).
    BenchCodecontests,
    /// Ejecuta TODAS las familias de benchmark (BigCodeBench + HumanEval + CodeContests).
    Bench,
    /// Auto-actualización: git pull + rebuild + reinstall.
    Update,
    /// Spawn REAL de un agente contra la cadena (headless).
    Spawn {
        /// Nombre del agente (general-purpose, code-reviewer, test-engineer).
        name: String,
        /// Tarea que ejecuta el agente.
        task: String,
    },
    /// Configura e verifica toda la integración con Claude Code (statusline, MCP, hooks).
    Setup,
    /// Ejecuta los hooks PHALANX reales (phalanx/hooks/*.toml) del evento CC.
    /// En user-prompt-submit/session-start inyecta el stdout como contexto.
    Hook {
        /// Evento: user-prompt-submit | pre-tool-use | post-tool-use | session-start | stop.
        event: String,
    },
    /// Imprime la memoria maestra (MISSION.md) + reglas globales.
    Mission,
    /// Captura aprendizaje del payload del hook (stdin) → recall caveman.
    MemoryCapture,
    /// Detecta operaciones repetidas (stdin) → candidato a harness evolutivo.
    EvolveDetect,
    /// Regla doc-min real sobre un fichero: documentado (exit 0) o no (exit 1).
    Docmin {
        /// Fichero a verificar.
        file: String,
    },
    /// Clasifica el payload del hook (stdin) → tier de modelo del governor.
    Classify,
    /// Doctor LSP: servers detectados + versión; `--live` hace handshake real.
    Lsp {
        /// Handshake initialize REAL contra cada server detectado.
        #[arg(long)]
        live: bool,
    },
    /// Diagnostics LSP reales sobre ficheros (rust-analyzer/tsserver/pyright).
    LspCheck {
        /// Ficheros a verificar.
        files: Vec<String>,
    },
    /// Crea (o reutiliza) el harness temporal de una skill con sus pasos.
    SkillHarness {
        /// Nombre de la skill (como en la tool Skill).
        skill: String,
    },
    /// Marca el paso n (1-indexed) del harness de skill como hecho.
    HarnessStep {
        /// Id del harness (hx-skill-<slug>).
        id: String,
        /// Número de paso.
        step: usize,
    },
    /// Retira el harness de la skill (fin de unidad — se archiva).
    SkillHarnessDone {
        /// Id del harness.
        id: String,
    },
    /// Guard de skills para Stop: pasos marcados o bloquea (exit 2).
    SkillCheck,
    /// Ejecuta un script de una skill como módulo ejecutable, con evidencia.
    SkillRun {
        /// Nombre de la skill.
        skill: String,
        /// Nombre del script (relativo a scripts/ o a la raíz de la skill).
        script: String,
        /// Argumentos para el script.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Buzón A2A entre sesiones paralelas (state/mailbox/).
    Mail {
        #[command(subcommand)]
        cmd: MailCmd,
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
            println!("{}", alx_cli::render_status_persisted());
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
            println!("{}", feature_run(&titulo, real, "artifacts/features"));
        }
        Some(Command::Evolve) => {
            println!("{}", run_evolve_cycle());
        }
        Some(Command::Init) => {
            println!("{}", alx_cli::project_init());
        }
        Some(Command::Polish { path, rubric }) => {
            println!("{}", alx_cli::run_polish(&path, &rubric));
        }
        Some(Command::Patterns { apply }) => {
            println!("{}", alx_cli::run_patterns(apply));
        }
        Some(Command::Research { pregunta }) => {
            println!("{}", alx_cli::run_research(&pregunta));
        }
        Some(Command::SkillsFetch { repo, search, challenge }) => {
            let out = alx_cli::run_skills_fetch(repo.as_deref(), search.as_deref());
            println!("{out}");
            // challenge automático tras descargar: ¿aporta lo que la IA no genera?
            if challenge {
                if let Some(repo) = repo.as_deref() {
                    let name = repo.rsplit('/').next().unwrap_or(repo);
                    let dir = std::path::Path::new(".alexandria/skills").join(name);
                    println!("\n{}", alx_cli::run_skills_challenge(&dir));
                }
            }
        }
        Some(Command::SkillsChallenge { path }) => {
            let p = std::path::PathBuf::from(&path);
            let p = if p.exists() { p } else {
                std::path::Path::new(".alexandria/skills").join(&path)
            };
            println!("{}", alx_cli::run_skills_challenge(&p));
        }
        Some(Command::SkillsScore { path }) => {
            println!("{}", alx_cli::render_skills_score(std::path::Path::new(&path)));
        }
        Some(Command::ResearchCheck { dir }) => {
            let informe = alx_cli::run_research_check(dir.as_deref());
            let ok = informe.starts_with('✓');
            println!("{informe}");
            if !ok {
                return ExitCode::from(1);
            }
        }
        Some(Command::HarnessNew { name, objective, doc, kind, trigger }) => {
            println!("{}", harness_new(&name, &objective, &doc, &kind, &trigger));
        }
        Some(Command::HarnessList) => {
            println!("{}", harness_list());
        }
        Some(Command::HarnessUse { id }) => {
            println!("{}", harness_use(&id));
        }
        Some(Command::HarnessUpdate { id, objective, doc, kind, trigger }) => {
            println!(
                "{}",
                harness_update(&id, objective.as_deref(), doc.as_deref(), kind.as_deref(), trigger.as_deref())
            );
        }
        Some(Command::HarnessRm { id }) => {
            println!("{}", harness_rm(&id));
        }
        Some(Command::SkillsSync) => {
            println!("{}", skills_sync());
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
        Some(Command::Watch { once }) => {
            alx_cli::run_watch(once);
        }
        Some(Command::Tui) => {
            // Dashboard ratatui vivo (alx-tui); fallback ANSI si no hay TTY.
            if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
                return match alx_tui::main_tui() {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(e) => {
                        eprintln!("tui: {e}");
                        ExitCode::from(1)
                    }
                };
            }
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
        Some(Command::Quality) => {
            println!("{}", render_quality());
        }
        Some(Command::Benchmark) => {
            println!("{}", render_benchmark());
        }
        Some(Command::BenchBigcode) => {
            println!("{}", render_bench_bigcode());
        }
        Some(Command::BenchHumaneval) => {
            println!("{}", render_bench_humaneval());
        }
        Some(Command::BenchCodecontests) => {
            println!("{}", render_bench_codecontests());
        }
        Some(Command::Bench) => {
            println!("{}", render_bench_all());
        }
        Some(Command::Setup) => {
            println!("{}", run_setup());
        }
        Some(Command::Hook { event }) => {
            return ExitCode::from(run_phalanx_event(&event).clamp(0, 255) as u8);
        }
        Some(Command::Mission) => {
            println!("{}", mission_print());
        }
        Some(Command::MemoryCapture) => {
            let (msg, code) = memory_capture_from_stdin();
            if !msg.is_empty() {
                println!("{msg}");
            }
            return ExitCode::from(code as u8);
        }
        Some(Command::EvolveDetect) => {
            let (msg, code) = evolve_detect_from_stdin();
            if !msg.is_empty() {
                println!("{msg}");
            }
            return ExitCode::from(code as u8);
        }
        Some(Command::Docmin { file }) => {
            let (msg, code) = docmin_check(&file);
            println!("{msg}");
            return ExitCode::from(code as u8);
        }
        Some(Command::Classify) => {
            let (msg, code) = classify_from_stdin();
            println!("{msg}");
            return ExitCode::from(code as u8);
        }
        Some(Command::Lsp { live }) => {
            println!("{}", run_lsp_doctor(live));
        }
        Some(Command::LspCheck { files }) => {
            let (msg, code) = run_lsp_check(&files);
            println!("{msg}");
            if code != 0 {
                return ExitCode::from(code as u8);
            }
        }
        Some(Command::SkillHarness { skill }) => {
            println!("{}", alx_cli::skill_harness_ensure(&skill));
        }
        Some(Command::HarnessStep { id, step }) => {
            println!("{}", alx_cli::harness_step(&id, step));
        }
        Some(Command::SkillHarnessDone { id }) => {
            println!("{}", alx_cli::skill_harness_done(&id));
        }
        Some(Command::SkillCheck) => {
            let (msg, code) = alx_cli::skill_check();
            if !msg.is_empty() {
                eprintln!("{msg}");
            }
            return ExitCode::from(code as u8);
        }
        Some(Command::SkillRun { skill, script, args }) => {
            println!("{}", alx_cli::skill_run(&skill, &script, &args));
        }
        Some(Command::Mail { cmd }) => match cmd {
            MailCmd::Send { to, msg } => println!("{}", alx_cli::mail_send(&to, &msg)),
            MailCmd::Read { clear } => println!("{}", alx_cli::mail_read(clear)),
        },
        Some(Command::Update) => {
            println!("{}", run_update());
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
