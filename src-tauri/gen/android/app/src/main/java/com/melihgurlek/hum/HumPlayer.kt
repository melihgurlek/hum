package com.melihgurlek.hum

import android.content.ComponentName
import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import org.json.JSONObject
import java.io.File
import java.util.concurrent.ConcurrentHashMap

/**
 * The Rust engine's player on Android. The engine sends JSON commands (player_android.rs) and
 * hears back JSON events; everything else (queue, radio, prefetch, Auto quality) stays in Rust.
 * ExoPlayer lives in [PlaybackService], so playback outlives the activity and the WebView.
 */
object HumPlayer {
    @JvmStatic external fun nativeInit()
    @JvmStatic external fun nativeEvent(json: String)

    private val main = Handler(Looper.getMainLooper())
    private lateinit var app: Context
    private var player: ExoPlayer? = null
    private var controller: com.google.common.util.concurrent.ListenableFuture<MediaController>? = null
    /** Commands that arrived before the service had its player. */
    private val pending = ArrayDeque<JSONObject>()

    /** Request headers per stream url: each YouTube client wants its own User-Agent. */
    val userAgents = ConcurrentHashMap<String, String>()
    private val gains = HashMap<String, Float>()
    private var userVolume = 1f
    private var startedId: String? = null
    private var lastDuration = -1L

    fun init(context: Context) {
        app = context.applicationContext
        nativeInit()
    }

    /** Called by Rust from any thread. */
    @JvmStatic
    fun command(json: String) {
        main.post { apply(JSONObject(json)) }
    }

    private fun send(ev: JSONObject) = nativeEvent(ev.toString())
    private fun ev(name: String) = JSONObject().put("ev", name)

    // Binding a controller is what starts the service and lets it go foreground while playing.
    private fun ensureService() {
        if (controller == null) {
            val token = SessionToken(app, ComponentName(app, PlaybackService::class.java))
            controller = MediaController.Builder(app, token).buildAsync()
        }
    }

    fun attach(p: ExoPlayer) {
        player = p
        p.addListener(listener)
        while (pending.isNotEmpty()) apply(pending.removeFirst())
    }

    fun detach() {
        player?.removeListener(listener)
        player = null
        controller = null
    }

    private fun item(s: JSONObject): MediaItem {
        val url = s.getString("url")
        val id = s.getString("id")
        if (s.optString("userAgent").isNotEmpty()) userAgents[url] = s.getString("userAgent")
        gains[id] = s.optDouble("gain", 1.0).toFloat()
        val uri = if (s.optBoolean("local")) Uri.fromFile(File(url)) else Uri.parse(url)
        // What the notification and lock screen show; Media3 loads the cover from its URL.
        val meta = MediaMetadata.Builder()
            .setTitle(s.optString("title").ifEmpty { null })
            .setArtist(s.optString("artist").ifEmpty { null })
            .setAlbumTitle(s.optString("album").ifEmpty { null })
            .setArtworkUri(s.optString("artwork").ifEmpty { null }?.let(Uri::parse))
            .build()
        return MediaItem.Builder().setUri(uri).setMediaId(id).setMediaMetadata(meta).build()
    }

    private fun applyVolume(p: Player) {
        val id = p.currentMediaItem?.mediaId
        p.volume = userVolume * (gains[id] ?: 1f)
    }

    private fun dropNext(p: Player) {
        while (p.mediaItemCount > p.currentMediaItemIndex + 1) p.removeMediaItem(p.mediaItemCount - 1)
    }

    private fun apply(c: JSONObject) {
        val p = player ?: run {
            pending.addLast(c)
            ensureService()
            return
        }
        when (c.getString("cmd")) {
            "play" -> {
                startedId = null
                lastDuration = -1
                p.setMediaItem(item(c.getJSONObject("stream")))
                if (c.has("start")) p.seekTo((c.getDouble("start") * 1000).toLong())
                applyVolume(p)
                p.prepare()
                p.play()
            }
            "setNext" -> {
                dropNext(p)
                p.addMediaItem(item(c.getJSONObject("stream")))
            }
            "clearNext" -> dropNext(p)
            "setPaused" -> p.playWhenReady = !c.getBoolean("paused")
            "togglePause" -> p.playWhenReady = !p.playWhenReady
            "seek" -> p.seekTo((c.getDouble("secs") * 1000).toLong())
            "volume" -> {
                userVolume = c.getDouble("volume").toFloat()
                applyVolume(p)
            }
            "stop" -> p.stop()
        }
    }

    private val ticker = object : Runnable {
        override fun run() {
            val p = player ?: return
            send(ev("position").put("secs", p.currentPosition / 1000.0))
            if (p.duration > 0 && p.duration != lastDuration) {
                lastDuration = p.duration
                send(ev("duration").put("secs", p.duration / 1000.0))
            }
            if (p.isPlaying) main.postDelayed(this, 1000)
        }
    }

    private val listener = object : Player.Listener {
        override fun onIsPlayingChanged(isPlaying: Boolean) {
            val p = player ?: return
            if (isPlaying) {
                val id = p.currentMediaItem?.mediaId
                if (id != null && id != startedId) {
                    startedId = id
                    send(ev("started").put("id", id))
                }
                main.removeCallbacks(ticker)
                main.post(ticker)
            }
        }

        override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
            send(ev("paused").put("paused", !playWhenReady))
        }

        override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
            val p = player ?: return
            applyVolume(p)
            // A gapless move to the prefetched song: tell the engine it started.
            if (reason == Player.MEDIA_ITEM_TRANSITION_REASON_AUTO && mediaItem != null) {
                startedId = mediaItem.mediaId
                lastDuration = -1
                send(ev("started").put("id", mediaItem.mediaId))
            }
        }

        override fun onPlaybackStateChanged(state: Int) {
            val p = player ?: return
            when (state) {
                Player.STATE_BUFFERING -> if (p.playWhenReady && startedId != null && p.currentPosition > 0) send(ev("stalled"))
                Player.STATE_ENDED -> send(ev("idle"))
                else -> {}
            }
        }

        override fun onPlayerError(error: PlaybackException) {
            send(
                ev("error")
                    .put("id", player?.currentMediaItem?.mediaId)
                    .put("message", "${error.errorCodeName}: ${error.message}"),
            )
        }
    }
}
