// JNI shim around hev-socks5-tunnel: bridges the VpnService tun fd to xray's
// local SOCKS inbound. hev's main blocks, so we run it on a dedicated NATIVE
// pthread (not a JVM thread — running hev's task system on a JVM-managed thread
// crashes) and return immediately, matching how v2rayNG drives it.
#include <jni.h>
#include <pthread.h>
#include <stdlib.h>
#include <string.h>
#include "hev-socks5-tunnel.h"

typedef struct {
    char *path;
    int fd;
} args_t;

static void *run_tunnel(void *p) {
    args_t *a = (args_t *) p;
    hev_socks5_tunnel_main_from_file(a->path, a->fd);
    free(a->path);
    free(a);
    return NULL;
}

JNIEXPORT void JNICALL
Java_app_varmlen_client_TProxy_startTun2socks(JNIEnv *env, jclass clazz,
                                              jstring configPath, jint fd) {
    (void) clazz;
    const char *p = (*env)->GetStringUTFChars(env, configPath, NULL);
    args_t *a = (args_t *) malloc(sizeof(args_t));
    a->path = strdup(p);
    a->fd = fd;
    (*env)->ReleaseStringUTFChars(env, configPath, p);

    pthread_t t;
    pthread_create(&t, NULL, run_tunnel, a);
    pthread_detach(t);
}

JNIEXPORT void JNICALL
Java_app_varmlen_client_TProxy_stopTun2socks(JNIEnv *env, jclass clazz) {
    (void) env;
    (void) clazz;
    hev_socks5_tunnel_quit();
}
