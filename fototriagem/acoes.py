"""Move, manda para o lixo e desfaz. Toda escrita em disco passa por aqui.

Nenhuma acao apaga arquivo. O 'lixo' e uma pasta, o desfazer devolve ao caminho
original completo, e tudo fica registrado num JSONL para o caso de a sessao
morrer no meio.
"""

import json
import os
import shutil
from datetime import datetime
from pathlib import Path

from .biblioteca import LIXO, Midia

LOG = ".triagem_log.jsonl"


class Acoes:
    def __init__(self, destino: Path):
        self.destino = destino
        self.pilha = []          # [(caminho_original, caminho_atual, midia)]

    # ---------- auxiliares

    def _sem_colisao(self, pasta: Path, nome: str, ignorar: Path | None = None) -> Path:
        alvo = pasta / nome
        if not alvo.exists():
            return alvo
        if ignorar and alvo.resolve() == ignorar.resolve():
            return alvo
        base, ext = os.path.splitext(nome)
        n = 2
        while True:
            cand = pasta / f"{base}_{n}{ext}"
            if not cand.exists() or (ignorar and cand.resolve() == ignorar.resolve()):
                return cand
            n += 1

    def _registrar(self, acao: str, de: Path, para: Path):
        try:
            with open(self.destino / LOG, "a", encoding="utf-8") as f:
                f.write(json.dumps({
                    "quando": datetime.now().isoformat(timespec="seconds"),
                    "acao": acao, "de": str(de), "para": str(para),
                }) + "\n")
        except OSError:
            pass

    # ---------- operacoes

    def mover(self, midia: Midia, pasta: Path) -> Path:
        pasta.mkdir(parents=True, exist_ok=True)
        origem = midia.caminho
        alvo = self._sem_colisao(pasta, origem.name, ignorar=origem)
        if origem != alvo:
            try:
                origem.rename(alvo)
            except OSError:
                shutil.move(str(origem), str(alvo))
        self.pilha.append((origem, alvo, midia))
        self._registrar("mover", origem, alvo)
        midia.caminho = alvo
        midia.triada = True
        return alvo

    def para_o_lixo(self, midia: Midia) -> Path:
        return self.mover(midia, self.destino / LIXO)

    def desfazer(self):
        """Devolve o ultimo arquivo movido ao caminho original completo.

        Devolve a Midia restaurada, ou None se nao havia nada a desfazer ou se
        o arquivo nao esta mais onde foi posto.
        """
        if not self.pilha:
            return None
        origem, atual, midia = self.pilha.pop()
        if not atual.exists():
            return None
        origem.parent.mkdir(parents=True, exist_ok=True)
        alvo_origem = self._sem_colisao(origem.parent, origem.name, ignorar=atual) if (origem.exists() and origem != atual) else origem
        if atual != alvo_origem:
            try:
                atual.rename(alvo_origem)
            except OSError:
                shutil.move(str(atual), str(alvo_origem))
        self._registrar("desfazer", atual, alvo_origem)
        midia.caminho = alvo_origem
        midia.triada = False
        return midia

    @property
    def tem_o_que_desfazer(self) -> bool:
        return bool(self.pilha)
