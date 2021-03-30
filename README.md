# carapace

[![Crates.io][crates-badge]][crates-url]
[![MIT licensed][mit-badge]][mit-url]
[![Docs][docs-badge]][docs-url]
![CI][ci-badge]

[crates-badge]: https://img.shields.io/crates/v/carapace.svg
[crates-url]: https://crates.io/crates/carapace
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[docs-badge]: https://docs.rs/carapace/badge.svg
[docs-url]: https://docs.rs/carapace/
[ci-badge]: https://github.com/ThinkSpiritLab/carapace/workflows/CI/badge.svg

A code runner for online judge.

`carapace` spawns an untrusted program and measure the time and memory consumed by the program.

`carapace` is designed for secure computing. It can utilize Linux namespace subsystem, resource limits, cgroups, seccomp-bpf and chroot to jail a program.

## Install

By cargo:

```sh
cargo install carapace
```

From source:

```sh
cargo install --path .
```

Install to `/usr/local/bin/carapace`

```sh
./install.sh
```

## Usage

```
carapace 0.2.0
Nugine <Nugine@163.com>

USAGE:
    carapace [FLAGS] [OPTIONS] <bin> [--] [args]...

ARGS:
    <bin>        
    <args>...    

FLAGS:
        --seccomp-forbid-ipc    
    -h, --help                  Prints help information
    -V, --version               Prints version information

OPTIONS:
    -e, --env <env>...                      
    -c, --chroot <path>                     
        --uid <uid>                         
        --gid <gid>                         
        --stdin <path>                      
        --stdout <path>                     
        --stderr <path>                     
        --stdin-fd <fd>                     
        --stdout-fd <fd>                    
        --stderr-fd <fd>                    
    -t, --real-time-limit <milliseconds>    
        --rlimit-cpu <seconds>              
        --rlimit-as <bytes>                 
        --rlimit-data <bytes>               
        --rlimit-fsize <bytes>              
        --cg-limit-memory <bytes>           
        --cg-limit-max-pids <count>         
        --bindmount-rw <bindmount>...       
    -b, --bindmount-ro <bindmount>...       
        --mount-proc=<path>                 
        --mount-tmpfs=<path>                
        --priority <prio>                   
        --report <path>                     
        --report-fd <fd>
```

## Examples

### Minimal untrusted shell

```shell
mkdir untrusted-workspace

sudo carapace \
    --uid `id -u` --gid `id -g` \
    -c untrusted-workspace \
    -b /bin /lib /lib64 \
    -t 60000 \
    --cg-limit-memory 256000000 \
    -- /bin/sh
```

Run as current user, chroot to untrusted-workspace and mount necessary dependencies.

Time limit: 60s. Memory limit: 256MB.

### hello-world.c

```c
#include <stdio.h>
int main(){
    printf("Hello, World!\n");
    return 0;
}
```

```shell
mkdir workspace
gcc hello-world.c -o workspace/hello

sudo carapace \
    --uid `id -u` --gid `id -g` \
    -c workspace \
    -b /lib /lib64 \
    -t 1000 \
    --cg-limit-memory 512000 \
    -- ./hello
```

Run as current user, chroot to workspace and mount necessary dependencies.

Time limit: 1s. Memory limit: 512KB.

Output:

```
Hello, World!
{"code":0,"signal":0,"real_time":1,"sys_time":0,"user_time":0,"memory":248}
```

Real time: 1ms. Sys time: 0ms. User time: 0ms.

Memory: 248 KiB.
