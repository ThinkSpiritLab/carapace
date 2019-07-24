#include <errno.h>
#include <stdio.h>
#include <unistd.h>

int main() {
    int n = 3;
    while (n--) {
        fprintf(stdout, "%d\n", n);
        if (fork() < 0) {
            perror(NULL);
            return 1;
        };
    }
    return 0;
}