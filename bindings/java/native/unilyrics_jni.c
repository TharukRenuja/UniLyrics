#include <jni.h>
#include <limits.h>
#include <string.h>
#include "../../c/unilyrics.h"

JNIEXPORT jbyteArray JNICALL Java_org_tharukrj_unilyrics_UniLyrics_processBytes(
    JNIEnv *env, jclass type, jbyteArray request) {
    (void)type;
    if (!request) return NULL;
    jsize length = (*env)->GetArrayLength(env, request);
    if (length > 1048576) return NULL;
    jbyte *input = (*env)->GetByteArrayElements(env, request, NULL);
    if (!input) return NULL;
    char *output = unilyrics_process((const uint8_t *)input, (size_t)length);
    (*env)->ReleaseByteArrayElements(env, request, input, JNI_ABORT);
    if (!output) return NULL;
    size_t count = strlen(output);
    if (count > INT_MAX) { unilyrics_free(output); return NULL; }
    jbyteArray result = (*env)->NewByteArray(env, (jsize)count);
    if (result) (*env)->SetByteArrayRegion(env, result, 0, (jsize)count, (const jbyte *)output);
    unilyrics_free(output);
    return result;
}
