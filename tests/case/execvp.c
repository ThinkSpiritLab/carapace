#include <errno.h>
#include <stdio.h>
#include <unistd.h>

int main() {
    usleep(150 * 1000); // 150 ms
    printf("loop\n");
    char *argv[] = {NULL};
    execvp("./tests/bin/execvp", argv);
    return errno;
}