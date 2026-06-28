package app.varmlen.client

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // Ask for notification permission ONCE (Android 13+). The VPN shows a live
    // status notification with speed + uptime. If the user declines, they can
    // grant it later from Settings (we don't re-prompt on every launch).
    if (Build.VERSION.SDK_INT >= 33) {
      val prefs = getSharedPreferences("varmlen_ui", Context.MODE_PRIVATE)
      val asked = prefs.getBoolean("asked_notif", false)
      if (!asked &&
        checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
      ) {
        try {
          requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1001)
        } catch (_: Throwable) {}
        prefs.edit().putBoolean("asked_notif", true).apply()
      }
    }
  }
}
