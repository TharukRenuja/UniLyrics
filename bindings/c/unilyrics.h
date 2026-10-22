#ifndef UNILYRICS_H
#define UNILYRICS_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* data must contain length readable UTF-8 bytes. Input is never retained.
 * Returns a NUL-terminated JSON response owned by this library.
 * Release exactly once with unilyrics_free; do not use the system free(). */
char *unilyrics_process(const uint8_t *data, size_t length);
typedef enum {
    UNILYRICS_AUTO = 0, UNILYRICS_PLAIN = 1, UNILYRICS_LRC = 2,
    UNILYRICS_ENHANCED_LRC = 3, UNILYRICS_JSON = 4,
    UNILYRICS_TTML = 5, UNILYRICS_TTML_LYRICS = 6, UNILYRICS_SRT = 7,
    UNILYRICS_WEBVTT = 8
} unilyrics_format;
/* Operation adapter used by the convenience functions below. Empty input may be NULL.
 * export/validate consume canonical document JSON, not an entire parse-result envelope. */
char *unilyrics_call(const uint8_t *data, size_t length, int32_t operation, int32_t format, int32_t strict);
static inline char *unilyrics_parse(const uint8_t *text, size_t length, unilyrics_format format) {
    return unilyrics_call(text, length, 0, format, 0);
}
static inline char *unilyrics_detect(const uint8_t *text, size_t length) {
    return unilyrics_call(text, length, 1, UNILYRICS_AUTO, 0);
}
static inline char *unilyrics_export(const uint8_t *document, size_t length, unilyrics_format format, int32_t strict) {
    return unilyrics_call(document, length, 2, format, strict);
}
static inline char *unilyrics_validate(const uint8_t *document, size_t length) {
    return unilyrics_call(document, length, 3, UNILYRICS_AUTO, 0);
}
static inline char *unilyrics_inspect(const uint8_t *document, size_t length) {
    return unilyrics_call(document, length, 4, UNILYRICS_AUTO, 0);
}
/* Shift explicit times in canonical document JSON; input is never mutated. */
char *unilyrics_normalize(const uint8_t *document, size_t length, int64_t offset_us);
/* Profile options are a UTF-8 JSON object; empty options (NULL, 0) select full.
 * Documents are canonical JSON. Replies are owned; free with unilyrics_free.
 * Strict: 0 compatible, 1 strict; format uses unilyrics_format codes. */
char *unilyrics_project(const uint8_t *document, size_t length, const uint8_t *options, size_t options_length);
char *unilyrics_export_profile(const uint8_t *document, size_t length, int32_t format, int32_t strict, const uint8_t *options, size_t options_length);
/* Preservation replies contain output, losses and sidecar. Save output + sidecar.
 * Restore takes exact UTF-8 output and the sidecar JSON object, not its envelope.
 * Empty output may be NULL; sidecar must be non-NULL. All inputs are borrowed.
 * Every reply is owned; release once with unilyrics_free. Options may be (NULL,0). */
char *unilyrics_export_preserved(const uint8_t *document, size_t length, int32_t format, const uint8_t *options, size_t options_length);
char *unilyrics_restore_preserved(const uint8_t *output, size_t length, int32_t format, const uint8_t *sidecar, size_t sidecar_length);
/* Scoped custom formats. Callback inputs are borrowed UTF-8 byte ranges for that call.
 * Parse/export callbacks TRANSFER an owned ordinary JSON reply from this same library
 * (e.g. unilyrics_parse/export/process/export_result). Transfer it unchanged and do not
 * free it inside the callback. Never return malloc/static/reallocated strings.
 * Context is BORROWED: keep it/callbacks live and thread-safe until the registry is freed
 * and all calls finish. No exceptions/longjmp may cross a callback. Never free a handle
 * during an active call. Registry registration/lookups are synchronized; callbacks run
 * outside its lock. A failed registration retains no callback/context. */
typedef struct unilyrics_registry unilyrics_registry;
typedef int32_t (*unilyrics_match_callback)(void *, const uint8_t *, size_t);
typedef char *(*unilyrics_parse_callback)(void *, const uint8_t *, size_t);
typedef char *(*unilyrics_export_callback)(void *, const uint8_t *, size_t, int32_t);
unilyrics_registry *unilyrics_registry_new(void);
void unilyrics_registry_free(unilyrics_registry *registry);
/* Returns 1/0. Optional error slot is set to NULL on success or an owned JSON reply on
 * failure; free it exactly once. Do not pass an already-owned string in the output slot. */
int32_t unilyrics_registry_register(unilyrics_registry *, const uint8_t *name, size_t length,
    void *context, unilyrics_match_callback, unilyrics_parse_callback, unilyrics_export_callback, char **error);
/* Operation 0=parse, 1=detect, 2=export. Empty format means auto for parse.
 * Export data is canonical document JSON. Strict is 0=compatible, 1=strict.
 * Callback replies have a 1 MiB encoded limit; free every returned operation reply. */
char *unilyrics_registry_call(unilyrics_registry *, const uint8_t *data, size_t length,
    int32_t operation, const uint8_t *format, size_t format_length, int32_t strict);
/* UTF-8 output -> owned export-result envelope. The registry still verifies losses. */
/* Copy a JSON reply envelope into library-owned storage for callback transfer. */
char *unilyrics_copy_reply(const uint8_t *data, size_t length);
char *unilyrics_export_result(const uint8_t *data, size_t length);
char *unilyrics_error_result(const uint8_t *code, size_t code_length, const uint8_t *message, size_t message_length);
void unilyrics_free(char *response);
#ifdef __cplusplus
}
#endif
#endif
