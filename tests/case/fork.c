#include <stdio.h>
#include <unistd.h>

int main() {
    int n = 3;
    while (n--) {
        if (fork() < 0) {
            return 1;
        };
        fprintf(stderr, "%d\n", n);
    }
    return 0;
}