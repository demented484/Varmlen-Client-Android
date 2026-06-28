package app.varmlen.client

/** JNI bindings to hev-socks5-tunnel (tun2socks). `startTun2socks` runs the
 *  tunnel on a native pthread and returns immediately. */
object TProxy {
    init {
        System.loadLibrary("hev-socks5-tunnel")
        System.loadLibrary("tproxy")
    }

    /** @param configPath path to the hev YAML config; @param fd the tun fd. */
    external fun startTun2socks(configPath: String, fd: Int)
    external fun stopTun2socks()
}
