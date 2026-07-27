//! Reading, solving, and comparing system designs held in a directory.
//!
//! These commands work against files rather than a server. A design is a
//! directory of YAML that belongs in the same repository as the system it
//! describes, so answering a capacity question is a local operation and can run
//! in the same continuous integration that builds the thing being designed.

use std::{collections::BTreeMap, path::PathBuf};

use clap::{Args, Subcommand};

use crate::system::{
    Bottleneck, ComponentId, Evaluation, EvaluationConfig, InterventionId, LoadedSystem,
    bottlenecks, compare, evaluate, evaluate_intervention, read_system,
};

use super::output::OutputFormat;

/// Commands over a system design directory.
#[derive(Debug, Args)]
pub(super) struct SystemArgs {
    #[command(subcommand)]
    command: SystemCommand,
}

#[derive(Debug, Subcommand)]
enum SystemCommand {
    /// Loads a design and reports what it contains without solving it.
    Check(DesignArgs),
    /// Solves a design and reports the quantities flowing through it.
    Solve(SolveArgs),
    /// Ranks the constraints a design is closest to exhausting.
    Bottlenecks(BottlenecksArgs),
    /// Weighs a proposed change against the design it would replace.
    Compare(CompareArgs),
    /// Lists the component types and behaviours available to a design.
    Catalogue(DesignArgs),
}

#[derive(Debug, Args)]
struct DesignArgs {
    /// Directory holding the design.
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
}

impl SolveOptions {
    fn config(&self) -> EvaluationConfig {
        EvaluationConfig {
            seed: self.seed,
            sample_count: self.samples.max(1),
            horizon: self.horizon.max(1),
            step: self.step,
            ..EvaluationConfig::default()
        }
    }
}

#[derive(Debug, Args)]
struct SolveArgs {
    #[command(flatten)]
    design: DesignArgs,
    #[command(flatten)]
    options: SolveOptions,
    /// Report only this component.
    #[arg(long)]
    component: Option<String>,
    /// Apply this intervention before solving.
    #[arg(long)]
    intervention: Option<String>,
}

#[derive(Debug, Args)]
struct BottlenecksArgs {
    #[command(flatten)]
    design: DesignArgs,
    #[command(flatten)]
    options: SolveOptions,
    /// Apply this intervention before ranking.
    #[arg(long)]
    intervention: Option<String>,
    /// Report only constraints that bind in at least one draw.
    #[arg(long)]
    binding: bool,
}

#[derive(Debug, Args)]
struct CompareArgs {
    #[command(flatten)]
    design: DesignArgs,
    #[command(flatten)]
    options: SolveOptions,
    /// The intervention to weigh.
    intervention: String,
}

/// Executes one system design command.
pub(super) fn run(args: SystemArgs, output: OutputFormat) -> Result<(), human_errors::Error> {
    let rendered = match args.command {
        SystemCommand::Check(args) => {
            let loaded = load(&args.directory)?;
            output.system_summary(&loaded)?
        }
        SystemCommand::Catalogue(args) => {
            let loaded = load(&args.directory)?;
            output.system_catalogue(&loaded)?
        }
        SystemCommand::Solve(args) => {
            let loaded = load(&args.design.directory)?;
            let evaluation = solve(&loaded, args.intervention.as_deref(), &args.options)?;
            output.system_channels(&evaluation, args.component.as_deref())?
        }
        SystemCommand::Bottlenecks(args) => {
            let loaded = load(&args.design.directory)?;
            let evaluation = solve(&loaded, args.intervention.as_deref(), &args.options)?;
            let mut ranked = rank(&loaded, &evaluation, &args.options)?;
            if args.binding {
                ranked.retain(Bottleneck::binds);
            }
            output.system_bottlenecks(&ranked)?
        }
        SystemCommand::Compare(args) => {
            let loaded = load(&args.design.directory)?;
            let comparison = compare(
                &loaded.model,
                &loaded.component_types,
                &InterventionId::new(args.intervention),
                args.options.config(),
            )
            .map_err(evaluation_error)?;
            output.system_comparison(&comparison)?
        }
    };
    println!("{rendered}");
    Ok(())
}

fn load(directory: &std::path::Path) -> Result<LoadedSystem, human_errors::Error> {
    read_system(directory).map_err(|error| {
        human_errors::user(
            format!(
                "The design in {} could not be read: {error}",
                directory.display()
            ),
            &[
                "Check that the directory holds a _system.yaml written by this version.",
                "Run `optimist system check <directory>` after fixing the file it names.",
            ],
        )
    })
}

fn solve(
    loaded: &LoadedSystem,
    intervention: Option<&str>,
    options: &SolveOptions,
) -> Result<Evaluation, human_errors::Error> {
    match intervention {
        Some(id) => evaluate_intervention(
            &loaded.model,
            &loaded.component_types,
            &InterventionId::new(id),
            options.config(),
        ),
        None => evaluate(&loaded.model, &loaded.component_types, options.config()),
    }
    .map_err(evaluation_error)
}

fn rank(
    loaded: &LoadedSystem,
    evaluation: &Evaluation,
    options: &SolveOptions,
) -> Result<Vec<Bottleneck>, human_errors::Error> {
    bottlenecks(
        &loaded.model,
        &loaded.component_types,
        evaluation.settled(),
        options.config(),
    )
    .map_err(evaluation_error)
}

fn evaluation_error(error: crate::system::EvaluationError) -> human_errors::Error {
    human_errors::user(
        format!("The design could not be solved: {error}"),
        &[
            "Check that every component supplies the properties its type declares.",
            "Run `optimist system check <directory>` to validate the design without solving it.",
        ],
    )
}

/// Groups a step's channels by component for rendering.
pub(super) fn channels<'a>(
    evaluation: &'a Evaluation,
    only: Option<&str>,
) -> Vec<(
    &'a ComponentId,
    &'a BTreeMap<String, crate::squiggle::Value>,
)> {
    evaluation
        .settled()
        .components
        .iter()
        .filter(|(id, _)| only.is_none_or(|only| id.as_str() == only))
        .map(|(id, state)| (id, &state.channels))
        .collect()
}
