#!/usr/bin/env python3
"""Injeta no AndroidManifest gerado as permissoes que o GS PhotoTriage precisa.

`tauri android init` escreve `src-tauri/gen/android/` do zero e essa pasta e
ignorada pelo git, entao editar o manifesto a mao se perde no proximo init.
Este script e o registro durável: rode depois de todo `tauri android init`.

    python3 scripts/android_permissoes.py

E idempotente: rodar duas vezes nao duplica nada.

Por que MANAGE_EXTERNAL_STORAGE e nao o Storage Access Framework: o nucleo do
app e Rust puro sobre caminhos de arquivo (walkdir para varrer, fs::rename
para mover, filetime para gravar a data). O SAF nao entrega caminho, entrega
um ContentResolver, e sob ele nada disso funciona: seria reescrever o nucleo
inteiro e ainda assim perder o carimbo de data. O preco e que o app so pode
ser instalado por sideload; a Play Store exige justificativa para essa
permissao.
"""

import re
import sys
from pathlib import Path

MANIFESTO = Path(__file__).resolve().parent.parent / "src-tauri/gen/android/app/src/main/AndroidManifest.xml"

# Cada entrada: (linha a inserir, comentario do porque)
PERMISSOES = [
    ('<uses-permission android:name="android.permission.MANAGE_EXTERNAL_STORAGE" />',
     "unica que da caminho de arquivo cru, e portanto mover e carimbar data"),
    ('<uses-permission android:name="android.permission.READ_MEDIA_IMAGES" />',
     "Android 13+ para ler as fotos"),
    ('<uses-permission android:name="android.permission.READ_MEDIA_VIDEO" />',
     "Android 13+ para ler os videos"),
    ('<uses-permission android:name="android.permission.READ_EXTERNAL_STORAGE" android:maxSdkVersion="32" />',
     "Android 12 e abaixo, onde as READ_MEDIA_* ainda nao existem"),
]

# A triagem e uma sessao longa olhando foto por foto: a tela nao pode apagar.
ATRIBUTOS_ACTIVITY = [
    ('android:keepScreenOn', 'true'),
]


def main() -> int:
    if not MANIFESTO.is_file():
        print(f"manifesto nao encontrado: {MANIFESTO}", file=sys.stderr)
        print("rode antes: cd src-tauri && cargo tauri android init", file=sys.stderr)
        return 1

    texto = MANIFESTO.read_text()
    original = texto

    novas = [linha for linha, _ in PERMISSOES if linha.split('android:name="')[1].split('"')[0] not in texto]
    if novas:
        bloco = "\n".join(f"    {linha}" for linha in novas)
        texto = texto.replace("<manifest", "<manifest", 1)
        # entra logo depois da abertura de <manifest ...>
        texto = re.sub(r"(<manifest\b[^>]*>)", r"\1\n" + bloco.replace("\\", "\\\\"), texto, count=1)

    for attr, valor in ATRIBUTOS_ACTIVITY:
        if attr not in texto:
            texto = re.sub(r"(<activity\b)", r'\1\n            %s="%s"' % (attr, valor), texto, count=1)

    if texto != original:
        MANIFESTO.write_text(texto)
        for linha in novas:
            print(f"+ {linha.strip()}")
        print(f"\nescrito em {MANIFESTO}")

    patch_main_activity()
    return 0


def patch_main_activity():
    main_kt = Path(__file__).resolve().parent.parent / "src-tauri/gen/android/app/src/main/java/com/anonjoey/fototriagem/MainActivity.kt"
    if not main_kt.is_file():
        return
    conteudo = main_kt.read_text()
    if "checkStoragePermission" in conteudo:
        return

    kt_code = """package com.anonjoey.fototriagem

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    checkStoragePermission()
  }

  private fun checkStoragePermission() {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
      if (!Environment.isExternalStorageManager()) {
        try {
          val intent = Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
            data = Uri.parse("package:$packageName")
          }
          startActivity(intent)
        } catch (e: Exception) {
          val intent = Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION)
          startActivity(intent)
        }
      }
    }
  }
}
"""
    main_kt.write_text(kt_code)
    print(f"+ patch aplicado em {main_kt}")


if __name__ == "__main__":
    sys.exit(main())
