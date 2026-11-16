package com.melihgurlek.hum

import android.app.PendingIntent
import android.content.Intent
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.ResolvingDataSource
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import org.json.JSONObject

/**
 * Owns ExoPlayer and the media session: a foreground service while playing, with the
 * notification, lock-screen controls and headset buttons Media3 provides. Swiping the app
 * away closes it, music included (MainActivity.onDestroy); it stops itself once nothing plays.
 */
class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        // Downloads counts network bytes for Data used; Recorder copies them into the song cache.
        val http = DefaultHttpDataSource.Factory().setAllowCrossProtocolRedirects(true).setTransferListener(Downloads)
        // Each YouTube client's urls want that client's User-Agent.
        val withAgent = ResolvingDataSource.Factory(http) { spec ->
            HumPlayer.userAgents[spec.uri.toString()]
                ?.let { spec.withAdditionalHeaders(mapOf("User-Agent" to it)) } ?: spec
        }
        val player = ExoPlayer.Builder(this)
            .setMediaSourceFactory(
                DefaultMediaSourceFactory(
                    DataSource.Factory { RecordingDataSource(DefaultDataSource.Factory(this, withAgent).createDataSource()) },
                ),
            )
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
        // No audio offload. It let the CPU sleep while the audio chip decoded (screen off: 63 % of
        // a core down to 4.4 %), but a song streamed from the network then took ~2.5 s to sound:
        // ExoPlayer reported playing after ~0.45 s and the position only moved 2 s later. Without
        // it sound starts 0.35-0.48 s after the play command (S23 FE, 2026-11-16, 7 vs 9 taps).
        // Tapping the notification (or One UI's status-bar media pill) opens hum. Without a
        // session activity the notification had no contentIntent: taps did nothing.
        val open = packageManager.getLaunchIntentForPackage(packageName)!!
            .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_REORDER_TO_FRONT)
        val activity = PendingIntent.getActivity(this, 0, open, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        session = MediaSession.Builder(this, QueuePlayer(player)).setSessionActivity(activity).setCallback(AnyController).build()
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

/**
 * The player the session (notification, lock screen, watches, headset buttons) controls:
 * ExoPlayer, except that Next and Previous go to hum's queue. ExoPlayer holds only the current
 * song and the prefetched next one, so on its own Previous could only restart the song.
 * (Listeners are left alone: wrapping them with Kotlin's `by` drops every Player.Listener
 * callback, since they are Java default methods, and the session went blind.)
 */
private class QueuePlayer(player: ExoPlayer) : ForwardingPlayer(player) {
    override fun seekToNext() = skip("next")
    override fun seekToNextMediaItem() = skip("next")
    override fun seekToPrevious() = skip("previous")
    override fun seekToPreviousMediaItem() = skip("previous")

    private fun skip(ev: String) = HumPlayer.event(JSONObject().put("ev", ev))
}

/**
 * Every app that asks may control playback. Media3 gives full control only to "trusted" apps
 * (the system, media-control permission holders) and read-only access to the rest, so Mi
 * Fitness showed the song on a Redmi Watch while its pause and skip buttons did nothing.
 */
private object AnyController : MediaSession.Callback {
    @androidx.annotation.OptIn(UnstableApi::class)
    override fun onConnect(session: MediaSession, controller: MediaSession.ControllerInfo) =
        MediaSession.ConnectionResult.AcceptedResultBuilder()
            .setAvailableSessionCommands(MediaSession.ConnectionResult.DEFAULT_SESSION_COMMANDS)
            .setAvailablePlayerCommands(MediaSession.ConnectionResult.DEFAULT_PLAYER_COMMANDS)
            .build()
}
