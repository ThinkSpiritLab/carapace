carapace -p --rule-c-cpp --forbid-inherited-env --forbid-target-execve tests/bin/execvp
carapace -p --rule-c-cpp --forbid-inherited-env --forbid-target-execve tests/bin/fork
carapace -p --rule-c-cpp --forbid-inherited-env --forbid-target-execve tests/bin/forkbomb
carapace -p --rule-c-cpp --forbid-inherited-env --forbid-target-execve tests/bin/hello
carapace -p --rule-c-cpp --forbid-inherited-env --forbid-target-execve --max-memory 10000 tests/bin/mle
carapace -p --rule-c-cpp --forbid-inherited-env --forbid-target-execve --max-real-time 1000000 tests/bin/real_tle
carapace -p --rule-c-cpp --forbid-inherited-env --forbid-target-execve --max-cpu-time 1 tests/bin/tle
