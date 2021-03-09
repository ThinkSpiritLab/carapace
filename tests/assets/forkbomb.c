// WARNING: DO NOT TRY TO RUN THIS CODE WITHOUT A PROCESS NUMBER LIMIT
#include <stdio.h>
#include <sys/prctl.h>
#include <unistd.h>

int main() {
    int i = 0;

    for (;;) {
        if (fork() > 0) {
            for (;;) {
                ++i;
            }
        }
    }

    return 0;
}