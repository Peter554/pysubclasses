//! CLI tool for finding Python subclasses.

use anyhow::{Context, Result};
use clap::Parser;
use pysubclasses::{ClassReference, SearchMode, SubclassFinder};
use serde::Serialize;
use std::path::PathBuf;

/// Find all subclasses of a Python class
#[derive(Parser, Debug)]
#[command(
    name = "pysubclasses",
    version,
    about = "Find all subclasses of a Python class",
    long_about = "Recursively finds all subclasses (direct and transitive) of a given Python class within a codebase.\n\n\
                  Handles imports, re-exports, and ambiguous class names."
)]
struct Args {
    /// Name of the class to find subclasses for
    #[arg()]
    class_name: String,

    /// Dotted module path where the class is defined (e.g., 'foo.bar')
    ///
    /// Use this to disambiguate when the same class name appears in multiple modules.
    #[arg(short, long)]
    module: Option<String>,

    /// Root directory to search for Python files
    ///
    /// If not specified, uses the current directory.
    #[arg(short, long, default_value = ".")]
    directory: PathBuf,

    /// Exclude directories from analysis (can be specified multiple times)
    ///
    /// Paths can be relative to the search directory or absolute.
    /// Example: --exclude ./tests
    #[arg(short, long)]
    exclude: Vec<PathBuf>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "text")]
    format: OutputFormat,

    /// Subclass search mode
    #[arg(long, value_enum, default_value = "all")]
    mode: Mode,

    /// Disable cache (always parse all files)
    #[arg(long)]
    no_cache: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    /// Human-readable text output
    Text,
    /// JSON output
    Json,
    /// Graphviz dot format
    Dot,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum Mode {
    /// Find only direct subclasses
    Direct,
    /// Find all transitive subclasses
    All,
}

#[derive(Serialize)]
struct JsonOutput {
    class_name: String,
    module_path: Option<String>,
    subclasses: Vec<JsonClass>,
}

#[derive(Serialize)]
struct JsonClass {
    class_name: String,
    module_path: String,
    file_path: String,
}

fn main() -> Result<()> {
    // Initialize logger
    env_logger::init();

    let args = Args::parse();

    // Canonicalize the directory path
    let root_dir = args
        .directory
        .canonicalize()
        .with_context(|| format!("Failed to access directory: {}", args.directory.display()))?;

    log::debug!("Searching for Python files in: {}", root_dir.display());
    if !args.exclude.is_empty() {
        log::debug!("Excluding directories: {:?}", args.exclude);
    }
    if args.no_cache {
        log::debug!("Cache disabled");
    }

    // Create the finder (this parses all Python files)
    let finder = SubclassFinder::with_options(root_dir, args.exclude, !args.no_cache)
        .context("Failed to analyze codebase")?;

    log::debug!("Found {} classes in codebase", finder.class_count());
    log::debug!(
        "Searching for subclasses of '{}'{}",
        args.class_name,
        args.module
            .as_ref()
            .map(|m| format!(" in module '{m}'"))
            .unwrap_or_default()
    );

    // Convert CLI Mode to SearchMode
    let mode = match args.mode {
        Mode::Direct => SearchMode::Direct,
        Mode::All => SearchMode::All,
    };

    // Find subclasses
    let subclasses = finder
        .find_subclasses(&args.class_name, args.module.as_deref(), mode)
        .map_err(|e| match &e {
            pysubclasses::Error::AmbiguousClassName { name, candidates } => {
                let formatted_candidates = candidates
                    .iter()
                    .map(|c| format!("  - {c}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                anyhow::anyhow!(
                    "Class '{name}' found in multiple modules:\n{formatted_candidates}\n\nPlease specify --module to disambiguate."
                )
            }
            _ => anyhow::Error::from(e),
        })
        .context("Failed to find subclasses")?;

    // Output results
    match args.format {
        OutputFormat::Text => output_text(&args.class_name, &subclasses),
        OutputFormat::Json => output_json(&args.class_name, &args.module, &subclasses)?,
        OutputFormat::Dot => output_dot(&args.class_name, &args.module, &subclasses, &finder)?,
    }

    Ok(())
}

fn output_text(class_name: &str, subclasses: &[ClassReference]) {
    if subclasses.is_empty() {
        println!("No subclasses found for '{class_name}'");
        return;
    }

    println!(
        "Found {} subclass(es) of '{}':\n",
        subclasses.len(),
        class_name
    );

    for class_ref in subclasses {
        println!("  {} ({})", class_ref.class_name, class_ref.module_path);
    }
}

fn output_json(
    class_name: &str,
    module_path: &Option<String>,
    subclasses: &[ClassReference],
) -> Result<()> {
    let output = JsonOutput {
        class_name: class_name.to_string(),
        module_path: module_path.clone(),
        subclasses: subclasses
            .iter()
            .map(|c| JsonClass {
                class_name: c.class_name.clone(),
                module_path: c.module_path.clone(),
                file_path: c.file_path.display().to_string(),
            })
            .collect(),
    };

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn output_dot(
    class_name: &str,
    module_path: &Option<String>,
    subclasses: &[ClassReference],
    finder: &SubclassFinder,
) -> Result<()> {
    use std::collections::HashSet;

    // Resolve the actual base class
    let base_class = finder
        .resolve_class_reference(class_name, module_path.as_deref())
        .with_context(|| format!("Failed to resolve base class '{}'", class_name))?;

    // Create a set of all classes in our graph for quick lookup
    let mut class_set = HashSet::new();
    class_set.insert((
        base_class.class_name.clone(),
        base_class.module_path.clone(),
    ));
    for subclass in subclasses {
        class_set.insert((subclass.class_name.clone(), subclass.module_path.clone()));
    }

    println!("digraph {{");
    println!("  rankdir=TB;");
    println!("  node [shape=box, style=filled, fillcolor=lightblue];");
    println!();

    // Add base class node
    let base_node_id = format!(
        "{}_{}",
        sanitize_for_dot(&base_class.module_path),
        sanitize_for_dot(&base_class.class_name)
    );
    println!(
        "  {} [label=\"{}\\n({})\", fillcolor=lightgreen];",
        base_node_id, base_class.class_name, base_class.module_path
    );

    // Add subclass nodes
    for subclass in subclasses {
        let node_id = format!(
            "{}_{}",
            sanitize_for_dot(&subclass.module_path),
            sanitize_for_dot(&subclass.class_name)
        );
        println!(
            "  {} [label=\"{}\\n({})\"];",
            node_id, subclass.class_name, subclass.module_path
        );
    }

    println!();

    // Add edges
    for subclass in subclasses {
        let parents = finder
            .find_parent_classes(
                &subclass.class_name,
                Some(&subclass.module_path),
                SearchMode::Direct,
            )
            .unwrap_or_default();

        for parent in parents {
            // Only draw edge if parent is in our class set (either base or another subclass)
            if class_set.contains(&(parent.class_name.clone(), parent.module_path.clone())) {
                let parent_node_id = format!(
                    "{}_{}",
                    sanitize_for_dot(&parent.module_path),
                    sanitize_for_dot(&parent.class_name)
                );
                let child_node_id = format!(
                    "{}_{}",
                    sanitize_for_dot(&subclass.module_path),
                    sanitize_for_dot(&subclass.class_name)
                );
                println!("  {} -> {};", parent_node_id, child_node_id);
            }
        }
    }

    println!("}}");
    Ok(())
}

fn sanitize_for_dot(s: &str) -> String {
    s.replace(['.', '-'], "_")
}
