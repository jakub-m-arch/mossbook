package io.github.jakubmarch.mossbook

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class FolderArgs {
  lateinit var uri: String
}

@InvokeArg
class FileArgs {
  lateinit var uri: String
  lateinit var name: String
  /** A file in the app's cache: where `read` puts the file, or what `write` stores. */
  var path: String? = null
}

/**
 * The sync folder on Android (decision D-22): a folder the user picks in the
 * system's folder picker, typically the one Syncthing shares. Access is kept
 * across restarts with a persisted URI permission. Only Rust calls this plugin;
 * none of its commands is granted to the web view.
 *
 * Files pass through the app's private cache because they can be large; their
 * content is already encrypted. Nothing here is logged.
 */
@TauriPlugin
class SyncFolderPlugin(private val activity: Activity) : Plugin(activity) {
  private val suffix = ".mossbook-sync"
  private val resolver get() = activity.contentResolver

  @Command
  fun pick(invoke: Invoke) {
    val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).addFlags(
      Intent.FLAG_GRANT_READ_URI_PERMISSION or
        Intent.FLAG_GRANT_WRITE_URI_PERMISSION or
        Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION
    )
    startActivityForResult(invoke, intent, "picked")
  }

  @ActivityCallback
  fun picked(invoke: Invoke, result: ActivityResult) {
    val tree = result.data?.data
    if (result.resultCode != Activity.RESULT_OK || tree == null) {
      invoke.reject("no folder picked", "cancelled")
      return
    }
    try {
      resolver.takePersistableUriPermission(
        tree,
        Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
      )
      val out = JSObject()
      out.put("uri", tree.toString())
      out.put("name", folderName(tree))
      invoke.resolve(out)
    } catch (e: Exception) {
      invoke.reject("the folder cannot be used", "unavailable")
    }
  }

  /** The folder's name as the system shows it. */
  @Command
  fun name(invoke: Invoke) {
    val args = invoke.parseArgs(FolderArgs::class.java)
    guarded(invoke) {
      val out = JSObject()
      out.put("name", folderName(Uri.parse(args.uri)))
      invoke.resolve(out)
    }
  }

  @Command
  fun list(invoke: Invoke) {
    val args = invoke.parseArgs(FolderArgs::class.java)
    guarded(invoke) {
      val files = JSArray()
      for (child in children(Uri.parse(args.uri))) {
        if (!child.name.endsWith(suffix) || child.name.startsWith(".") ||
          child.name.contains(".sync-conflict")
        ) {
          continue
        }
        val file = JSObject()
        file.put("name", child.name)
        file.put("modified", child.modified)
        file.put("len", child.size)
        files.put(file)
      }
      val out = JSObject()
      out.put("files", files)
      invoke.resolve(out)
    }
  }

  @Command
  fun read(invoke: Invoke) {
    val args = invoke.parseArgs(FileArgs::class.java)
    guarded(invoke) {
      val tree = Uri.parse(args.uri)
      val doc = children(tree).firstOrNull { it.name == args.name }
        ?: throw IllegalStateException("missing")
      resolver.openInputStream(doc.uri(tree)).use { input ->
        File(args.path!!).outputStream().use { output -> input!!.copyTo(output) }
      }
      invoke.resolve()
    }
  }

  /** Writes a temporary file, then puts it in place, so a reader never sees half a file. */
  @Command
  fun write(invoke: Invoke) {
    val args = invoke.parseArgs(FileArgs::class.java)
    guarded(invoke) {
      val tree = Uri.parse(args.uri)
      val parent = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
      val tmpName = ".${args.name}.tmp"
      children(tree).filter { it.name == tmpName }.forEach {
        DocumentsContract.deleteDocument(resolver, it.uri(tree))
      }
      val tmp = DocumentsContract.createDocument(resolver, parent, "application/octet-stream", tmpName)
        ?: throw IllegalStateException("cannot create")
      resolver.openOutputStream(tmp, "wt").use { output ->
        File(args.path!!).inputStream().use { input -> input.copyTo(output!!) }
      }
      children(tree).filter { it.name == args.name }.forEach {
        DocumentsContract.deleteDocument(resolver, it.uri(tree))
      }
      val renamed = try {
        DocumentsContract.renameDocument(resolver, tmp, args.name)
      } catch (e: Exception) {
        null
      }
      if (renamed == null) {
        // A provider without renaming: write in place instead.
        val target = DocumentsContract.createDocument(resolver, parent, "application/octet-stream", args.name)
          ?: throw IllegalStateException("cannot create")
        resolver.openOutputStream(target, "wt").use { output ->
          File(args.path!!).inputStream().use { input -> input.copyTo(output!!) }
        }
        DocumentsContract.deleteDocument(resolver, tmp)
      }
      invoke.resolve()
    }
  }

  @Command
  fun remove(invoke: Invoke) {
    val args = invoke.parseArgs(FileArgs::class.java)
    guarded(invoke) {
      val tree = Uri.parse(args.uri)
      children(tree).filter { it.name == args.name }.forEach {
        DocumentsContract.deleteDocument(resolver, it.uri(tree))
      }
      invoke.resolve()
    }
  }

  private class Child(val id: String, val name: String, val modified: Long, val size: Long) {
    fun uri(tree: Uri): Uri = DocumentsContract.buildDocumentUriUsingTree(tree, id)
  }

  private fun children(tree: Uri): List<Child> {
    val uri = DocumentsContract.buildChildDocumentsUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
    val columns = arrayOf(
      DocumentsContract.Document.COLUMN_DOCUMENT_ID,
      DocumentsContract.Document.COLUMN_DISPLAY_NAME,
      DocumentsContract.Document.COLUMN_LAST_MODIFIED,
      DocumentsContract.Document.COLUMN_SIZE,
    )
    val out = mutableListOf<Child>()
    resolver.query(uri, columns, null, null, null)?.use { c ->
      while (c.moveToNext()) {
        out.add(Child(c.getString(0), c.getString(1) ?: "", c.getLong(2), c.getLong(3)))
      }
    }
    return out
  }

  private fun folderName(tree: Uri): String {
    val doc = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
    resolver.query(doc, arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME), null, null, null)?.use { c ->
      if (c.moveToFirst()) return c.getString(0) ?: ""
    }
    return ""
  }

  /** The folder may have been removed or its permission revoked. */
  private fun guarded(invoke: Invoke, body: () -> Unit) {
    try {
      body()
    } catch (e: Exception) {
      invoke.reject("the sync folder cannot be used", "unavailable")
    }
  }
}
