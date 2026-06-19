use std::io::{Read, Write};
use std::path::Path;

use clap::{Parser, Subcommand};
use miette::NamedSource;

#[derive(Parser)]
#[command(name = "nemo", about = "NemoIR DSL Frontend CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Check {
        file: String,
    },
    Compile {
        file: String,

        #[arg(long, default_value = "none")]
        target: String,

        #[arg(short = 'o', long)]
        output: Option<String>,

        #[arg(long)]
        dump_ir: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Check { file } => {
            let (source, display_name) = read_input(&file)?;
            match nemoir_dsl_fe::check(&source, &display_name) {
                Ok(()) => println!("OK: {}", display_name),
                Err(diag) => {
                    print_diagnostic(diag, &display_name, source);
                    std::process::exit(1);
                }
            }
        }
        Command::Compile {
            file,
            target,
            output,
            dump_ir,
        } => {
            let (source, display_name) = read_input(&file)?;

            let ir = nemoir_dsl_fe::lower(&source, &display_name).unwrap_or_else(|diag| {
                print_diagnostic(diag, &display_name, source);
                std::process::exit(1);
            });

            if let Err(errors) = nemoir_ir::validate::validate(&ir) {
                for e in &errors.errors {
                    eprintln!("ir-validation-error: {}", e);
                }
                eprintln!(
                    "error: IR validation failed with {} error(s)",
                    errors.errors.len()
                );
                std::process::exit(1);
            }

            if dump_ir {
                let yaml_str = serde_yaml::to_string(&ir).unwrap_or_else(|e| {
                    eprintln!("error: YAML serialization failed: {}", e);
                    std::process::exit(1);
                });
                std::io::stdout().write_all(yaml_str.as_bytes())?;
            }

            match target.as_str() {
                "none" => {
                    if !dump_ir {
                        eprintln!("IR validated successfully (no target artifact generated)");
                    }
                }
                "visualizer" => {
                    if file == "-" && output.is_none() {
                        eprintln!("error: stdin input with visualizer target requires --output/-o");
                        std::process::exit(1);
                    }
                    let out_path = match output {
                        Some(ref p) => p.clone(),
                        None => {
                            let stem = Path::new(&file)
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("output");
                            format!("{}.html", stem)
                        }
                    };

                    let html = nemoir_backend_visualizer::render_html(
                        &ir,
                        &nemoir_backend_visualizer::VisualizerOptions::default(),
                    )
                    .unwrap_or_else(|e| {
                        eprintln!("error: visualizer backend failed: {}", e);
                        std::process::exit(1);
                    });

                    std::fs::write(&out_path, html).unwrap_or_else(|e| {
                        eprintln!("error: failed to write output file '{}': {}", out_path, e);
                        std::process::exit(1);
                    });

                    if !dump_ir {
                        eprintln!("wrote: {}", out_path);
                    }
                }
                "python" => {
                    if file == "-" && output.is_none() {
                        eprintln!("error: stdin input with python target requires --output/-o");
                        std::process::exit(1);
                    }
                    let out_dir = match output {
                        Some(ref p) => p.clone(),
                        None => {
                            let parent = Path::new(&file)
                                .parent()
                                .filter(|p| !p.as_os_str().is_empty())
                                .map(|p| p.to_path_buf())
                                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                            parent.to_string_lossy().into_owned()
                        }
                    };

                    let generated = nemoir_backend_python::generate_package(
                        &ir,
                        &nemoir_backend_python::PythonBackendOptions::default(),
                    )
                    .unwrap_or_else(|e| {
                        eprintln!("error: python backend failed: {}", e);
                        std::process::exit(1);
                    });

                    std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| {
                        eprintln!(
                            "error: failed to create output directory '{}': {}",
                            out_dir, e
                        );
                        std::process::exit(1);
                    });

                    let package_root = Path::new(&out_dir).join(&generated.package_name);
                    std::fs::create_dir_all(&package_root).unwrap_or_else(|e| {
                        eprintln!(
                            "error: failed to create package directory '{}': {}",
                            package_root.display(),
                            e
                        );
                        std::process::exit(1);
                    });

                    for file in &generated.files {
                        let target = Path::new(&out_dir).join(&file.relative_path);
                        if let Some(parent) = target.parent() {
                            std::fs::create_dir_all(parent).ok();
                        }
                        std::fs::write(&target, &file.content).unwrap_or_else(|e| {
                            eprintln!("error: failed to write '{}': {}", target.display(), e);
                            std::process::exit(1);
                        });
                    }

                    if !dump_ir {
                        eprintln!("wrote: {}", package_root.display());
                    }
                }
                other => {
                    eprintln!("error: unknown compile target `{}`", other);
                    eprintln!("supported targets: none, visualizer, python");
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}

fn read_input(path: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    if path == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok((buf, "<stdin>".into()))
    } else {
        let source = std::fs::read_to_string(path)?;
        Ok((source, path.into()))
    }
}

fn print_diagnostic(diag: nemoir_dsl_fe::Diagnostic, filename: &str, source: String) {
    let report = miette::Report::new(diag).with_source_code(NamedSource::new(filename, source));
    eprintln!("{:?}", report);
}
