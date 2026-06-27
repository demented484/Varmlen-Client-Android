package app.varmlen.client

import android.app.Activity
import android.content.Intent
import android.net.VpnService
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class ConnectArgs {
    var config: String = ""
    var socksPort: Int = 10808
    var dns: String = "1.1.1.1"
    var apps: Array<String> = arrayOf()
    var appsAllow: Boolean = false
}

/** Tauri bridge: the Rust `vpn_connect`/`vpn_disconnect` commands call into this
 *  on Android to drive the VpnService (with the system consent dialog). */
@TauriPlugin
class VpnPlugin(private val activity: Activity) : Plugin(activity) {
    private var pendingArgs: ConnectArgs? = null

    @Command
    fun connect(invoke: Invoke) {
        val args = invoke.parseArgs(ConnectArgs::class.java)
        val consent = VpnService.prepare(activity)
        if (consent != null) {
            // First run: ask for VPN permission, then start on the result.
            pendingArgs = args
            startActivityForResult(invoke, consent, "onConsent")
            return
        }
        startVpn(args)
        invoke.resolve()
    }

    @ActivityCallback
    fun onConsent(invoke: Invoke, result: ActivityResult) {
        val args = pendingArgs
        pendingArgs = null
        if (result.resultCode == Activity.RESULT_OK && args != null) {
            startVpn(args)
            invoke.resolve()
        } else {
            invoke.reject("VPN permission denied")
        }
    }

    @Command
    fun disconnect(invoke: Invoke) {
        val intent = Intent(activity, VarmlenVpnService::class.java)
        intent.action = VarmlenVpnService.ACTION_DISCONNECT
        activity.startService(intent)
        invoke.resolve()
    }

    @Command
    fun status(invoke: Invoke) {
        val ret = JSObject()
        ret.put("running", VarmlenVpnService.running)
        invoke.resolve(ret)
    }

    private fun startVpn(args: ConnectArgs) {
        val intent = Intent(activity, VarmlenVpnService::class.java)
        intent.action = VarmlenVpnService.ACTION_CONNECT
        intent.putExtra(VarmlenVpnService.EXTRA_CONFIG, args.config)
        intent.putExtra(VarmlenVpnService.EXTRA_SOCKS_PORT, args.socksPort)
        intent.putExtra(VarmlenVpnService.EXTRA_DNS, args.dns)
        intent.putExtra(VarmlenVpnService.EXTRA_APPS, args.apps)
        intent.putExtra(VarmlenVpnService.EXTRA_APPS_ALLOW, args.appsAllow)
        activity.startForegroundService(intent)
    }
}
