package com.melihgurlek.hum

import android.app.PendingIntent
import android.content.Intent
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.TrackSelectionParameters.AudioOffloadPreferences
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.ResolvingDataSource
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

/**
 * Owns ExoPlayer and the media session: a foreground service while playing, with the
 * notification, lock-screen controls and headset buttons Media3 provides. Swiping the app
 * away keeps it playing; it stops itself once nothing is playing.
 */
class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        val http = DefaultHttpDataSource.Factory().setAllowCrossProtocolRedirects(true)
        // Each YouTube client's urls want that client's User-Agent.
        val withAgent = ResolvingDataSource.Factory(http) { spec ->
            HumPlayer.userAgents[spec.uri.toString()]
                ?.let { spec.withAdditionalHeaders(mapOf("User-Agent" to it)) } ?: spec
        }
        val player = ExoPlayer.Builder(this)
            .setMediaSourceFactory(DefaultMediaSourceFactory(DefaultDataSource.Factory(this, withAgent)))
            .setAudioAttributes(
                AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MUSIC).build(),
                true,
            )
            .setHandleAudioBecomingNoisy(true)
            // Start once 0.5 s is buffered, not ExoPlayer's 2.5 s (a video default): measured, the
            // player took ~920 ms from command to sound and the stream URL only ~220 ms. After a
            // stall, wait for 2 s so a weak connection doesn't stutter.
            .setLoadControl(
                DefaultLoadControl.Builder()
                    .setBufferDurationsMs(
                        DefaultLoadControl.DEFAULT_MIN_BUFFER_MS,
                        DefaultLoadControl.DEFAULT_MAX_BUFFER_MS,
                        500,
                        2000,
                    )
                    .build(),
            )
            .setWakeMode(C.WAKE_MODE_NETWORK)
            .build()
        // Audio offload: the audio chip decodes AAC itself and the CPU sleeps between large
        // buffers. Screen off, playing, on the S23 FE: hum plus the system's decoder and audio
        // services went from 63.4 % of a core to 4.4 % (`dumpsys media.audio_flinger` shows an
        // OFFLOAD output). Only where the device keeps songs gapless offloaded; otherwise
        // playback stays as it was.
        player.trackSelectionParameters = player.trackSelectionParameters.buildUpon()
            .setAudioOffloadPreferences(
                AudioOffloadPreferences.Builder()
                    .setAudioOffloadMode(AudioOffloadPreferences.AUDIO_OFFLOAD_MODE_ENABLED)
                    .setIsGaplessSupportRequired(true)
                    .build(),
            )
            .build()
        // Tapping the notification (or One UI's status-bar media pill) opens hum. Without a
        // session activity the notification had no contentIntent: taps did nothing.
        val open = packageManager.getLaunchIntentForPackage(packageName)!!
            .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_REORDER_TO_FRONT)
        val activity = PendingIntent.getActivity(this, 0, open, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        session = MediaSession.Builder(this, player).setSessionActivity(activity).build()
        HumPlayer.attach(player)
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onTaskRemoved(rootIntent: Intent?) {
        val p = session?.player
        if (p == null || !p.playWhenReady || p.mediaItemCount == 0) stopSelf()
    }

    override fun onDestroy() {
        HumPlayer.detach()
        session?.run {
            player.release()
            release()
        }
        session = null
        super.onDestroy()
    }
}
