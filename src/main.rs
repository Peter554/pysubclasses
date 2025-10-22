//! CLI tool for finding Python subclasses.

use anyhow::{Context, Result};
use clap::Parser;
use pysubclasses::{ClassReference, SubclassFinder};
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

    /// Output format
    #[arg(short, long, value_enum, default_value = "text")]
    format: OutputFormat,

    /// Show additional information
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    /// Human-readable text output
    Text,
    /// JSON output
    Json,
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
    let args = Args::parse();

    // Canonicalize the directory path
    let root_dir = args
        .directory
        .canonicalize()
        .with_context(|| format!("Failed to access directory: {}", args.directory.display()))?;

    if args.verbose {
        eprintln!("Searching for Python files in: {}", root_dir.display());
    }

    // Create the finder (this parses all Python files)
    let finder = SubclassFinder::new(root_dir).context("Failed to analyze codebase")?;

    if args.verbose {
        eprintln!("Found {} classes in codebase", finder.class_count());
        eprintln!(
            "Searching for subclasses of '{}'{}\n",
            args.class_name,
            args.module
                .as_ref()
                .map(|m| format!(" in module '{m}'"))
                .unwrap_or_default()
        );
    }

    // Find subclasses
    let subclasses = finder
        .find_subclasses(&args.class_name, args.module.as_deref())
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
        OutputFormat::Text => output_text(&args.class_name, &subclasses, args.verbose),
        OutputFormat::Json => output_json(&args.class_name, &args.module, &subclasses)?,
    }

    Ok(())
}

fn output_text(class_name: &str, subclasses: &[ClassReference], verbose: bool) {
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
        if verbose {
            println!(
                "  {} ({})\n    └─ {}",
                class_ref.class_name,
                class_ref.module_path,
                class_ref.file_path.display()
            );
        } else {
            println!("  {} ({})", class_ref.class_name, class_ref.module_path);
        }
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
