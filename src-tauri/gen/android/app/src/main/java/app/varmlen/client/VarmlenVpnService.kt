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
        private const val PREFS = "varmlen_vpn"
        private const val RUNNING_FILE = "running.flag"

        /** Cross-process running flag. The service runs in its own process
         *  (:vpn), so the main process (plugin / tile) can't read a static — it
         *  reads this file in the shared filesDir instead. Doubles as the
         *  "should be running" flag for START_STICKY restart recovery. */
        fun isRunning(ctx: Context): Boolean =
            try { File(ctx.filesDir, RUNNING_FILE).readText() == "1" } catch (_: Throwable) { false }

        private fun setRunning(ctx: Context, on: Boolean) {
            try { File(ctx.filesDir, RUNNING_FILE).writeText(if (on) "1" else "0") } catch (_: Throwable) {}
        }

        /** Whether a previous connect saved a config we can re-launch without
         *  the app being open (used by the Quick Settings tile). */
        fun hasSavedConfig(ctx: Context): Boolean =
            ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString("config", null) != null

        /** Re-launch the VPN from the last saved config (tile / shade). */
        fun start(ctx: Context) {
            val p = ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            val config = p.getString("config", null) ?: return
            val i = Intent(ctx, VarmlenVpnService::class.java).setAction(ACTION_CONNECT)
            i.putExtra(EXTRA_CONFIG, config)
            i.putExtra(EXTRA_SOCKS_PORT, p.getInt("socksPort", 2081))
            i.putExtra(EXTRA_DNS, p.getString("dns", "1.1.1.1"))
            i.putExtra(EXTRA_APPS, (p.getStringSet("apps", emptySet()) ?: emptySet()).toTypedArray())
            i.putExtra(EXTRA_APPS_ALLOW, p.getBoolean("appsAllow", false))
            i.putExtra(EXTRA_LOG_LEVEL, p.getString("logLevel", "warn"))
            ctx.startService(i)
        }

        fun stop(ctx: Context) {
            ctx.startService(
                Intent(ctx, VarmlenVpnService::class.java).setAction(ACTION_DISCONNECT)
            )
        }
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
            else -> {
                // Restarted by the system (START_STICKY, null intent after an
                // OOM kill). Re-establish from the saved config if we were meant
                // to be running; otherwise stop.
                if (isRunning(this) && hasSavedConfig(this)) {
                    log("auto-restart from saved config")
                    start(this)
                } else {
                    stopSelf()
                    return START_NOT_STICKY
                }
            }
        }
        // STICKY: if the OS kills us, restart and (above) re-establish the tunnel.
        return START_STICKY
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        // Keep the VPN alive when the app is swiped from recents — do NOT stop.
        log("task removed — VPN stays up")
        super.onTaskRemoved(rootIntent)
    }

    private fun startAll(
        config: String, socksPort: Int, dns: String,
        apps: Array<String>, appsAllow: Boolean, logLevel: String
    ) {
        log("startAll socksPort=$socksPort dns=$dns apps=${apps.size} allow=$appsAllow level=$logLevel")
        // Remember the connect params so the Quick Settings tile can re-launch
        // the VPN without the app being open.
        getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
            .putString("config", config)
            .putInt("socksPort", socksPort)
            .putString("dns", dns)
            .putStringSet("apps", apps.toSet())
            .putBoolean("appsAllow", appsAllow)
            .putString("logLevel", logLevel)
            .apply()
        // Tear down any previous instance first — a reconnect (e.g. after a split
        // change) must not stack a second xray on the same port or clobber hev's
        // single work thread.
        teardown()
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
        TProxyService.TProxyStartService(hevFile.absolutePath, fd.fd)

        setRunning(this, true)
        log("connected")
    }

    /** Stop hev + xray + the tun, but leave the service running (used to restart). */
    private fun teardown() {
        try { TProxyService.TProxyStopService() } catch (_: Throwable) {}
        try { xray?.destroy() } catch (_: Throwable) {}
        xray = null
        try { tun?.close() } catch (_: Throwable) {}
        tun = null
    }

    private fun stopAll() {
        setRunning(this, false)
        teardown()
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
                // MIN: the foreground-service notice is required by Android to keep
                // the service alive, but kept as unobtrusive as possible (no status
                // icon / sound). With POST_NOTIFICATIONS not granted it's hidden
                // entirely while the service stays foreground.
                nm.createNotificationChannel(
                    NotificationChannel(CHANNEL, "VPN", NotificationManager.IMPORTANCE_MIN)
                )
            }
            val open = PendingIntent.getActivity(
                this, 0, Intent(this, MainActivity::class.java),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
            )
            val stopIntent = PendingIntent.getService(
                this, 1,
                Intent(this, VarmlenVpnService::class.java).setAction(ACTION_DISCONNECT),
                PendingIntent.FLAG_IMMUTABLE
            )
            val notif: Notification = Notification.Builder(this, CHANNEL)
                .setContentTitle("Varmlen")
                .setContentText("VPN active")
                .setSmallIcon(R.drawable.ic_tile)
                .setContentIntent(open)
                .addAction(Notification.Action.Builder(null, "Disconnect", stopIntent).build())
                .setOngoing(true)
                .build()
            if (Build.VERSION.SDK_INT >= 34) {
                startForeground(NOTIF_ID, notif, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
            } else {
                startForeground(NOTIF_ID, notif)
            }
        } catch (e: Throwable) {
            log("startForeground failed (continuing)", e)
        }
    }
}
