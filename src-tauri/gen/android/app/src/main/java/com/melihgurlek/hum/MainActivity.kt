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
}
