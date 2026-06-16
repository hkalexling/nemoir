use std::io::{Read, Write};

use clap::{Parser, Subcommand};
use miette::NamedSource;

#[derive(Parser)]
#[command(name = "nemoir-dsl", about = "NemoIR DSL Frontend CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Check {
        file: String,
    },
    Lower {
        file: String,
        #[arg(short = 'o', long)]
        output: Option<String>,
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
        Command::Lower { file, output } => {
            let (source, display_name) = read_input(&file)?;
            let ir = nemoir_dsl_fe::lower(&source, &display_name).unwrap_or_else(|diag| {
                print_diagnostic(diag, &display_name, source);
                std::process::exit(1);
            });
            let yaml_str = serde_yaml::to_string(&ir).unwrap_or_else(|e| {
                eprintln!("error: YAML serialization failed: {}", e);
                std::process::exit(1);
            });
            if let Some(out_path) = output {
                std::fs::write(&out_path, yaml_str)?;
            } else {
                std::io::stdout().write_all(yaml_str.as_bytes())?;
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
