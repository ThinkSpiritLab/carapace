use std::fs;
use std::io::{self, Write};
use std::os::unix::io::RawFd;
use std::path::PathBuf;

use anyhow::{Context, Result};
use carapace::{SandboxConfig, SandboxOutput};
use clap::Clap;
use nix::unistd;
use tokio::runtime;

fn setup_tracing() {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{fmt, EnvFilter};

    tracing_subscriber::fmt()
        .event_format(fmt::format::Format::default().pretty())
        .with_env_filter(EnvFilter::from_default_env())
        .with_timer(fmt::time::ChronoLocal::rfc3339())
        .finish()
        .with(ErrorLayer::default())
        .init();
}

#[derive(Debug, Clap)]
struct Opt {
    #[clap(flatten)]
    config: SandboxConfig,

    #[clap(long, value_name = "path")]
    report: Option<PathBuf>,

    #[clap(long, value_name = "fd", conflicts_with = "report")]
    report_fd: Option<RawFd>,
}

fn main() -> Result<()> {
    dotenv::dotenv().ok();
    setup_tracing();

    let opt = Opt::parse();

    let runtime = runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_time()
        .build()?;

    let output: SandboxOutput = {
        let _enter = runtime.enter();
        carapace::run(&opt.config)?
    };

    match (opt.report, opt.report_fd) {
        (Some(path), _) => {
            let mut report_file = fs::File::create(&path).with_context(|| {
                format!("failed to create report file: path = {}", path.display())
            })?;
            let out = &mut report_file;
            serde_json::to_writer(&mut *out, &output)?;
            writeln!(out)?;
            report_file.flush()?;
        }
        (None, Some(fd)) => {
            let mut buf = serde_json::to_string(&output)?;
            buf.push('\n');
            unistd::write(fd, buf.as_bytes())
                .with_context(|| format!("failed to write report: fd = {}", fd))?;
        }
        (None, None) => {
            let stdout = io::stdout();
            let mut stdout_lock = stdout.lock();
            let out = &mut stdout_lock;
            serde_json::to_writer(&mut *out, &output)?;
            writeln!(out)?;
            out.flush()?;
        }
    };

    Ok(())
}
