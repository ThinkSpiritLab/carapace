#include <stdlib.h>
int main() {
    for (;;) {
        malloc(1 * 1024 * 1024);
    }
    return 0;
}