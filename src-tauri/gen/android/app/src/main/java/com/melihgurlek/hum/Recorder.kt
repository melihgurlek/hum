package com.melihgurlek.hum

import android.net.Uri
import androidx.media3.common.C
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.TransferListener
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.util.concurrent.ConcurrentHashMap

/**
 * The song cache's recorder on Android, standing in for mpv's `stream-record` on the desktop.
 * The engine names a `.part` file for each network stream; the bytes ExoPlayer downloads are
 * copied into it while they arrive in one run from the start. When the stream ends whole, the
 * engine is told (`recorded`) and keeps the file as a saved song, exactly as on the desktop.
 * A gap (a seek past what was downloaded) abandons the copy.
 */
object Recorder {
    private class Take(val path: String) {
        var written = 0L
        var total = C.LENGTH_UNSET.toLong()
        var broken = false
    }

    private val takes = ConcurrentHashMap<String, Take>()
    @Volatile private var swept = false

    /** The engine wants this stream recorded to `path`. */
    fun expect(url: String, path: String) {
        if (takes.containsKey(url)) return
        val file = File(path)
        // Once per run: copies a killed process left half written.
        if (!swept) {
            swept = true
            file.parentFile?.listFiles { f -> f.name.endsWith(".part.m4a") }?.forEach { it.delete() }
        }
        file.parentFile?.mkdirs()
        file.delete()
        takes[url] = Take(path)
    }

    internal fun take(uri: Uri): Any? = takes[uri.toString()]

    internal fun start(t: Any, position: Long, opened: Long): FileOutputStream? {
        t as Take
        if (t.broken || position != t.written) {
            abandon(t)
            return null
        }
        if (position == 0L && opened != C.LENGTH_UNSET.toLong()) t.total = opened
        return FileOutputStream(t.path, true)
    }

    internal fun wrote(t: Any, n: Int) {
        (t as Take).written += n
    }

    /** The stream ran out: whole if it reached the length the server gave. */
    internal fun ended(uri: Uri, t: Any) {
        t as Take
        takes.remove(uri.toString())
        if (t.broken) return
        if (t.total > 0 && t.written == t.total) {
            HumPlayer.event(JSONObject().put("ev", "recorded").put("path", t.path).put("expected", t.total))
        } else {
            File(t.path).delete()
        }
    }

    private fun abandon(t: Take) {
        t.broken = true
        File(t.path).delete()
    }
}

/** Passes reads through and copies them into the stream's recording, if it has one. */
class RecordingDataSource(private val upstream: DataSource) : DataSource {
    private var uri: Uri? = null
    private var take: Any? = null
    private var out: FileOutputStream? = null

    override fun addTransferListener(transferListener: TransferListener) = upstream.addTransferListener(transferListener)

    override fun open(dataSpec: DataSpec): Long {
        val opened = upstream.open(dataSpec)
        uri = dataSpec.uri
        take = Recorder.take(dataSpec.uri)
        out = take?.let { Recorder.start(it, dataSpec.position, opened) }
        return opened
    }

    override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
        val n = upstream.read(buffer, offset, length)
        val t = take
        val o = out
        if (t != null && o != null) {
            if (n > 0) {
                o.write(buffer, offset, n)
                Recorder.wrote(t, n)
            } else if (n == C.RESULT_END_OF_INPUT) {
                o.close()
                out = null
                Recorder.ended(uri!!, t)
                take = null
            }
        }
        return n
    }

    override fun getUri(): Uri? = upstream.uri

    override fun getResponseHeaders(): Map<String, List<String>> = upstream.responseHeaders

    override fun close() {
        try {
            upstream.close()
        } finally {
            out?.close()
            out = null
        }
    }
}

/** Network bytes, reported about once a second for Data used and Auto quality. */
object Downloads : TransferListener {
    private var bytes = 0L
    private var since = 0L

    override fun onTransferInitializing(source: DataSource, dataSpec: DataSpec, isNetwork: Boolean) {}
    override fun onTransferStart(source: DataSource, dataSpec: DataSpec, isNetwork: Boolean) {
        synchronized(this) { if (since == 0L) since = System.currentTimeMillis() }
    }

    override fun onBytesTransferred(source: DataSource, dataSpec: DataSpec, isNetwork: Boolean, bytesTransferred: Int) {
        if (!isNetwork) return
        synchronized(this) {
            bytes += bytesTransferred
            val now = System.currentTimeMillis()
            if (now - since >= 1000) flush(now)
        }
    }

    override fun onTransferEnd(source: DataSource, dataSpec: DataSpec, isNetwork: Boolean) {
        if (!isNetwork) return
        synchronized(this) {
            flush(System.currentTimeMillis())
            since = 0
        }
    }

    private fun flush(now: Long) {
        if (bytes > 0) {
            val secs = ((now - since).coerceAtLeast(1)) / 1000.0
            HumPlayer.event(JSONObject().put("ev", "downloaded").put("bytes", bytes).put("rate", bytes / secs))
        }
        bytes = 0
        since = now
    }
}
