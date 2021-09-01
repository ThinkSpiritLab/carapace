use carapace::{SandboxConfig, SandboxOutput};

use std::fs;
use std::sync::Once;

use anyhow::Result;
use tracing::{debug, error, info};

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

fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        dotenv::dotenv().ok();
        setup_tracing();

        let workspace = "/tmp/carapace_test";
        let _ = fs::remove_dir_all(workspace);
        fs::create_dir(workspace).unwrap();
    });
}

fn run(config: &SandboxConfig) -> Result<SandboxOutput> {
    match carapace::run(config) {
        Ok(output) => {
            debug!("sandbox output = {:?}", output);
            Ok(output)
        }
        Err(err) => {
            error!("sandbox error:\n{:?}", err);
            Err(err)
        }
    }
}

fn gcc_compile(src: &str, bin: &str) -> Result<SandboxOutput> {
    let args = SandboxConfig {
        bin: "/usr/bin/gcc".into(),
        args: vec!["-o".into(), bin.into(), src.into()],
        env: vec!["PATH".into()],
        cg_limit_memory: Some(256 * 1024 * 1024), // 256 MiB
        real_time_limit: Some(3000),              // 3000 ms
        ..Default::default()
    };

    run(&args)
}

fn test_compile(name: &str, src: &str, bin: &str, check: impl FnOnce(SandboxOutput)) -> Result<()> {
    init();
    info!("{} src = {}, bin = {}", name, src, bin);
    let output = gcc_compile(src, bin)?;
    check(output);
    info!("{} finished", name);
    Ok(())
}

fn test_hack(
    name: &str,
    src: &str,
    bin: &str,
    config: &SandboxConfig,
    check: impl FnOnce(SandboxOutput),
) -> Result<()> {
    init();
    info!("{} src = {}, bin = {}", name, src, bin);
    gcc_compile(src, bin)?;
    info!("{} run hack", name);
    let output = run(config)?;
    assert_ne!(output.code, 101);
    check(output);
    info!("{} finished", name);
    Ok(())
}

macro_rules! assets {
    ($file:literal) => {
        concat!("tests/assets/", $file)
    };
}

macro_rules! tmp {
    ($file:literal) => {
        concat!("/tmp/carapace_test/", $file)
    };
}

macro_rules! assert_le {
    ($lhs:expr, $rhs:expr) => {{
        let lhs = $lhs;
        let rhs = $rhs;
        assert!(lhs <= rhs, "lhs = {:?}, rhs = {:?}", lhs, rhs)
    }};
}

#[tokio::test(flavor = "multi_thread")]
async fn t01_empty() -> Result<()> {
    let name = "t01_empty";
    let src = assets!("empty.c");
    let bin = tmp!("t01_empty");

    let args = &SandboxConfig {
        bin: bin.into(),
        ..Default::default()
    };

    test_hack(name, src, bin, args, |output| {
        assert_eq!(output.code, 0);
        assert_eq!(output.signal, 0);

        assert_le!(output.real_time, 100);
        assert_eq!(output.sys_time, 0);
        assert_le!(output.user_time, 1);
        assert_le!(output.memory, 400);
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn t02_sleep() -> Result<()> {
    let name = "t02_sleep";
    let src = assets!("sleep.c");
    let bin = tmp!("t02_sleep");

    let args = &SandboxConfig {
        bin: bin.into(),
        real_time_limit: Some(1000),
        ..Default::default()
    };

    test_hack(name, src, bin, args, |output| {
        assert_eq!(output.code, 0);
        assert_eq!(output.signal, 9);

        assert_le!(output.real_time, 1000 + 100);
        assert_eq!(output.sys_time, 0);
        assert_le!(output.user_time, 1);
        assert_le!(output.memory, 400);
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn t03_forkbomb() -> Result<()> {
    let name = "t03_forkbomb";
    let src = assets!("forkbomb.c");
    let bin = tmp!("t03_forkbomb");

    let args = &SandboxConfig {
        bin: bin.into(),
        cg_limit_max_pids: Some(3),
        real_time_limit: Some(1000),
        stdout: Some(tmp!("t03_forkbomb_stdout").into()),
        ..Default::default()
    };

    test_hack(name, src, bin, args, |output| {
        assert_eq!(output.code, 0);
        assert_eq!(output.signal, 9);

        assert_le!(output.real_time, 1000 + 100);
        assert_eq!(output.sys_time, 0);
        assert_le!(output.user_time, 3000);
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn t04_includebomb() -> Result<()> {
    let name = "t04_includebomb";
    let src = assets!("includebomb.c");
    let bin = tmp!("t04_includebomb");

    test_compile(name, src, bin, |output| {
        if output.code == 0 {
            assert!(output.memory >= 256 * 1024);
        }
        assert_le!(output.real_time, 3000 + 100);
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn t05_oom() -> Result<()> {
    let name = "t05_oom";
    let src = assets!("oom.c");
    let bin = tmp!("t05_oom");

    let args = &SandboxConfig {
        bin: bin.into(),
        cg_limit_memory: Some(16 * 1024 * 1024), // 16 MiB
        real_time_limit: Some(1000),
        ..Default::default()
    };

    test_hack(name, src, bin, args, |output| {
        assert_eq!(output.code, 0);
        assert_eq!(output.signal, 9);

        assert_le!(output.real_time, 1000 + 100);
        assert_eq!(output.sys_time, 0);
        assert_le!(output.user_time, 1000);
    })
}
