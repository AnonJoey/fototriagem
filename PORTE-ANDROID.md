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

## O que falta e depende da maquina

Nada disso da para resolver sem voce, e tudo bloqueia o primeiro APK.

1. **Nao ha `rustup`.** O Rust aqui e o do pacote do Arch (`/usr/bin/rustc`,
   1.98.0), que nao adiciona alvo nenhum. Tauri Android precisa de
   `aarch64-linux-android` e companhia, o que so o rustup instala. Instalar o
   rustup substitui o pacote `rust` do sistema, entao a chamada e sua.
2. **Nao ha SDK nem NDK.** O `android-studio` esta instalado mas nunca baixou
   o SDK: `~/.android` so tem chave de adb. Faltam plataforma, build-tools,
   NDK e as variaveis `ANDROID_HOME` e `NDK_HOME`.
3. **O JDK do sistema e o 26**, novo demais para o Android Gradle Plugin. A
   saida provavel e apontar `JAVA_HOME` para o JBR que veio junto do Android
   Studio, em `/opt/android-studio/jbr`.
4. **Nenhum aparelho conectado.** `adb devices` volta vazio: falta o celular
   plugado com depuracao USB, ou a decisao de so gerar o APK para instalar
   na mao.

## O que ainda e questao de projeto, nao de ambiente

- **A interface e de teclado.** Quatro setas, ESPACO, CTRL+Z e ESC. Num
  celular nada disso existe. O principio do projeto proibe gesto que o usuario
  precise adivinhar, entao cada gesto tera que ter um controle visivel na tela
  que faz a mesma coisa. Rascunho pedido ao modelo local, ainda na fila.
- **`load_image_data` devolve a foto inteira em base64.** No desktop isso e um
  desperdicio; num celular, com foto de 12 megapixels, e o caminho para o app
  ser morto por memoria. Tem que passar a usar o protocolo de asset do Tauri.
- **Os dados falsos de desenvolvimento em `src/main.js`** (por volta das
  linhas 372 e 605) escondem falha de IPC atras de uma tela que parece
  funcionar. No celular, onde a permissao pode simplesmente nao ter sido
  concedida, isso deixa de ser cosmetico.
