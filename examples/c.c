#include "unilyrics.h"
#include <stdio.h>
#include <string.h>

int main(void) {
    const char *input = "[00:01]Hello 世界 🎵";
    char *response = unilyrics_parse((const uint8_t *)input, strlen(input), UNILYRICS_LRC);
    if (!response) return 1;
    puts(response); /* JSON response envelope; application JSON handling stays with the caller. */
    unilyrics_free(response);
    return 0;
}
