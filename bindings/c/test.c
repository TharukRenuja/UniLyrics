#include "unilyrics.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "--protocol") == 0) {
        char buffer[1048578];
        while (fgets(buffer, sizeof buffer, stdin)) {
            char *output = unilyrics_process((const uint8_t *)buffer, strlen(buffer));
            assert(output);
            puts(output);
            unilyrics_free(output);
        }
        return 0;
    }
    const char *request = "{\"op\":\"parse\",\"input\":\"[00:01.25]hello\",\"format\":\"lrc\"}";
    char *response = unilyrics_process((const uint8_t *)request, strlen(request));
    assert(response && strstr(response, "1250000") && strstr(response, "hello"));
    unilyrics_free(response);
    response = unilyrics_process(NULL, 0);
    assert(response && strstr(response, "INVALID_BUFFER"));
    unilyrics_free(response);
    response = unilyrics_parse(NULL, 0, UNILYRICS_PLAIN);
    assert(response && strstr(response, "\"ok\":true"));
    unilyrics_free(response);
    const char *lyrics = "[00:01]hi";
    response = unilyrics_detect((const uint8_t *)lyrics, strlen(lyrics));
    assert(response && strstr(response, "lrc"));
    unilyrics_free(response);
    const char *document = "{\"version\":1,\"cues\":[]}";
    response = unilyrics_inspect((const uint8_t *)document, strlen(document));
    assert(response && strstr(response, "\"agents\":false"));
    unilyrics_free(response);
    const char *timed = "{\"version\":1,\"cues\":[{\"id\":\"cue\",\"start_us\":100,\"segments\":[]}]}";
    response = unilyrics_normalize((const uint8_t *)timed, strlen(timed), -100);
    assert(response && strstr(response, "\"start_us\":0") && strstr(response, "TIMING_SHIFTED"));
    unilyrics_free(response);
    response = unilyrics_normalize((const uint8_t *)timed, strlen(timed), -101);
    assert(response && strstr(response, "INVALID_TIME"));
    unilyrics_free(response);
    response = unilyrics_call(NULL, 0, 99, 0, 0);
    assert(response && strstr(response, "INVALID_OPERATION"));
    unilyrics_free(response);
    response = unilyrics_project((const uint8_t *)timed, strlen(timed), NULL, 0);
    assert(response && strstr(response, "\"start_us\":100"));
    unilyrics_free(response);
    const char *options = "{\"profile\":\"custom\",\"features\":[]}";
    response = unilyrics_project((const uint8_t *)timed, strlen(timed), (const uint8_t *)options, strlen(options));
    assert(response && strstr(response, "PROFILE_LINE_TIMING_DROPPED") && strstr(response, "\"start_us\":null"));
    unilyrics_free(response);
    response = unilyrics_export_profile((const uint8_t *)timed, strlen(timed), UNILYRICS_JSON, 1, (const uint8_t *)options, strlen(options));
    assert(response && strstr(response, "LOSSY_CONVERSION"));
    unilyrics_free(response);
    response = unilyrics_export_preserved((const uint8_t *)document, strlen(document), UNILYRICS_PLAIN, NULL, 0);
    assert(response && strstr(response, "sidecar") && strstr(response, "\"ok\":true"));
    unilyrics_free(response);
    const char *sidecar = "{\"version\":1,\"format\":\"plain\",\"output\":\"\",\"document\":{\"version\":1,\"cues\":[]}}";
    response = unilyrics_restore_preserved(NULL, 0, UNILYRICS_PLAIN, (const uint8_t *)sidecar, strlen(sidecar));
    assert(response && strstr(response, "SIDECAR_RESTORED"));
    unilyrics_free(response);
    return 0;
}
