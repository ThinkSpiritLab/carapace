#include <stdio.h>
#include <sys/prctl.h>
#include <unistd.h>

int main() {
    int pid = fork();
    if (pid > 0) {
        // p1
        usleep(500 * 1000);
        printf("p1 done\n");
        return 0;
    }

    // p2
    pid = fork();
    if (pid > 0) {
        printf("p2 done\n");
        return 0;
    }

    // p3
    sleep(3);
    return 0;
}