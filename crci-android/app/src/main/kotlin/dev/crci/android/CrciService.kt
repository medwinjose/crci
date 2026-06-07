package dev.crci.android

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat

class CrciService : Service() {

    companion object {
        const val ACTION_START = "dev.crci.android.ACTION_START"
        const val ACTION_STOP  = "dev.crci.android.ACTION_STOP"
        const val NOTIFICATION_ID = 1001
        const val CHANNEL_ID = "crci_service_channel"
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> startForegroundWithNotification()
            ACTION_STOP  -> stopSelf()
        }
        return START_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun startForegroundWithNotification() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "CRCI Service Channel",
                NotificationManager.IMPORTANCE_LOW
            )
            val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            manager.createNotificationChannel(channel)
        }

        val notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("CRCI")
            .setContentText("CRCI node running")
            // Android 8.0+ requires a valid small icon. Assuming ic_launcher exists,
            // or just use android.R.drawable.ic_dialog_info to be safe if no custom icon.
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()

        startForeground(NOTIFICATION_ID, notification)
    }
}
