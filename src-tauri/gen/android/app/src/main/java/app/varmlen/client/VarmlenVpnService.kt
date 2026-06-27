package app.varmlen.client

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
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
    private var t2sThread: Thread? = null

    companion object {
        const val ACTION_CONNECT = "app.varmlen.client.CONNECT"
        const val ACTION_DISCONNECT = "app.varmlen.client.DISCONNECT"
        const val EXTRA_CONFIG = "config"
        const val EXTRA_SOCKS_PORT = "socksPort"
        const val EXTRA_DNS = "dns"
        const val EXTRA_APPS = "apps"
        const val EXTRA_APPS_ALLOW = "appsAllow"
        private const val CHANNEL = "varmlen_vpn"
        private const val NOTIF_ID = 1
        private const val TUN_ADDR = "10.10.10.2"
        private const val MTU = 8500

        @Volatile
        var running = false
            private set
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_DISCONNECT -> {
                stopAll()
                return START_NOT_STICKY
            }
            ACTION_CONNECT -> {
                val config = intent.getStringExtra(EXTRA_CONFIG)
                val socksPort = intent.getIntExtra(EXTRA_SOCKS_PORT, 10808)
                val dns = intent.getStringExtra(EXTRA_DNS) ?: "1.1.1.1"
                val apps = intent.getStringArrayExtra(EXTRA_APPS) ?: emptyArray()
                val appsAllow = intent.getBooleanExtra(EXTRA_APPS_ALLOW, false)
                if (config == null) {
                    stopSelf(); return START_NOT_STICKY
                }
                try {
                    startAll(config, socksPort, dns, apps, appsAllow)
                } catch (e: Exception) {
                    stopAll()
                }
            }
        }
        return START_STICKY
    }

    private fun startAll(
        config: String, socksPort: Int, dns: String,
        apps: Array<String>, appsAllow: Boolean
    ) {
        startForegroundNotification()

        // 1) xray as a local SOCKS proxy (the generated config binds 127.0.0.1:socksPort).
        val cfgFile = File(filesDir, "xray.json").apply { writeText(config) }
        val xrayBin = File(applicationInfo.nativeLibraryDir, "libxray.so")
        xray = ProcessBuilder(xrayBin.absolutePath, "run", "-c", cfgFile.absolutePath)
            .directory(filesDir)
            .redirectErrorStream(true)
            .start()

        // 2) the tun interface.
        val builder = Builder()
            .setSession("Varmlen")
            .setMtu(MTU)
            .addAddress(TUN_ADDR, 30)
            .addRoute("0.0.0.0", 0)
            .addDnsServer(dns)
        // Per-app split: selective = only these apps; general = all except these.
        for (pkg in apps) {
            try {
                if (appsAllow) builder.addAllowedApplication(pkg)
                else builder.addDisallowedApplication(pkg)
            } catch (_: Exception) { /* app uninstalled */ }
        }
        builder.addDisallowedApplication(packageName) // never tunnel ourselves
        val fd = builder.establish() ?: throw IllegalStateException("establish() failed")
        tun = fd

        // 3) tun2socks: bridge the tun fd to xray's SOCKS inbound (blocking → thread).
        val yaml = """
            tunnel:
              mtu: $MTU
            socks5:
              address: 127.0.0.1
              port: $socksPort
              udp: 'udp'
            misc:
              task-stack-size: 20480
        """.trimIndent()
        t2sThread = Thread {
            TProxy.startTun2socks(yaml, fd.fd)
        }.apply { isDaemon = true; start() }

        running = true
    }

    private fun stopAll() {
        running = false
        try { TProxy.stopTun2socks() } catch (_: Throwable) {}
        t2sThread = null
        xray?.destroy(); xray = null
        try { tun?.close() } catch (_: Throwable) {}
        tun = null
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() {
        stopAll()
        super.onDestroy()
    }

    private fun startForegroundNotification() {
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
        startForeground(NOTIF_ID, notif)
    }
}
