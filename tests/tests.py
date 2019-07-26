from subprocess import Popen, PIPE
from copy import deepcopy
from pprint import pprint
import json

base = {
    "bin": None,
    "uid": None,
    "gid": None,
    "stdin": "tests/in",
    "stdout": "tests/out",
    "stderr": "tests/err",
    "max_real_time": 1000000,
    "max_cpu_time": 1,
    "max_memory": 256*1024*1024,
    "max_output_size": 32*1024*1024
}

reqs = [
    {
        "bin": "tests/bin/execvp",
    },
    {
        "bin": "tests/bin/fork"
    },
    {
        "bin": "tests/bin/forkbomb"
    },
    {
        "bin": "tests/bin/hello"
    },
    {
        "bin": "tests/bin/mle",
        "max_memory": 10000,
    },
    {
        "bin": "tests/bin/real_tle",
    },
    {
        "bin": "tests/bin/tle"
    }
]

opts = [{**base, **r} for r in reqs]

pprint(opts)

runalg = Popen("runalg", shell=True, stdin=PIPE, stdout=PIPE, close_fds=True)
(tx, rx) = (runalg.stdin, runalg.stdout)

for opt in opts:
    msg = json.dumps(opt)+"\n"
    tx.write(msg.encode())
    tx.flush()
    code = int(rx.readline())
    result = json.loads(rx.readline())

    pprint(code)
    pprint(result)
