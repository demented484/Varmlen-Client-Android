// JNI shim around hev-socks5-tunnel: bridges the VpnService tun fd to xray's
// local SOCKS inbound. `startTun2socks` blocks until `stopTun2socks` is called,
// so Kotlin runs it on a dedicated thread.
#include <jni.h>
#include <string.h>
#include "hev-socks5-tunnel.h"

JNIEXPORT jint JNICALL
Java_app_varmlen_client_TProxy_startTun2socks(JNIEnv *env, jclass clazz,
                                              jstring config, jint fd) {
    (void) clazz;
    const char *cfg = (*env)->GetStringUTFChars(env, config, NULL);
    int ret = hev_socks5_tunnel_main_from_str((const unsigned char *) cfg,
                                              (unsigned int) strlen(cfg), fd);
    (*env)->ReleaseStringUTFChars(env, config, cfg);
    return ret;
}

JNIEXPORT void JNICALL
Java_app_varmlen_client_TProxy_stopTun2socks(JNIEnv *env, jclass clazz) {
    (void) env;
    (void) clazz;
    hev_socks5_tunnel_quit();
}
