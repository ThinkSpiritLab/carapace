#include <stdlib.h>
#include <string.h>

int main() {
    int size = sizeof(int) * 8 * 1024 * 1024;
    int *p = malloc(size);
    memset(p, -1, size);
    free(p);
    return 0;
}