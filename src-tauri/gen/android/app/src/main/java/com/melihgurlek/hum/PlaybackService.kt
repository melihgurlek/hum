package com.melihgurlek.hum

import android.content.Intent
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.ResolvingDataSource
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
            .setWakeMode(C.WAKE_MODE_NETWORK)
            .build()
        session = MediaSession.Builder(this, player).build()
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
