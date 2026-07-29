//! Reading, solving, and comparing system designs held in a directory.
//!
//! These commands work against files rather than a server. A design is a
//! directory of YAML that belongs in the same repository as the system it
//! describes, so answering a capacity question is a local operation and can run
//! in the same continuous integration that builds the thing being designed.
//!
//! Every command takes the design directory as its first argument and defaults
//! to the working directory, so somebody standing in a design can ask anything
//! of it without naming it again.

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use crate::system::{
    Bottleneck, Comparison, Evaluation, EvaluationConfig, InterventionId, LoadedSystem, Solve,
    SolveMode, bottlenecks_with_mutators, read_system,
};

use super::{diagnose, output::OutputFormat};

/// The things one can ask of a design held in a directory.
#[derive(Debug, Subcommand)]
pub(super) enum SystemCommand {
    /// Loads a design, looks it over, and reports anything wrong with it.
    Check(CheckArgs),
    /// Lists the component types and behaviours available to a design.
    Catalogue(CatalogueArgs),
    /// Solves a design and reports the quantities flowing through it.
    Solve(SolveArgs),
    /// Ranks the constraints a design is closest to exhausting.
    Bottlenecks(BottlenecksArgs),
    /// Weighs proposed changes against the design they would replace.
    Compare(CompareArgs),
}

/// The design a command works against.
#[derive(Debug, Args)]
struct DesignArgs {
    /// Directory holding the design.
    #[arg(default_value = ".")]
    directory: PathBuf,
}

/// Controls shared by every command that solves a design.
#[derive(Debug, Args)]
struct SolveOptions {
    /// Root of the deterministic random stream.
    #[arg(long, default_value_t = 0)]
    seed: u64,
    /// Draws carried through every uncertain quantity.
    #[arg(long, default_value_t = 1_000)]
    samples: usize,
    /// Number of steps to advance.
    #[arg(long, default_value_t = 1)]
    horizon: usize,
    /// Length of one step, in seconds.
    #[arg(long, default_value_t = 1.0)]
    step: f64,
    /// Advance queues through time rather than solving for where they balance.
    ///
    /// Gives a design memory, so a queue filled by a surge has to drain before
    /// the design recovers. Faithful only while the step is short against the
    /// time a queue takes to empty, so expect to shorten `--step` and lengthen
    /// `--horizon` together.
    #[arg(long)]
    transient: bool,
    /// Divide the draws this many ways and solve them at once.
    ///
    /// Each draw settles independently, so the shares are one answer computed in
    /// pieces. Left at one because dividing is not free: every share repeats the
    /// per-pass work that does not depend on the draw count, and a design with
    /// more than one resting state can send a draw to a different branch
    /// depending on which share it was solved in.
    #[arg(long, default_value_t = 1)]
    threads: usize,
}

impl SolveOptions {
    fn config(&self) -> EvaluationConfig {
        EvaluationConfig {
            seed: self.seed,
            sample_count: self.samples.max(1),
            horizon: self.horizon.max(1),
            step: self.step,
            threads: self.threads.max(1),
            mode: if self.transient {
                SolveMode::Transient
            } else {
                SolveMode::Steady
            },
            ..EvaluationConfig::default()
        }
    }
}

#[derive(Debug, Args)]
pub(super) struct CheckArgs {
    #[command(flatten)]
    design: DesignArgs,
    /// Check the structure only, without trying to solve the design.
    #[arg(long)]
    no_solve: bool,
}

#[derive(Debug, Args)]
pub(super) struct CatalogueArgs {
    #[command(flatten)]
    design: DesignArgs,
    /// Describe one component type or behaviour in full.
    #[arg(long = "type", value_name = "ID")]
    definition: Option<String>,
}

#[derive(Debug, Args)]
pub(super) struct SolveArgs {
    #[command(flatten)]
    design: DesignArgs,
    #[command(flatten)]
    options: SolveOptions,
    /// Report only this component.
    #[arg(long, short)]
    component: Option<String>,
    /// Apply this intervention before solving.
    #[arg(long, short)]
    intervention: Option<String>,
}

#[derive(Debug, Args)]
pub(super) struct BottlenecksArgs {
    #[command(flatten)]
    design: DesignArgs,
    #[command(flatten)]
    options: SolveOptions,
    /// Rank only this component's constraints.
    #[arg(long, short)]
    component: Option<String>,
    /// Apply this intervention before ranking.
    #[arg(long, short)]
    intervention: Option<String>,
    /// Report only constraints that bind in at least one draw.
    #[arg(long)]
    binding: bool,
}

/// Weighing proposals needs the design named, because the arguments after it
/// are interventions and one of them would otherwise be read as the directory.
#[derive(Debug, Args)]
pub(super) struct CompareArgs {
    /// Directory holding the design.
    directory: PathBuf,
    #[command(flatten)]
    options: SolveOptions,
    /// The interventions to weigh, each against the unchanged design.
    #[arg(required = true)]
    interventions: Vec<String>,
}

/// Executes one design command.
pub(super) fn run(command: SystemCommand, output: OutputFormat) -> Result<(), human_errors::Error> {
    match command {
        SystemCommand::Check(args) => check(args, output),
        SystemCommand::Catalogue(args) => {
            let loaded = load(&args.design.directory)?;
            print(output.catalogue(&loaded, args.definition.as_deref())?)
        }
        SystemCommand::Solve(args) => {
            let loaded = load(&args.design.directory)?;
            let evaluation = solve(&loaded, args.intervention.as_deref(), &args.options)?;
            print(output.solved(&evaluation, args.component.as_deref())?)
        }
        SystemCommand::Bottlenecks(args) => {
            let loaded = load(&args.design.directory)?;
            let evaluation = solve(&loaded, args.intervention.as_deref(), &args.options)?;
            let mut ranked = rank(&loaded, &evaluation, &args.options)?;
            if args.binding {
                ranked.retain(Bottleneck::binds);
            }
            if let Some(component) = &args.component {
                ranked.retain(|entry| entry.component.as_str() == component);
            }
            print(output.bottlenecks(&ranked)?)
        }
        SystemCommand::Compare(args) => {
            let loaded = load(&args.directory)?;
            let compared = compare(&loaded, &args.interventions, &args.options)?;
            print(output.comparison(&compared)?)
        }
    }
}

/// Reports what is wrong with a design, and fails when something is.
///
/// The findings go to standard output whatever the verdict, because somebody
/// reading them wants them the same way either way. Only the exit status
/// changes, which is what lets this stand as a step in continuous integration
/// without anybody having to grep the report for the word `error`.
fn check(args: CheckArgs, output: OutputFormat) -> Result<(), human_errors::Error> {
    let loaded = load(&args.design.directory)?;
    let findings = diagnose::findings(&loaded, !args.no_solve);
    print(output.check(&loaded, &findings)?)?;

    if diagnose::fatal(&findings) {
        return Err(human_errors::user(
            format!(
                "The design in {} has problems which stop it being solved.",
                args.design.directory.display()
            ),
            &[
                "Fix the findings marked `error` above; the advice beneath them says how.",
                "Run `optimist catalogue --type <ID>` to see what a component type expects.",
            ],
        ));
    }
    Ok(())
}

/// Writes a report to standard output.
///
/// A closed pipe is not a failure. `optimist bottlenecks | head` is a
/// reasonable thing to type, and a report that panics when the reader stops
/// reading is worse than one that stops writing.
fn print(rendered: String) -> Result<(), human_errors::Error> {
    use std::io::Write;

    let mut stdout = std::io::stdout().lock();
    match writeln!(stdout, "{rendered}") {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(human_errors::system(
            format!("The report could not be written: {error}"),
            &["Check that the destination this output was redirected to is writable."],
        )),
    }
}

fn load(directory: &Path) -> Result<LoadedSystem, human_errors::Error> {
    read_system(directory).map_err(|error| {
        human_errors::user(
            format!(
                "The design in {} could not be read: {error}",
                directory.display()
            ),
            &[
                "Check that the directory holds a _system.yaml written by this version.",
                "Run `optimist check <directory>` after fixing the file it names.",
            ],
        )
    })
}

fn solve(
    loaded: &LoadedSystem,
    intervention: Option<&str>,
    options: &SolveOptions,
) -> Result<Evaluation, human_errors::Error> {
    let asking = Solve::new(&loaded.model, &loaded.component_types)
        .mutators(&loaded.mutators)
        .with(options.config());
    match intervention {
        Some(id) => asking.intervention(&named(loaded, id)?).evaluate(),
        None => asking.evaluate(),
    }
    .map_err(evaluation_error)
}

/// Resolves an intervention, naming the ones a design has when it has no such one.
///
/// Interventions are referred to by identifier on the command line and by name
/// everywhere else, so a mistyped one is the easiest mistake to make here and
/// the least useful to be told about without the alternatives.
fn named(loaded: &LoadedSystem, id: &str) -> Result<InterventionId, human_errors::Error> {
    if loaded
        .model
        .interventions
        .iter()
        .any(|intervention| intervention.id.as_str() == id)
    {
        return Ok(InterventionId::new(id));
    }

    let available = loaded
        .model
        .interventions
        .iter()
        .map(|intervention| intervention.id.to_string())
        .collect::<Vec<_>>();
    let alternatives = if available.is_empty() {
        "It declares none at all.".to_owned()
    } else {
        format!("It declares: {}.", available.join(", "))
    };
    Err(human_errors::user(
        format!("This design has no intervention called `{id}`. {alternatives}"),
        &[
            "An intervention is named by its `id`, not by its human-readable `name`.",
            "Run `optimist check` to see every intervention a design carries.",
        ],
    ))
}

fn rank(
    loaded: &LoadedSystem,
    evaluation: &Evaluation,
    options: &SolveOptions,
) -> Result<Vec<Bottleneck>, human_errors::Error> {
    bottlenecks_with_mutators(
        &loaded.model,
        &loaded.component_types,
        &loaded.mutators,
        evaluation.settled(),
        options.config(),
    )
    .map_err(evaluation_error)
}

/// Weighs each proposal against the same unchanged design.
///
/// Every comparison uses one seed and one set of draws, so two proposals are as
/// comparable with each other as each is with the design they would replace.
fn compare(
    loaded: &LoadedSystem,
    interventions: &[String],
    options: &SolveOptions,
) -> Result<Vec<(String, Comparison)>, human_errors::Error> {
    let wanted = interventions
        .iter()
        .map(|intervention| named(loaded, intervention))
        .collect::<Result<Vec<_>, _>>()?;
    let weighed = Solve::new(&loaded.model, &loaded.component_types)
        .mutators(&loaded.mutators)
        .with(options.config())
        .compare_many(&wanted)
        .map_err(evaluation_error)?;
    Ok(interventions
        .iter()
        .cloned()
        .zip(weighed.into_iter().map(|(_, comparison)| comparison))
        .collect())
}

fn evaluation_error(error: crate::system::EvaluationError) -> human_errors::Error {
    human_errors::user(
        format!("The design could not be solved: {error}"),
        &[
            "Check that every component supplies the properties its type declares.",
            "Run `optimist check` to look the design over without solving it.",
        ],
    )
}
