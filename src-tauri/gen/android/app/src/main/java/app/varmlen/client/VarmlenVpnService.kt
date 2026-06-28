package app.varmlen.client

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import java.io.File

/**
 * The Android data plane. Establishes a tun via VpnService, runs the bundled
 * xray (a local SOCKS proxy) as a child process, and bridges the tun to it with
 * hev-socks5-tunnel (tun2socks). Mirrors the desktop "tun2socks" path, so the
 * same generated xray config is reused.
 */
class VarmlenVpnService : VpnService() {
    private var tun: ParcelFileDescriptor? = null
    private var xray: Process? = null

    companion object {
        const val ACTION_CONNECT = "app.varmlen.client.CONNECT"
        const val ACTION_DISCONNECT = "app.varmlen.client.DISCONNECT"
        const val EXTRA_CONFIG = "config"
        const val EXTRA_SOCKS_PORT = "socksPort"
        const val EXTRA_DNS = "dns"
        const val EXTRA_APPS = "apps"
        const val EXTRA_APPS_ALLOW = "appsAllow"
        const val EXTRA_LOG_LEVEL = "logLevel"
        const val LOG_FILE = "varmlen.log"
        private const val CHANNEL = "varmlen_vpn"
        private const val NOTIF_ID = 1
        private const val TUN_ADDR = "10.10.10.2"
        private const val MTU = 8500

        @Volatile
        var running = false
            private set
    }

    /** Append a line to filesDir/varmlen.log so the in-app log viewer (and Rust)
     *  can read it without adb. */
    private fun log(msg: String, e: Throwable? = null) {
        try {
            val f = File(filesDir, LOG_FILE)
            f.appendText("[${System.currentTimeMillis()}] $msg\n")
            if (e != null) f.appendText(android.util.Log.getStackTraceString(e) + "\n")
        } catch (_: Throwable) {}
        android.util.Log.i("VarmlenVpn", msg, e)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_DISCONNECT -> {
                log("disconnect")
                stopAll()
                return START_NOT_STICKY
            }
            ACTION_CONNECT -> {
                try {
                    startAll(
                        intent.getStringExtra(EXTRA_CONFIG) ?: error("no config"),
                        intent.getIntExtra(EXTRA_SOCKS_PORT, 2081),
                        intent.getStringExtra(EXTRA_DNS) ?: "1.1.1.1",
                        intent.getStringArrayExtra(EXTRA_APPS) ?: emptyArray(),
                        intent.getBooleanExtra(EXTRA_APPS_ALLOW, false),
                        intent.getStringExtra(EXTRA_LOG_LEVEL) ?: "warn"
                    )
                } catch (e: Throwable) {
                    log("connect failed", e)
                    stopAll()
                }
            }
        }
        return START_STICKY
    }

    private fun startAll(
        config: String, socksPort: Int, dns: String,
        apps: Array<String>, appsAllow: Boolean, logLevel: String
    ) {
        log("startAll socksPort=$socksPort dns=$dns apps=${apps.size} allow=$appsAllow level=$logLevel")
        startForegroundSafe()

        // 1) xray as a local SOCKS proxy (the generated config binds 127.0.0.1:socksPort).
        val cfgFile = File(filesDir, "xray.json").apply { writeText(config) }
        val xrayBin = File(applicationInfo.nativeLibraryDir, "libxray.so")
        log("exec xray: ${xrayBin.absolutePath} (exists=${xrayBin.exists()})")
        xray = ProcessBuilder(xrayBin.absolutePath, "run", "-c", cfgFile.absolutePath)
            .directory(filesDir)
            .redirectErrorStream(true)
            .start()
        // Drain xray output into the log so config errors are visible.
        Thread {
            try {
                xray?.inputStream?.bufferedReader()?.forEachLine { log("xray: $it") }
            } catch (_: Throwable) {}
        }.apply { isDaemon = true; start() }

        // 2) the tun interface.
        val builder = Builder()
            .setSession("Varmlen")
            .setMtu(MTU)
            .addAddress(TUN_ADDR, 30)
            .addRoute("0.0.0.0", 0)
            .addDnsServer(dns)
        for (pkg in apps) {
            try {
                if (appsAllow) builder.addAllowedApplication(pkg)
                else builder.addDisallowedApplication(pkg)
            } catch (_: Exception) { /* app not installed */ }
        }
        try { builder.addDisallowedApplication(packageName) } catch (_: Exception) {}
        val fd = builder.establish() ?: error("establish() returned null")
        tun = fd
        log("tun established fd=${fd.fd}")

        // 3) tun2socks: bridge the tun fd to xray's SOCKS inbound. The JNI runs
        //    hev on a native pthread and returns immediately.
        val yaml = """
            tunnel:
              mtu: $MTU
              ipv4: $TUN_ADDR
            socks5:
              address: 127.0.0.1
              port: $socksPort
              udp: 'udp'
            misc:
              tcp-read-write-timeout: 300000
              udp-read-write-timeout: 60000
              log-level: $logLevel
        """.trimIndent()
        val hevFile = File(filesDir, "hev.yaml").apply { writeText(yaml) }
        log("tun2socks starting (native)")
        TProxy.startTun2socks(hevFile.absolutePath, fd.fd)

        running = true
        log("connected")
    }

    private fun stopAll() {
        running = false
        try { TProxy.stopTun2socks() } catch (_: Throwable) {}
        try { xray?.destroy() } catch (_: Throwable) {}
        xray = null
        try { tun?.close() } catch (_: Throwable) {}
        tun = null
        try { stopForeground(STOP_FOREGROUND_REMOVE) } catch (_: Throwable) {}
        stopSelf()
    }

    override fun onDestroy() {
        stopAll()
        super.onDestroy()
    }

    /** Best-effort foreground; never throws (the VPN works either way). */
    private fun startForegroundSafe() {
        try {
            val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                nm.createNotificationChannel(
                    NotificationChannel(CHANNEL, "VPN", NotificationManager.IMPORTANCE_LOW)
                )
            }
            val open = PendingIntent.getActivity(
                this, 0, Intent(this, MainActivity::class.java),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
            )
            val notif: Notification = Notification.Builder(this, CHANNEL)
                .setContentTitle("Varmlen")
                .setContentText("VPN active")
                .setSmallIcon(android.R.drawable.ic_lock_lock)
                .setContentIntent(open)
                .setOngoing(true)
                .build()
            if (Build.VERSION.SDK_INT >= 34) {
                startForeground(NOTIF_ID, notif, ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED)
            } else {
                startForeground(NOTIF_ID, notif)
            }
        } catch (e: Throwable) {
            log("startForeground failed (continuing)", e)
        }
    }
}
