#!/usr/bin/env python3
"""Injeta no AndroidManifest gerado as permissoes que o GS PhotoTriage precisa.

`tauri android init` escreve `src-tauri/gen/android/` do zero e essa pasta e
ignorada pelo git, entao editar o manifesto a mao se perde no proximo init.
Este script e o registro durável: rode depois de todo `tauri android init`.

    python3 scripts/android_permissoes.py

E idempotente: rodar duas vezes nao duplica nada.

No fim ele chama `scripts/android_plugin_midia.py`, que injeta o plugin Kotlin
de aviso ao MediaStore pelo mesmo motivo (fonte em `src-tauri/android/`, copia
em `gen/`). Um comando so para os dois assuntos.

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
# O Gradle junta `src/debug/AndroidManifest.xml` so no build de debug.
MANIFESTO_DEBUG = MANIFESTO.parent.parent / "debug" / "AndroidManifest.xml"

# Ate 30/10 o release saia sem INTERNET (o app so mexia em arquivos locais). Desde o Epico 6 (30/10) ela volta ao
# release: o app fala com a API do Google Drive (HTTPS) e com o Immich no PC pela rede local (HTTP). O que limita
# a rede agora e o network_security_config abaixo: HTTPS em toda parte, HTTP so para os enderecos locais listados.
INTERNET = '<uses-permission android:name="android.permission.INTERNET" />'
MANIFESTO_DEBUG_TEXTO = (
    '<?xml version="1.0" encoding="utf-8"?>\n'
    '<manifest xmlns:android="http://schemas.android.com/apk/res/android">\n'
    f'    {INTERNET}\n'
    '</manifest>\n'
)

# Cada entrada: (linha a inserir, comentario do porque)
PERMISSOES = [
    ('<uses-permission android:name="android.permission.INTERNET" />',
     "Google Drive API v3 e comunicacao com Immich no PC local via LAN (Epico 6)"),
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
        texto = re.sub(r"(<manifest\b[^>]*>)", r"\1\n" + bloco.replace("\\", "\\\\"), texto, count=1)

    for attr, valor in ATRIBUTOS_ACTIVITY:
        if attr not in texto:
            texto = re.sub(r"(<activity\b)", r'\1\n            %s="%s"' % (attr, valor), texto, count=1)

    if 'android:networkSecurityConfig="@xml/network_security_config"' not in texto:
        texto = re.sub(r"(<application\b)", r'\1\n        android:networkSecurityConfig="@xml/network_security_config"', texto, count=1)

    if texto != original:
        MANIFESTO.write_text(texto)
        for linha in novas:
            print(f"+ {linha.strip()}")
        print(f"\nescrito em {MANIFESTO}")
    garantir_network_security_config()
    garantir_internet_no_debug()

    patch_main_activity()
    return injetar_plugin_de_midia()


# Android nao aceita faixa de rede (CIDR) no network_security_config: cada <domain> casa so com aquele
# endereco exato. Por isso a lista tem os enderecos reais do PC (medidos em 06/11/2026 com `ip -4 addr`):
# 10.42.0.1 no cabo direto com o servidor e 192.168.68.106 no Wi-Fi (DHCP: se mudar, atualizar aqui).
HOSTS_HTTP_LOCAL = ["localhost", "127.0.0.1", "10.42.0.1", "192.168.68.106"]
NSC = MANIFESTO.parent / "res" / "xml" / "network_security_config.xml"


def texto_network_security_config(hosts: list[str]) -> str:
    dominios = "\n".join(f"        <domain includeSubdomains=\"false\">{h}</domain>" for h in hosts)
    return (
        '<?xml version="1.0" encoding="utf-8"?>\n'
        "<!-- Gerado por scripts/android_permissoes.py. Editar la, nao aqui: gen/ e recriado pelo tauri. -->\n"
        "<network-security-config>\n"
        "    <!-- HTTPS em toda parte (Google Drive API v3) -->\n"
        '    <base-config cleartextTrafficPermitted="false">\n'
        "        <trust-anchors>\n"
        '            <certificates src="system" />\n'
        "        </trust-anchors>\n"
        "    </base-config>\n"
        "    <!-- HTTP so para o Immich no PC, pelos enderecos locais exatos -->\n"
        '    <domain-config cleartextTrafficPermitted="true">\n'
        f"{dominios}\n"
        "    </domain-config>\n"
        "</network-security-config>\n"
    )


def garantir_network_security_config() -> None:
    """Escreve o XML que o manifesto referencia. Idempotente."""
    texto = texto_network_security_config(HOSTS_HTTP_LOCAL)
    if NSC.is_file() and NSC.read_text() == texto:
        return
    NSC.parent.mkdir(parents=True, exist_ok=True)
    NSC.write_text(texto)
    print(f"escrito em {NSC}")


def tirar_internet_do_manifesto_principal(texto: str) -> tuple[str, bool]:
    """Superado em 30/10 no Epico 6: a permissao INTERNET permanece ativa."""
    return texto, False


def garantir_internet_no_debug() -> None:
    """Cria `app/src/debug/AndroidManifest.xml` com INTERNET, para o debug continuar funcionando."""
    if MANIFESTO_DEBUG.is_file() and INTERNET in MANIFESTO_DEBUG.read_text():
        return
    MANIFESTO_DEBUG.parent.mkdir(parents=True, exist_ok=True)
    MANIFESTO_DEBUG.write_text(MANIFESTO_DEBUG_TEXTO)
    print(f"+ INTERNET so no debug: {MANIFESTO_DEBUG}")


def injetar_plugin_de_midia() -> int:
    """Roda o irmao deste script, que injeta o plugin Kotlin do MediaStore.

    Sao dois scripts porque sao dois assuntos, mas um comando so: ninguem vai
    lembrar de rodar o segundo depois de um `tauri android init`.
    """
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import android_plugin_midia

    return android_plugin_midia.main()


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
