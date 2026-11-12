package com.melihgurlek.hum

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // The Rust library is loaded by now: hand the engine its player.
    HumPlayer.init(this)
  }

  // Rust stops sending the page position updates while it can't be seen.
  override fun onStart() {
    super.onStart()
    nativeVisible(true)
  }

  override fun onStop() {
    nativeVisible(false)
    super.onStop()
  }

  companion object {
    @JvmStatic external fun nativeVisible(visible: Boolean)
  }
}
