#include "unilyrics.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct { unilyrics_registry *registry; int mutate; int calls; } context;
static int32_t matches(void *state, const uint8_t *text, size_t length) {
    (void)state;
    return length >= 7 && memcmp(text, "CUSTOM:", 7) == 0;
}
static char *parse(void *state, const uint8_t *text, size_t length) {
    context *ctx = (context *)state;
    ctx->calls++;
    if (!matches(state, text, length)) {
        const char *code = "BAD_HEADER", *message = "Missing CUSTOM prefix";
        return unilyrics_error_result((const uint8_t *)code, strlen(code), (const uint8_t *)message, strlen(message));
    }
    return unilyrics_parse(text + 7, length - 7, UNILYRICS_JSON);
}
static char *export_lyrics(void *state, const uint8_t *document, size_t length, int32_t strict) {
    context *ctx = (context *)state;
    (void)strict;
    if (ctx->mutate) {
        const char *changed = "CUSTOM:{\"version\":1,\"cues\":[]}";
        return unilyrics_export_result((const uint8_t *)changed, strlen(changed));
    }
    uint8_t *buffer = (uint8_t *)malloc(length + 7);
    assert(buffer);
    memcpy(buffer, "CUSTOM:", 7);
    memcpy(buffer + 7, document, length);
    char *result = unilyrics_export_result(buffer, length + 7);
    free(buffer);
    return result;
}
static char *null_reply(void *state, const uint8_t *text, size_t length) {
    (void)state; (void)text; (void)length;
    return NULL;
}
static char *reentrant(void *state, const uint8_t *text, size_t length) {
    context *ctx = (context *)state;
    char *error = NULL;
    const char *name = "x-inside";
    assert(unilyrics_registry_register(ctx->registry, (const uint8_t *)name, strlen(name), NULL, NULL, NULL, NULL, &error) == 1);
    assert(error == NULL);
    return unilyrics_parse(text, length, UNILYRICS_PLAIN);
}
static char *call(unilyrics_registry *registry, const char *text, int operation, const char *format, int strict) {
    return unilyrics_registry_call(registry, (const uint8_t *)text, strlen(text), operation, (const uint8_t *)format, strlen(format), strict);
}
static void expect(char *reply, const char *text) {
    assert(reply && strstr(reply, text));
    unilyrics_free(reply);
}
int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "--protocol") == 0) {
        char buffer[1048578];
        while (fgets(buffer, sizeof buffer, stdin)) {
            char *reply = unilyrics_process((const uint8_t *)buffer, strlen(buffer));
            assert(reply); puts(reply); unilyrics_free(reply);
        }
        return 0;
    }
    unilyrics_registry *registry = unilyrics_registry_new();
    assert(registry);
    context ctx = {registry, 0, 0};
    char *error = NULL;
    const char *name = "x-wrapped";
    assert(unilyrics_registry_register(registry, (const uint8_t *)name, strlen(name), &ctx, matches, parse, export_lyrics, &error) == 1);
    assert(error == NULL);
    assert(unilyrics_registry_register(registry, (const uint8_t *)name, strlen(name), &ctx, matches, parse, export_lyrics, &error) == 0);
    expect(error, "DUPLICATE_FORMAT"); error = NULL;
    const char *doc = "{\"version\":1,\"cues\":[{\"id\":\"cue\",\"start_us\":0,\"segments\":[{\"text\":\"සිංහල 👩🏽‍💻\"}]}],\"attributes\":{\"precise\":18446744073709551615}}";
    expect(call(registry, doc, 2, name, 1), "CUSTOM:");
    const char *wrapped = "CUSTOM:{\"version\":1,\"cues\":[{\"id\":\"cue\",\"segments\":[{\"text\":\"සිංහල 👩🏽‍💻\"}]}]}";
    expect(call(registry, wrapped, 0, "", 0), "සිංහල 👩🏽‍💻");
    expect(call(registry, wrapped, 1, "", 0), "x-wrapped");
    expect(call(registry, "bad", 0, name, 0), "BAD_HEADER");
    ctx.mutate = 1;
    expect(call(registry, doc, 2, name, 0), "CUSTOM_ROUNDTRIP_CHANGED");
    expect(call(registry, doc, 2, name, 1), "LOSSY_CONVERSION");
    ctx.mutate = 0;
    name = "x-output";
    assert(unilyrics_registry_register(registry, (const uint8_t *)name, strlen(name), &ctx, NULL, NULL, export_lyrics, &error) == 1);
    expect(call(registry, doc, 2, name, 0), "CUSTOM_ROUNDTRIP_UNVERIFIED");
    expect(call(registry, doc, 2, name, 1), "LOSSY_CONVERSION");
    name = "x-null";
    assert(unilyrics_registry_register(registry, (const uint8_t *)name, strlen(name), NULL, NULL, null_reply, NULL, &error) == 1);
    expect(call(registry, "hi", 0, name, 0), "INVALID_CALLBACK_RESULT");
    name = "x-reentrant";
    assert(unilyrics_registry_register(registry, (const uint8_t *)name, strlen(name), &ctx, NULL, reentrant, NULL, &error) == 1);
    expect(call(registry, "hi", 0, name, 0), "\"text\":\"hi\"");
    expect(call(NULL, "hi", 0, "plain", 0), "INVALID_REGISTRY");
    assert(ctx.calls > 0);
    unilyrics_registry_free(registry);
    unilyrics_registry_free(NULL);
    puts("C registry callbacks, ownership and preservation checks passed.");
    return 0;
}
