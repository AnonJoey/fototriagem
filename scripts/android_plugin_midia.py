#!/usr/bin/env python3
"""Injeta o plugin Kotlin de aviso ao MediaStore dentro de `src-tauri/gen/android/`.

`tauri android init` escreve `src-tauri/gen/android/` do zero e essa pasta e
ignorada pelo git. Kotlin largado la direto se perde no proximo init. Por isso
a fonte de verdade fica em `src-tauri/android/`, rastreada, e este script copia
para dentro do projeto Gradle gerado.

    python3 scripts/android_plugin_midia.py

`scripts/android_permissoes.py` ja chama este script no fim, entao na pratica
basta rodar aquele depois de todo `tauri android init`. E idempotente: rodar
duas vezes nao muda nada.

O que ele coloca onde:

  src-tauri/android/MidiaPlugin.kt
      -> gen/android/app/src/main/java/<pacote>/MidiaPlugin.kt
      O pacote sai da propria linha `package` do arquivo, entao mudar o pacote
      no Kotlin basta: nao ha caminho duplicado aqui para sair de sincronia.

  src-tauri/android/proguard-midia.pro
      -> gen/android/app/proguard-midia.pro
      O `build.gradle.kts` gerado varre `**/*.pro` dentro de `app/` e passa
      tudo para o R8, entao basta o arquivo existir la.

Por que o plugin precisa existir: o nucleo move foto com `fs::rename`, o
MediaProvider nao percebe, e a galeria fica com fantasma no caminho antigo
enquanto o caminho novo nao existe para o sistema. Detalhe completo em
`src-tauri/src/midia_android.rs`.
"""

import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
FONTE = RAIZ / "src-tauri/android"
GEN_ANDROID = RAIZ / "src-tauri/gen/android"

KOTLIN = "MidiaPlugin.kt"
PROGUARD = "proguard-midia.pro"


def pacote_do_kotlin(texto: str) -> str:
    """Le a linha `package ...` do fonte Kotlin.

    O pacote e a unica coisa que o Rust e o Gradle precisam concordar: o Tauri
    acha a classe por `Class.forName("<pacote>.MidiaPlugin")`, e o Gradle so
    compila Kotlin cujo diretorio espelha o pacote.
    """
    achado = re.search(r"^\s*package\s+([A-Za-z0-9_.]+)\s*$", texto, re.MULTILINE)
    if not achado:
        raise ValueError(f"{KOTLIN} nao declara `package`")
    return achado.group(1)


def escrever_se_mudou(destino: Path, conteudo: str) -> bool:
    """Grava so quando o conteudo difere. Devolve True se escreveu."""
    if destino.is_file() and destino.read_text(encoding="utf-8") == conteudo:
        return False
    destino.parent.mkdir(parents=True, exist_ok=True)
    destino.write_text(conteudo, encoding="utf-8")
    return True


def injetar(gen_android: Path, fonte: Path = FONTE) -> list[Path]:
    """Copia os arquivos do plugin para dentro de um `gen/android`.

    Devolve a lista do que foi realmente escrito, vazia quando ja estava tudo
    no lugar. Recebe o diretorio como argumento para que o teste possa apontar
    para uma arvore de mentira sem tocar na de verdade.
    """
    gen_android = Path(gen_android)
    fonte = Path(fonte)

    kotlin = (fonte / KOTLIN).read_text(encoding="utf-8")
    pacote = pacote_do_kotlin(kotlin)

    escritos = []

    destino_kt = gen_android.joinpath(
        "app/src/main/java", *pacote.split("."), KOTLIN
    )
    if escrever_se_mudou(destino_kt, kotlin):
        escritos.append(destino_kt)

    origem_pro = fonte / PROGUARD
    if origem_pro.is_file():
        destino_pro = gen_android / "app" / PROGUARD
        if escrever_se_mudou(destino_pro, origem_pro.read_text(encoding="utf-8")):
            escritos.append(destino_pro)

    limpar_copias_orfas(gen_android, destino_kt)
    return escritos


def limpar_copias_orfas(gen_android: Path, atual: Path) -> None:
    """Apaga MidiaPlugin.kt deixado num pacote antigo.

    Sem isto, renomear o pacote no fonte deixa duas classes no APK e o Gradle
    reclama, ou pior, compila a velha junto.
    """
    raiz_java = gen_android / "app/src/main/java"
    if not raiz_java.is_dir():
        return
    for velho in raiz_java.rglob(KOTLIN):
        if velho.resolve() != atual.resolve():
            velho.unlink()
            print(f"- removido de pacote antigo: {velho}")


def main() -> int:
    if not GEN_ANDROID.is_dir():
        print(f"projeto Android gerado nao encontrado: {GEN_ANDROID}", file=sys.stderr)
        print("rode antes: cd src-tauri && cargo tauri android init", file=sys.stderr)
        return 1

    if not (FONTE / KOTLIN).is_file():
        print(f"fonte do plugin nao encontrada: {FONTE / KOTLIN}", file=sys.stderr)
        return 1

    escritos = injetar(GEN_ANDROID)
    for caminho in escritos:
        print(f"+ {caminho}")
    if not escritos:
        print("plugin de midia ja estava no lugar")
    return 0


if __name__ == "__main__":
    sys.exit(main())
