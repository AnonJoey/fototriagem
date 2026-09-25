# FONTE DE VERDADE. Copiado para `gen/android/app/proguard-midia.pro` por
# `scripts/android_plugin_midia.py`. O `build.gradle.kts` gerado varre
# `**/*.pro` dentro de `app/`, entao basta o arquivo estar la.
#
# O R8 so roda no build de release (`isMinifyEnabled = true`). A classe
# MidiaPlugin e achada por nome, com `Class.forName`, e o metodo `scanFile` e
# achado por reflexao sobre a anotacao `@Command`: nada disso aparece como
# chamada no bytecode, entao sem estas regras o R8 renomeia ou remove os dois
# e o plugin morre so no APK assinado, nunca no debug.
#
# A regra generica do tauri-android (`-keep @app.tauri.annotation.TauriPlugin
# public class *`) ja deveria cobrir, mas ela depende da anotacao sobreviver a
# analise. Esta aqui e explicita e nao custa nada.

-keep class com.anonjoey.fototriagem.midia.MidiaPlugin {
  public <init>(...);
  @app.tauri.annotation.Command public <methods>;
}
