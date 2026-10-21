package io.github.jakubmarch.mossbook

import android.os.Bundle
import android.view.View
import android.view.WindowManager
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // No screenshots, screen recording or app-switcher previews of the journal
    // (SPEC §8.4).
    window.setFlags(WindowManager.LayoutParams.FLAG_SECURE, WindowManager.LayoutParams.FLAG_SECURE)
    keepContentClearOfSystemBars(findViewById(android.R.id.content))
  }

  /**
   * The web view does not get safe-area insets, so the content would run under
   * the status bar, the navigation bar and the keyboard. Pad it natively instead;
   * the window background (themes.xml) matches the page behind the bars.
   */
  private fun keepContentClearOfSystemBars(content: View) {
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
      )
      val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime())
      view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, keyboard.bottom))
      WindowInsetsCompat.CONSUMED
    }
  }
}
