# Porte para Android

Estado em 10/09/2026. O nucleo em Rust do GS PhotoTriage foi escrito sem
saber que iria para um celular, e a boa noticia e que quase tudo dele
atravessa: `takeout.rs`, `library.rs` e `actions.rs` sao Rust puro sobre
caminhos de arquivo, e Tauri 2 compila para Android o mesmo `lib.rs`.

O que segue e o que **nao** atravessa, e o que ja foi feito a respeito.

## A decisao que manda em todas as outras: acesso a arquivo

O nucleo depende de caminho de arquivo cru em tres lugares que nao tem
substituto barato:

| Onde | O que usa | Precisa de |
|---|---|---|
| `library.rs` | `walkdir` para varrer a arvore do Takeout | caminho |
| `actions.rs` | `fs::rename` para mover | caminho |
| `actions.rs` | `filetime::set_file_mtime` para carimbar a data | caminho |

O Storage Access Framework nao entrega caminho, entrega um `ContentResolver`.
Sob SAF os tres morrem juntos, e mesmo reescrevendo o nucleo contra
`DocumentFile` o carimbo de data se perde, que e justamente a coisa que a v1
acabou de consertar.

**Decisao: `MANAGE_EXTERNAL_STORAGE`.** Preco: instalacao por sideload, porque
a Play Store exige justificativa formal para essa permissao. Ganho: o nucleo
inteiro atravessa sem reescrita, e o app faz no celular exatamente o que faz
no desktop.

Permissoes em `scripts/android_permissoes.py`, que se roda depois de todo
`tauri android init` porque `src-tauri/gen/` e regerado e ignorado pelo git.

## O que ja mudou no codigo

- **`ffprobe` nao existe no Android.** `dates.rs::do_container` agora tem um
  ramo `#[cfg(target_os = "android")]` que devolve `None` em vez de tentar
  rodar um processo externo e falhar uma vez por video. La a cadeia de datas
  e Takeout JSON -> EXIF -> mtime, com um elo a menos, e `origem_data` diz
  isso na tela em vez de mentir.
- **`$HOME` nao serve de raiz.** `lib.rs::raiz_padrao()` devolve
  `/storage/emulated/0` no Android (com `/sdcard` de reserva) e a pasta do
  usuario no desktop. Some tambem o `/home/joey` que estava escrito no codigo
  como reserva.
- **Sugestoes de pasta inicial** mudam por plataforma: `Download/`, `Takeout/`,
  `DCIM/` e `Pictures/` no celular, contra a pasta de teste no desktop.
- As flags de WebKitGTK em `main.rs` ja estavam sob `target_os = "linux"`, e
  no Rust o Android e `target_os = "android"`, entao elas nao vazam para la.

Os dois ramos foram compilados: o de Android trocando o `cfg` por `linux` e
rodando `cargo check`, ja que a toolchain de verdade ainda nao existe nesta
maquina.

## O que foi resolvido e implementado

Todos os bloqueios de ambiente e projeto foram superados:

1. **Toolchain Android e Rust completa:**
   - Rustup instalado com alvo `aarch64-linux-android`.
   - Android SDK 35/36 e NDK 27.3.13750724 instalados e configurados.
   - JDK configurado apontando para o JBR do Android Studio (`/opt/android-studio/jbr`, Java 21).
   - Permissões de armazenamento (`MANAGE_EXTERNAL_STORAGE` e `READ_MEDIA_*`) injetadas no manifesto.
2. **Interface de toque e gestos de swipe:**
   - D-pad mecânico convertido em botões de toque com área >= 48dp (`.keycap`).
   - Gestos de deslizar (swipe up para lixeira, swipe horizontal para anterior/próximo, tap central para play/pause de vídeo).
   - Faixa persistente de ação e desfazer (`#triage-action-banner` com `#btn-banner-undo`).
3. **Asset streaming com protocolo Tauri nativo (zero-copy):**
   - Substituída a serialização base64 pelo streaming direto via `convertFileSrc(path)`, eliminando risco de Out-Of-Memory (OOM) em fotos de 12-50MP. Fallback para `load_image_data` em caso de erro.
4. **Gesto e botão de voltar do Android integrados:**
   - Interceptação nativa através de `popstate` e `history.pushState`, permitindo que o gesto de voltar feche modais, suba pastas ou retorne à lista de meses sem encerrar a aplicação acidentalmente.
5. **Caminhos padrão e dados de desenvolvimento saneados:**
   - Removidos mocks falsos de diretórios e fotos; suporte automático a `/storage/emulated/0/Download` e `/storage/emulated/0/Pictures/Organizadas`.
6. **APK gerado e assinado com sucesso:**
   - APK Debug assinado (V2 scheme) gerado em:
     `/home/joey/GS_PhotoTriage-debug.apk`
   - Pronto para sideload em qualquer dispositivo Android (ARM64).
