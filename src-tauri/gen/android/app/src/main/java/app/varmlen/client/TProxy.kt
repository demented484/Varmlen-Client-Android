package app.varmlen.client

/** JNI bindings to hev-socks5-tunnel (tun2socks). `startTun2socks` blocks until
 *  `stopTun2socks`, so callers run it on a dedicated thread. */
object TProxy {
    init {
        System.loadLibrary("hev-socks5-tunnel")
        System.loadLibrary("tproxy")
    }

    external fun startTun2socks(config: String, fd: Int): Int
    external fun stopTun2socks()
}
