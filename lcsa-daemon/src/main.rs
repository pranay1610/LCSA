use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use lcsa_core::{PrimitiveEvent, normalize_event};
use lcsa_daemon::{primitive_from_notify_event, snapshot_events};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

#[derive(Debug, Parser)]
#[command(
    name = "lcsa-daemon",
    version,
    about = "Normalize filesystem events into semantic JSONL signals."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Emit semantic signals for the current contents of a directory tree.
    Scan {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        include_hidden: bool,
    },
    /// Watch a directory and stream semantic signals as files change.
    Watch {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        include_hidden: bool,
        #[arg(long, default_value_t = false)]
        initial_scan: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan {
            path,
            output,
            include_hidden,
        } => scan_command(&path, output.as_deref(), include_hidden),
        Command::Watch {
            path,
            output,
            include_hidden,
            initial_scan,
        } => watch_command(&path, output.as_deref(), include_hidden, initial_scan),
    }
}

fn scan_command(path: &Path, output: Option<&Path>, include_hidden: bool) -> Result<()> {
    let mut emitter = Emitter::new(path, output)?;
    emit_snapshot(path, include_hidden, &mut emitter)
}

fn watch_command(
    path: &Path,
    output: Option<&Path>,
    include_hidden: bool,
    initial_scan: bool,
) -> Result<()> {
    let mut emitter = Emitter::new(path, output)?;

    if initial_scan {
        emit_snapshot(path, include_hidden, &mut emitter)?;
    }

    let (tx, rx) = channel();
    let mut watcher = build_watcher(tx)?;

    watcher
        .watch(path, RecursiveMode::Recursive)
        .with_context(|| format!("failed to watch {}", path.display()))?;

    eprintln!("watching {}", path.display());

    loop {
        match rx.recv() {
            Ok(Ok(event)) => emit_notify_event(event, include_hidden, &mut emitter)?,
            Ok(Err(error)) => eprintln!("watch error: {error}"),
            Err(error) => {
                return Err(error).context("watch channel closed unexpectedly");
            }
        }
    }
}

fn build_watcher(tx: std::sync::mpsc::Sender<notify::Result<Event>>) -> Result<RecommendedWatcher> {
    RecommendedWatcher::new(
        move |result| {
            let _ = tx.send(result);
        },
        notify::Config::default(),
    )
    .context("failed to initialize filesystem watcher")
}

fn emit_snapshot(path: &Path, include_hidden: bool, emitter: &mut Emitter) -> Result<()> {
    for primitive in snapshot_events(path, include_hidden, emitter.output_path.as_deref())? {
        emitter.emit(primitive)?;
    }

    Ok(())
}

fn emit_notify_event(event: Event, include_hidden: bool, emitter: &mut Emitter) -> Result<()> {
    if let Some(primitive) = primitive_from_notify_event(
        event,
        emitter.root_path.as_path(),
        include_hidden,
        emitter.output_path.as_deref(),
    ) {
        emitter.emit(primitive)?;
    }

    Ok(())
}

struct Emitter {
    stdout: std::io::Stdout,
    file: Option<BufWriter<File>>,
    root_path: PathBuf,
    output_path: Option<PathBuf>,
}

impl Emitter {
    fn new(root_path: &Path, output: Option<&Path>) -> Result<Self> {
        let (file, output_path) = match output {
            Some(path) => {
                let file = File::create(path)
                    .with_context(|| format!("failed to create {}", path.display()))?;
                (Some(BufWriter::new(file)), canonicalize_lossy(path))
            }
            None => (None, None),
        };

        Ok(Self {
            stdout: std::io::stdout(),
            file,
            root_path: canonicalize_lossy(root_path).unwrap_or_else(|| root_path.to_path_buf()),
            output_path,
        })
    }

    fn emit(&mut self, primitive: PrimitiveEvent) -> Result<()> {
        let signal = normalize_event(&primitive);
        let encoded = serde_json::to_string(&signal).context("failed to encode signal")?;

        writeln!(self.stdout, "{encoded}").context("failed to write to stdout")?;

        if let Some(file) = &mut self.file {
            writeln!(file, "{encoded}").context("failed to write to output file")?;
            file.flush().context("failed to flush output file")?;
        }

        Ok(())
    }
}

fn canonicalize_lossy(path: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok().or_else(|| {
        if path.is_absolute() {
            Some(path.to_path_buf())
        } else {
            std::env::current_dir().ok().map(|cwd| cwd.join(path))
        }
    })
}
