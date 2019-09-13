# carapace

## Install

```
git clone https://github.com/Nugine/carapace.git
cd carapace
cargo install --path ./ --bin carapace
```

## Usage

```
carapace 0.0.0
Nugine <nugine@foxmail.com>
A code runner for online judge

USAGE:
    carapace [FLAGS] [OPTIONS] <bin> [--] [args]...

FLAGS:
        --forbid-inherited-env    
        --forbid-target-execve    
    -h, --help                    Prints help information
    -p, --pretty-json             
        --rule-c-cpp              
    -V, --version                 Prints version information

OPTIONS:
        --env <env>...                    
        --max-cpu-time <seconds>          
        --max-memory <bytes>              
        --max-output-size <bytes>         
        --max-process-number <number>     
        --max-real-time <microseconds>    
        --max-stack-size <bytes>          
        --stderr <path>                   
        --stdin <path>                    
        --stdout <path>                   
        --sudo-gid <gid>                  
        --sudo-uid <uid>                  

ARGS:
    <bin>        
    <args>... 
```

## Example

```
$ carapace ls
Cargo.lock  Cargo.toml	LICENSE  src  target  tests
{"code":0,"signal":null,"real_time":9141,"user_time":0,"sys_time":5380,"memory":2520}
```

real_time: 9141 μs

user_time:    0 μs

sys_time:  5380 μs

memory:    2520 kb


```
$ carapace --max-real-time 1000000 ping www.baidu.com
PING www.a.shifen.com (183.232.231.174) 56(84) bytes of data.
64 bytes from 183.232.231.174 (183.232.231.174): icmp_seq=1 ttl=55 time=34.7 ms
{"code":null,"signal":9,"real_time":1001249,"user_time":0,"sys_time":7285,"memory":2936}
```

```
$ carapace --max-memory 1024 node
{"code":null,"signal":11,"real_time":698,"user_time":0,"sys_time":918,"memory":924}
```