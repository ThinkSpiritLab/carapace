use carapace::built_in::c_cpp_rules;
use carapace::Target;

use std::ffi::{CString, NulError};

use structopt::StructOpt;

fn parse_c_string(s: &str) -> Result<CString, NulError> {
    CString::new(s)
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Opt {
    #[structopt(name = "BIN", parse(try_from_str = "parse_c_string"))]
    bin: CString,

    #[structopt(name = "ARGS", parse(try_from_str = "parse_c_string"))]
    args: Vec<CString>,

    #[structopt(
        long = "env",
        value_name = "env",
        parse(try_from_str = "parse_c_string")
    )]
    envs: Vec<CString>,

    #[structopt(long, value_name = "uid")]
    sudo_uid: Option<u32>,

    #[structopt(long, value_name = "gid")]
    sudo_gid: Option<u32>,

    #[structopt(long, value_name = "path", parse(try_from_str = "parse_c_string"))]
    stdin: Option<CString>,

    #[structopt(long, value_name = "path", parse(try_from_str = "parse_c_string"))]
    stdout: Option<CString>,

    #[structopt(long, value_name = "path", parse(try_from_str = "parse_c_string"))]
    stderr: Option<CString>,

    #[structopt(flatten)]
    limit: LimitOpt,

    #[structopt(long)]
    rule_c_cpp: bool,

    #[structopt(long)]
    forbid_inherited_env: bool,

    #[structopt(long)]
    forbid_target_execve: bool,

    #[structopt(long, short = "p")]
    pretty_json: bool,
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct LimitOpt {
    #[structopt(long, value_name = "microseconds")]
    max_real_time: Option<u32>, // in microseconds

    #[structopt(long, value_name = "seconds")]
    max_cpu_time: Option<u64>, // in seconds

    #[structopt(long, value_name = "bytes")]
    max_memory: Option<u64>, // in bytes

    #[structopt(long, value_name = "bytes")]
    max_output_size: Option<u64>, // in bytes

    #[structopt(long, value_name = "number")]
    max_process_number: Option<u64>,

    #[structopt(long, value_name = "bytes")]
    max_stack_size: Option<u64>, // in bytes
}

fn build_target(opt: Opt) -> Target {
    let mut target = Target::from_bin_path(opt.bin);
    target.args = opt.args;
    target.envs = opt.envs;

    target.uid = opt.sudo_uid;
    target.gid = opt.sudo_gid;

    target.stdin = opt.stdin;
    target.stdout = opt.stdout;
    target.stderr = opt.stderr;

    target.limit.max_real_time = opt.limit.max_real_time;
    target.limit.max_cpu_time = opt.limit.max_cpu_time;
    target.limit.max_memory = opt.limit.max_memory;
    target.limit.max_output_size = opt.limit.max_output_size;
    target.limit.max_process_number = opt.limit.max_process_number;
    target.limit.max_stack_size = opt.limit.max_stack_size;

    if opt.rule_c_cpp {
        target.rule = c_cpp_rules();
    }

    target.forbid_inherited_env = opt.forbid_inherited_env;
    target.forbid_target_execve = opt.forbid_target_execve;

    target
}

fn main() -> std::io::Result<()> {
    let opt = Opt::from_args();
    let pretty_json = opt.pretty_json;

    let target = build_target(opt);
    let status = target.run()?;

    let output = if pretty_json {
        serde_json::to_string_pretty(&status).expect("json error")
    } else {
        serde_json::to_string(&status).expect("json error")
    };

    Ok(println!("{}", output))
}
