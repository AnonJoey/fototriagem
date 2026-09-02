"""Conhecimento sobre a estrutura do Google Takeout.

O que este modulo sabe foi extraido da leitura do GooglePhotosTakeoutHelper,
digerido em Reference/2026-09-01-Como as ferramentas open source resolvem o
Google.md. Nada aqui e adivinhacao: cada estrategia corresponde a um caso real
que aquele projeto encontrou em producao.
"""

import json
import os
import re
import unicodedata
from pathlib import Path

# Sufixos que o Google acrescenta a copias editadas, em dez idiomas. A copia
# editada NAO tem sidecar proprio: ela usa o do arquivo original.
SUFIXOS_EDITADO = [
    "-edited", "-effects", "-smile", "-mix",   # EN
    "-edytowane",                              # PL
    "-bearbeitet",                             # DE
    "-bewerkt",                                # NL
    "-編集済み",                                 # JA
    "-modificato",                             # IT
    "-modifié",                                # FR
    "-ha editado",                             # ES
    "-editat",                                 # CA
    "-editado",                                # PT
]

# JSONs de nivel de pasta: descrevem o album ou a conta, nunca uma midia
JSON_DE_PASTA = {
    "metadata.json",
    "print-subscriptions.json",
    "shared_album_comments.json",
    "user-generated-memory-titles.json",
}

# Pastas que o Takeout traz mas que nao sao acervo comum
PASTAS_ESPECIAIS = {"archive", "trash", "bin", "locked folder", "lixeira", "arquivo"}

_RE_ANO = re.compile(r"^Photos from (20|19|18)\d{2}$")
_RE_PARENTESE = re.compile(r"\(\d+\)\.")
_RE_EXTRA = re.compile(r"(-[A-Za-zÀ-ÖØ-öø-ÿ]+(\(\d\))?)\.\w+$")
_RE_DIGITO = re.compile(r"\(\d\)\.")

# O Google trunca o nome do JSON em 51 caracteres CONTANDO o ".json"
LIMITE_NOME_JSON = 51


def nfc(texto: str) -> str:
    """Normaliza para NFC.

    O macOS grava nomes em NFD, onde 'á' e 'a' + acento combinante. Comparar
    NFD com NFC falha em silencio, e todo nome com acento deixa de casar. Foi
    um defeito real no GPTH (PR 247) e importa aqui porque nomes em portugues
    tem acento com frequencia.
    """
    return unicodedata.normalize("NFC", texto)


# ---------------------------------------------------------------- pastas

def eh_pasta_de_ano(pasta: Path) -> bool:
    """'Photos from 2024' e afins."""
    return bool(_RE_ANO.match(pasta.name))


def eh_pasta_de_album(pasta: Path) -> bool:
    """Uma pasta e album quando e irma de alguma pasta de ano.

    Definicao do GPTH, simples e robusta: o Takeout poe os albums lado a lado
    com as pastas de ano dentro de 'Google Photos'.
    """
    if eh_pasta_de_ano(pasta) or eh_pasta_especial(pasta):
        return False
    try:
        return any(eh_pasta_de_ano(d) for d in pasta.parent.iterdir() if d.is_dir())
    except OSError:
        return False


def eh_pasta_especial(pasta: Path) -> bool:
    return pasta.name.strip().lower() in PASTAS_ESPECIAIS


# ---------------------------------------------------------------- sidecar

def _variantes_do_nome(nome: str):
    """As sete estrategias do GPTH para achar o JSON de uma midia.

    Rende nomes candidatos em ordem de confianca. Quem chama testa cada um
    contra os arquivos que existem e para no primeiro que casar.
    """
    nome = nfc(nome)
    base, ext = os.path.splitext(nome)

    # 1. exato
    yield nome

    # 2. o Google trunca em 51 chars contando ".json"
    if len(nome) + len(".json") > LIMITE_NOME_JSON:
        yield nome[: LIMITE_NOME_JSON - len(".json")]

    # 3. parentese trocado de lugar: image(11).jpg -> image.jpg(11)
    achados = list(_RE_PARENTESE.finditer(nome))
    if achados:
        ultimo = achados[-1].group(0)
        parentese = ultimo.replace(".", "")
        pos = nome.rfind(parentese)
        yield nome[:pos] + nome[pos + len(parentese):] + parentese

    # 4. copia editada: usa o sidecar do original
    minusculo = base.lower()
    for sufixo in SUFIXOS_EDITADO:
        if minusculo.endswith(sufixo):
            yield base[: -len(sufixo)] + ext
            break

    # 5. enviado sem extensao: JSON e "20030616.json", a midia virou "20030616.jpg"
    yield base

    # 6. sufixo de edicao generico, por regex
    m = list(_RE_EXTRA.finditer(nome))
    if len(m) == 1:
        yield nome.replace(m[0].group(1), "")

    # 7. duplicata: "(1)." vira "."
    if _RE_DIGITO.search(nome):
        yield _RE_DIGITO.sub(".", nome)


def indexar_pasta(pasta: Path) -> dict:
    """Le todos os sidecars da pasta e indexa pelo campo 'title'.

    Indexar por conteudo complementa a busca por nome de arquivo: o 'title' de
    dentro do JSON nao sofre a truncagem em 46 caracteres nem a mistura entre
    '<midia>.json' e '<midia>.supplemental-metadata.json' que o Google passou a
    usar no fim de 2024.
    """
    indice = {}
    try:
        arquivos = list(pasta.glob("*.json"))
    except OSError:
        return indice
    for j in arquivos:
        if j.name in JSON_DE_PASTA:
            continue
        try:
            dados = json.loads(j.read_text(encoding="utf-8", errors="replace"))
        except Exception:
            continue
        if not isinstance(dados, dict):
            continue
        titulo = dados.get("title")
        carimbo = dados.get("photoTakenTime") or dados.get("creationTime")
        if not titulo or not isinstance(carimbo, dict) or not carimbo.get("timestamp"):
            continue
        try:
            ts = int(carimbo["timestamp"])
        except (TypeError, ValueError):
            continue
        indice.setdefault(nfc(str(titulo)).lower(), {
            "ts": ts,
            "exato": bool(dados.get("photoTakenTime")),
            "json": j.name,
        })
    return indice


def achar_sidecar(midia: Path, indice: dict, arquivos_da_pasta: set):
    """Acha o registro do sidecar desta midia, ou None.

    Combina os dois caminhos: as variantes de nome do GPTH contra os arquivos
    que realmente existem, e o indice por 'title'. Um cobre o que o outro
    perde.
    """
    for cand in _variantes_do_nome(midia.name):
        chave = nfc(cand).lower()

        # a) o indice por title conhece este nome
        if chave in indice:
            return indice[chave]

        # b) existe um arquivo <cand>.json ou <cand>.supplemental*.json
        for nome_arq in (f"{cand}.json",
                         f"{cand}.supplemental-metadata.json"):
            if nome_arq in arquivos_da_pasta:
                reg = _ler(midia.parent / nome_arq)
                if reg:
                    return reg

    # c) ultimo recurso: title truncado e prefixo do nome real
    alvo = nfc(midia.name).lower()
    for titulo, reg in indice.items():
        if len(titulo) >= 12 and alvo.startswith(titulo[:-1]):
            return reg
    return None


def _ler(caminho: Path):
    try:
        dados = json.loads(caminho.read_text(encoding="utf-8", errors="replace"))
        carimbo = dados.get("photoTakenTime") or dados.get("creationTime")
        return {"ts": int(carimbo["timestamp"]),
                "exato": bool(dados.get("photoTakenTime")),
                "json": caminho.name}
    except Exception:
        return None
