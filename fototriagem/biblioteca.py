"""Varre a origem, deduplica e agrupa por mes.

A deduplicacao existe porque o Takeout guarda uma copia da foto em CADA album
alem da copia na pasta do ano. Sem isso, uma foto que esta em tres albuns seria
triada quatro vezes.

A estrategia de comparacao vem do GPTH: agrupa por tamanho primeiro, que e
gratuito, e so calcula hash dentro dos grupos com mais de um arquivo do mesmo
tamanho. Dois arquivos de tamanhos diferentes nao podem ter o mesmo hash, entao
o hash da maioria nunca chega a ser calculado.
"""

import hashlib
import json
from collections import defaultdict
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path

from . import takeout
from .datas import SUPORTADOS, LeitorDeDatas

CACHE = ".triagem_cache.json"
RELATORIO_COPIAS = "COPIAS-NAO-APAGADAS.txt"
LIXO = "_lixo"

MESES_PT = ["", "janeiro", "fevereiro", "marco", "abril", "maio", "junho",
            "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"]


@dataclass
class Midia:
    caminho: Path
    dt: datetime
    origem: str                     # takeout | exif | video | mtime
    tamanho: int
    copias: list = field(default_factory=list)   # caminhos COMPLETOS das copias identicas
    triada: bool = False                        # ja foi movida nesta sessao

    @property
    def mes(self):
        return (self.dt.year, self.dt.month)

    @property
    def nome(self):
        return self.caminho.name


def _hash(caminho: Path, blocos=1 << 20) -> str:
    h = hashlib.sha256()
    with open(caminho, "rb") as f:
        while bloco := f.read(blocos):
            h.update(bloco)
    return h.hexdigest()


class Biblioteca:
    def __init__(self, origem: Path, destino: Path, incluir_especiais=False):
        self.origem = origem
        self.destino = destino
        self.incluir_especiais = incluir_especiais
        self.itens: list[Midia] = []
        self.duplicatas_removidas = 0
        self.especiais_ignoradas = 0
        self.sem_data: list[Midia] = []

    # ---------- varredura

    def _candidatos(self):
        for p in sorted(self.origem.rglob("*")):
            if not p.is_file() or p.suffix.lower() not in SUPORTADOS:
                continue
            if p.is_relative_to(self.destino / LIXO):
                continue
            # so olha as pastas ENTRE a origem e o arquivo. Antes isso subia por
            # p.parents ate a raiz, entao uma pasta chamada "Archive" em qualquer
            # ponto acima da origem fazia o acervo inteiro ser ignorado em silencio.
            if not self.incluir_especiais:
                try:
                    partes = p.relative_to(self.origem).parts[:-1]
                except ValueError:
                    partes = ()
                if any(parte.strip().lower() in takeout.PASTAS_ESPECIAIS
                       for parte in partes):
                    self.especiais_ignoradas += 1
                    continue
            yield p

    def carregar(self, aviso=print):
        cache_path = self.destino / CACHE
        cache = {}
        if cache_path.exists():
            try:
                cache = json.loads(cache_path.read_text())
            except Exception:
                cache = {}

        leitor = LeitorDeDatas()
        arquivos = list(self._candidatos())
        total = len(arquivos)
        aviso(f"lendo {total} arquivos...")

        itens = []
        novos = 0
        for n, p in enumerate(arquivos, 1):
            try:
                st = p.stat()
            except OSError:
                continue
            assinatura = f"{st.st_size}:{int(st.st_mtime)}"
            reg = cache.get(str(p))
            if reg and reg.get("sig") == assinatura:
                dt = datetime.fromisoformat(reg["dt"])
                origem = reg["src"]
            else:
                dt, origem = leitor.ler(p)
                cache[str(p)] = {"sig": assinatura, "dt": dt.isoformat(), "src": origem}
                novos += 1
                if novos % 50 == 0:
                    aviso(f"  {n}/{total}")
            itens.append(Midia(p, dt, origem, st.st_size))

        self.itens = self._deduplicar(itens)
        self.sem_data = [i for i in self.itens if i.origem == "mtime"]

        self.destino.mkdir(parents=True, exist_ok=True)
        try:
            cache_path.write_text(json.dumps(cache, indent=1))
        except OSError as e:
            aviso(f"aviso: nao gravei o cache: {e}")

        self.itens.sort(key=lambda i: i.dt)
        self.escrever_relatorio_copias()
        return self.itens

    # ---------- deduplicacao

    def _deduplicar(self, itens: list[Midia]) -> list[Midia]:
        por_tamanho = defaultdict(list)
        for i in itens:
            por_tamanho[i.tamanho].append(i)

        saida = []
        for tamanho, grupo in por_tamanho.items():
            if len(grupo) == 1:
                saida.extend(grupo)          # tamanho unico: nem calcula hash
                continue
            por_hash = defaultdict(list)
            for i in grupo:
                try:
                    por_hash[_hash(i.caminho)].append(i)
                except OSError:
                    saida.append(i)
            for iguais in por_hash.values():
                if len(iguais) == 1:
                    saida.extend(iguais)
                    continue
                # mantem a de data mais confiavel; em empate prefere a copia que
                # esta na pasta do ano, que e o lugar canonico do Takeout, e so
                # entao o nome mais curto. Sem isso a copia "oficial" podia acabar
                # sendo a de um album qualquer.
                ordem = {"takeout": 0, "takeout~": 1, "exif": 2, "video": 3, "mtime": 9}
                iguais.sort(key=lambda m: (
                    ordem.get(m.origem, 9),
                    0 if takeout.eh_pasta_de_ano(m.caminho.parent) else 1,
                    len(m.nome),
                    str(m.caminho)))
                vencedora = iguais[0]
                # guarda o caminho completo, nao so o nome da pasta: o objetivo e
                # voce conseguir chegar na copia depois. Nada e apagado aqui.
                vencedora.copias = [m.caminho for m in iguais[1:]]
                self.duplicatas_removidas += len(iguais) - 1
                saida.append(vencedora)
        return saida

    # ---------- agrupamento

    def por_mes(self):
        grupos = defaultdict(list)
        for i in self.itens:
            grupos[(i.dt.year, i.dt.month)].append(i)
        return dict(sorted(grupos.items()))

    @staticmethod
    def _tamanho(n: int) -> str:
        for unidade, div in (("GB", 1 << 30), ("MB", 1 << 20), ("KB", 1 << 10)):
            if n >= div:
                return f"{n / div:.2f} {unidade}"
        return f"{n} B"

    @property
    def com_copias(self):
        return [i for i in self.itens if i.copias]

    def escrever_relatorio_copias(self) -> Path | None:
        """Grava onde estao as copias identicas. Nao apaga nem move nada.

        O Takeout guarda uma copia da foto em cada album alem da copia na pasta
        do ano. A triagem mostra so uma delas, senao voce decidiria a mesma foto
        varias vezes. As outras continuam no lugar, e este arquivo diz onde.
        """
        destino = self.destino / RELATORIO_COPIAS
        com = self.com_copias
        if not com:
            try:
                destino.unlink(missing_ok=True)
            except OSError:
                pass
            return None
        linhas = [
            "COPIAS IDENTICAS ENCONTRADAS NA ORIGEM",
            f"gerado em {datetime.now().strftime('%d/%m/%Y %H:%M')}",
            f"origem: {self.origem}",
            "",
            "Nada foi apagado. A triagem mostra apenas a primeira de cada grupo;",
            "as demais continuam exatamente onde estao, listadas abaixo.",
            "",
            f"{len(com)} arquivos tem copia, somando "
            f"{sum(len(i.copias) for i in com)} copias "
            f"({self._tamanho(sum(i.tamanho * len(i.copias) for i in com))} ocupados a mais)",
            "",
        ]
        for i in sorted(com, key=lambda m: m.dt):
            linhas.append(f"{i.dt.strftime('%Y-%m-%d')}  {i.nome}")
            linhas.append(f"    mostrada : {i.caminho}")
            for c in i.copias:
                linhas.append(f"    copia    : {c}")
            linhas.append("")
        try:
            destino.parent.mkdir(parents=True, exist_ok=True)
            destino.write_text("\n".join(linhas), encoding="utf-8")
            return destino
        except OSError:
            return None

    def resumo(self) -> list[str]:
        linhas = []
        if self.duplicatas_removidas:
            com = self.com_copias
            tam = self._tamanho(sum(i.tamanho * len(i.copias) for i in com))
            linhas.append(
                f"ATENCAO: {self.duplicatas_removidas} copias identicas continuam na "
                f"origem ({tam}). Nao foram apagadas; a triagem so nao as repete. "
                f"Onde estao: {RELATORIO_COPIAS}")
        if self.especiais_ignoradas:
            linhas.append(
                f"{self.especiais_ignoradas} arquivos ignorados em pastas especiais "
                f"(Archive, Trash, Locked Folder)")
        if self.sem_data:
            linhas.append(
                f"{len(self.sem_data)} arquivos sem data confiavel: o mes deles vem do "
                f"mtime, que num Takeout e a data da extracao")
        return linhas
