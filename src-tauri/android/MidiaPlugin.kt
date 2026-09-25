// FONTE DE VERDADE. Este arquivo vive no git, em `src-tauri/android/`.
//
// A copia que o Gradle compila fica em
// `src-tauri/gen/android/app/src/main/java/com/anonjoey/fototriagem/midia/`,
// e essa pasta e regerada por `tauri android init` e ignorada pelo git.
// Quem faz a copia e `scripts/android_plugin_midia.py`, chamado no fim de
// `scripts/android_permissoes.py`. Edite aqui, nunca la.

package com.anonjoey.fototriagem.midia

import android.app.Activity
import android.media.MediaScannerConnection
import android.util.Log
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/**
 * Avisa o indexador de midia do Android sobre os arquivos que o app mexeu.
 *
 * O nucleo em Rust move foto com `fs::rename`, e o MediaProvider nao percebe
 * na hora: a foto continua indexada no caminho antigo (fantasma na galeria) e
 * o caminho novo nao existe para o sistema, entao a pasta organizada pode nem
 * aparecer na lista de pastas do Google Fotos.
 *
 * `MediaScannerConnection.scanFile` resolve os dois lados de uma vez. Para um
 * caminho que ainda existe ele indexa e devolve a Uri; para um caminho que
 * sumiu ele apaga a linha do indice e devolve `null`. Por isso o Rust manda
 * origem e destino juntos.
 *
 * Por que isto e Kotlin e nao Rust: pegar o `Context` a partir do Rust exigia
 * `ndk_context::android_context()`, que ninguem inicializa num app Tauri e que
 * entra em panico atravessando a fronteira JNI, virando SIGABRT. Aqui o
 * `Context` chega pronto, das maos do proprio Tauri, via
 * `api.register_android_plugin`.
 */
@TauriPlugin
class MidiaPlugin(private val activity: Activity) : Plugin(activity) {

  companion object {
    /** Filtro de logcat: `adb logcat -s GSPhotoTriageMidia`. */
    const val TAG = "GSPhotoTriageMidia"
  }

  /**
   * Recebe `{ "caminhos": ["/storage/...", ...] }` e entrega a lista ao
   * scanner. Responde na hora, sem esperar o scanner terminar: o resultado
   * de cada arquivo sai no logcat, e o Rust so precisa saber que a lista foi
   * aceita.
   */
  @Command
  fun scanFile(invoke: Invoke) {
    try {
      val bruto = invoke.getArgs().optJSONArray("caminhos")
      val caminhos = ArrayList<String>()
      if (bruto != null) {
        for (i in 0 until bruto.length()) {
          val caminho = bruto.optString(i, "")
          if (caminho.isNotEmpty()) {
            caminhos.add(caminho)
          }
        }
      }

      if (caminhos.isEmpty()) {
        invoke.resolve(resposta(0))
        return
      }

      Log.i(TAG, "scanFile: ${caminhos.size} caminho(s), primeiro = ${caminhos[0]}")

      MediaScannerConnection.scanFile(
        activity.applicationContext,
        caminhos.toTypedArray(),
        null
      ) { caminho, uri ->
        if (uri == null) {
          Log.i(TAG, "fora do indice: $caminho")
        } else {
          Log.i(TAG, "indexado: $caminho -> $uri")
        }
      }

      invoke.resolve(resposta(caminhos.size))
    } catch (e: Exception) {
      // Nada aqui pode escapar: o PluginManager rejeitaria de qualquer jeito,
      // mas com a mensagem crua. Rejeitar aqui deixa o motivo no logcat.
      Log.e(TAG, "scanFile falhou", e)
      invoke.reject(e.message ?: e.toString())
    }
  }

  private fun resposta(enviados: Int): JSObject {
    val r = JSObject()
    r.put("enviados", enviados)
    return r
  }
}
