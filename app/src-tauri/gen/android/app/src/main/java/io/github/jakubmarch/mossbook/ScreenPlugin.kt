package io.github.jakubmarch.mossbook

import android.app.Activity
import android.view.WindowManager
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@InvokeArg
class KeepOnArgs {
  var on: Boolean = false
}

/**
 * Keeps the screen on while a voice note is recorded (decision D-25), so the
 * system's screen timeout does not send the app to the background and lock the
 * journal mid-sentence. Only Rust calls this plugin; none of its commands is
 * granted to the web view. The flag belongs to this window only and needs no
 * permission.
 */
@TauriPlugin
class ScreenPlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun keepOn(invoke: Invoke) {
    val args = invoke.parseArgs(KeepOnArgs::class.java)
    activity.runOnUiThread {
      val flag = WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON
      if (args.on) activity.window.addFlags(flag) else activity.window.clearFlags(flag)
      invoke.resolve()
    }
  }
}
