# carapace

> A code runner for online judge

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
carapace 0.2.0-dev
Nugine <Nugine@163.com>

USAGE:
    carapace [OPTIONS] <bin> [--] [args]...

ARGS:
    <bin>        
    <args>...    

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

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
