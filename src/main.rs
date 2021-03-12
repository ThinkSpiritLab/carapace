use std::io::{self, Write};

use anyhow::Result;
use carapace::{SandboxConfig, SandboxOutput};
use clap::Clap;
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

fn main() -> Result<()> {
    dotenv::dotenv().ok();
    setup_tracing();

    let config = SandboxConfig::parse();

    let runtime = runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_time()
        .build()?;

    let output: SandboxOutput = {
        let _enter = runtime.enter();
        carapace::run(&config)?
    };

    {
        let stdout = io::stdout();
        let mut stdout_lock = stdout.lock();
        let out = &mut stdout_lock;
        serde_json::to_writer(&mut *out, &output)?;
        writeln!(out)?;
    }

    Ok(())
}
