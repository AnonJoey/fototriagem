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

## A interface de toque

O rascunho pedido ao modelo local nao serviu, entao segue o desenho feito a mao.

O achado que resolve quase tudo: **a interface de toque ja esta desenhada na tela**. A
v1 desktop desenha quatro keycaps mecanicos (`▲ LIXO`, `◄ ANTERIOR`, `▼ PASTAS`,
`► PROXIMO`) que hoje so reagem ao teclado fisico. No celular esses mesmos quatro viram
botoes de verdade. Nao ha interface nova a inventar: ha um controle decorativo que passa
a ser funcional, e o principio do projeto ja esta satisfeito de nascenca, porque toda
acao continua sendo uma coisa visivel na tela.

| Acao | No celular | Controle visivel |
|---|---|---|
| Lixo | tocar o keycap `▲`, ou arrastar a foto para cima | o proprio keycap, no topo do d-pad |
| Menu de pastas | tocar o keycap `▼` | o keycap, e o menu abre como a mesma coluna unica do desktop |
| Anterior / proximo | tocar `◄` `►`, ou arrastar a foto na horizontal | os dois keycaps laterais |
| Play/pause de video | tocar a propria foto | overlay de play sobre o video |
| Desfazer | tocar a faixa de desfazer | faixa fixa apos cada decisao, dizendo o que acabou de acontecer |
| Voltar um nivel | botao no canto superior esquerdo **e** o gesto de voltar do Android | ambos ligados na mesma acao |

O arrastar e sempre **acelerador, nunca o unico caminho**: cada gesto tem o botao ao lado
fazendo o mesmo. Quem nao descobrir gesto nenhum consegue triar a galeria inteira.

### Os tres riscos de disparo sem querer

1. **Arrastar para o lixo sem querer, ao rolar a foto.** O arrasto so decide depois de
   passar um limiar de distancia, e enquanto o dedo esta na tela o destino aparece
   escrito ("solte para mandar ao lixo"). Voltar com o dedo cancela. Nada acontece no
   momento do toque, so no momento de soltar.
2. **O gesto de voltar do Android brigando com a navegacao horizontal.** A borda da tela
   e do sistema, e ponto: o arrasto horizontal so vale na area central da foto, longe das
   duas bordas. Navegar continua sendo tocar os keycaps.
3. **O polegar apoiado disparando um botao ao segurar o aparelho.** O `▲ LIXO`, que e a
   unica acao destrutiva, fica no topo do d-pad, o ponto mais longe de onde o polegar
   descansa. E ele nao decide no toque simples: pede o arrasto, ou uma segunda
   confirmacao na propria faixa.

### Desfazer sem atalho

`CTRL+Z` nao existe num celular, e esconder o desfazer num menu contraria o principio.
Depois de cada decisao, uma faixa fixa no rodape diz o que aconteceu, em texto:

    IMG_0001.jpg → Junho · +2 copias · DESFAZER

Ela nao some sozinha por tempo; e substituida pela proxima decisao. Assim o desfazer e
uma linha visivel, como manda o principio, e de quebra vira o registro do que o app
acabou de fazer, que no celular importa mais do que no desktop porque a tela e pequena e
a decisao e rapida.

### Alcance do polegar

Numa tela de 6 polegadas segurada com uma mao, o alcance confortavel e o terco de baixo,
num arco. Entao: o d-pad no terco inferior, deslocado do centro para o lado da mao; a
faixa de desfazer logo acima dele, ainda dentro do arco; a foto ocupando o resto; e o
voltar no canto superior esquerdo, fora do alcance de proposito, porque o gesto de voltar
do Android ja faz a mesma coisa e alcancar aquele canto nunca deveria ser necessario.

### O que isso implica no codigo

- `src/main.js` hoje escuta `keydown`. Passa a ter uma camada de ponteiro (`pointerdown`,
  `pointermove`, `pointerup`) que resolve nas **mesmas funcoes** que as teclas ja chamam.
  Nenhuma acao nova: so outra porta de entrada para as que existem.
- Os keycaps deixam de ser `<div>` decorativo e viram alvo de toque com area minima de
  48dp, que e o minimo do Android.
- O gesto de voltar do sistema precisa ser interceptado, senao ele fecha o app no meio da
  triagem em vez de subir um nivel.
- `load_image_data`, que devolve a foto inteira em base64, tem que sair antes disso tudo:
  no desktop e desperdicio, no celular e o caminho para o app ser morto por memoria.

Nada disso foi implementado ainda: implementar interface que nao da para experimentar no
aparelho e o mesmo erro de escrever teste junto do codigo e chamar de prova.
