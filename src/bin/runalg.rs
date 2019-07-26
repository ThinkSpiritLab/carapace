use carapace::built_in::{c_cpp_rules, getPATH};
use carapace::{Target, TargetStatus};

use std::io::prelude::*;

use serde::Deserialize;

#[derive(Deserialize)]
struct Opt {
    bin: String,
    uid: Option<u32>,
    gid: Option<u32>,
    stdin: String,
    stdout: String,
    stderr: String,
    max_real_time: Option<u64>,
    max_cpu_time: Option<u64>,
    max_memory: Option<u64>,
    max_output_size: Option<u64>,
}

fn build_target(opt: Opt) -> Target {
    let mut target = Target::new(&opt.bin).expect("Unexpected \\0 in bin_path");

    if let Some(path) = getPATH() {
        target.envs.push(path);
    };

    target.uid = opt.uid;
    target.gid = opt.gid;

    target
        .set_stdin(&opt.stdin)
        .expect("Unexpected \\0 in stdin");
    target
        .set_stdout(&opt.stdout)
        .expect("Unexpected \\0 in stdout");;
    target
        .set_stderr(&opt.stderr)
        .expect("Unexpected \\0 in stderr");;

    target.limit.max_real_time = opt.max_real_time;
    target.limit.max_cpu_time = opt.max_cpu_time;
    target.limit.max_memory = opt.max_memory;
    target.limit.max_output_size = opt.max_output_size;
    target.limit.max_process_number = Some(1);

    target.rule = c_cpp_rules();
    target.forbid_inherited_env = true;
    target.forbid_target_execve = true;

    target
}

fn main() {
    loop {
        let opt: Opt = {
            let mut buf = String::new();
            let size = std::io::stdin()
                .lock()
                .read_line(&mut buf)
                .expect("stdin error");

            if size == 0 {
                break;
            }
            serde_json::from_str(&buf).expect("json error")
        };

        let target = build_target(opt);
        let status: std::io::Result<TargetStatus> = target.run();

        let (code, output) = match status {
            Ok(status) => (0, serde_json::to_string(&status).unwrap()),
            Err(err) => (err.raw_os_error().unwrap(), err.to_string()),
        };

        print!("{}\n{}\n", code, output);
    }
}
