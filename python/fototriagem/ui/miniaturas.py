"""Cache de imagens ja decodificadas e escalonadas.

Ideia tomada do picsort: sem isso, cada troca de foto redecodifica o arquivo em
resolucao cheia, e num mes com centenas de fotos de 8 MP a navegacao trava.

Detalhe de Qt: QPixmap so pode ser tocado na thread principal, QImage pode ser
criado em qualquer thread. Por isso o trabalho pesado, decodificar e reescalar,
acontece em QImage numa thread do pool, e a conversao para QPixmap fica na
thread da interface.
"""

from collections import OrderedDict
from pathlib import Path

from PySide6.QtCore import QObject, QRunnable, QSize, Qt, QThreadPool, Signal
from PySide6.QtGui import QImage, QPixmap


class _Sinais(QObject):
    # caminho, largura pedida, altura pedida, imagem (nula se falhou)
    pronto = Signal(str, int, int, QImage)


class _Carga(QRunnable):
    def __init__(self, caminho: Path, tamanho: QSize, sinais: _Sinais):
        super().__init__()
        self.caminho = caminho
        self.tamanho = QSize(tamanho)
        self.sinais = sinais

    def run(self):
        img = QImage(str(self.caminho))
        if not img.isNull() and self.tamanho.isValid() and not self.tamanho.isEmpty():
            img = img.scaled(self.tamanho, Qt.KeepAspectRatio, Qt.SmoothTransformation)
        try:
            # emite mesmo se a imagem for nula: e assim que quem espera descobre
            # que o arquivo e ilegivel, em vez de ficar em "carregando" para sempre
            self.sinais.pronto.emit(
                str(self.caminho), self.tamanho.width(), self.tamanho.height(), img)
        except RuntimeError:
            # a janela foi fechada enquanto esta carga rodava; nada a fazer
            pass


class CacheDeImagens(QObject):
    """Guarda as ultimas N imagens escalonadas e adianta as vizinhas."""

    chegou = Signal(str)

    def __init__(self, limite=24, parent=None):
        super().__init__(parent)
        self.limite = limite
        self._cache = OrderedDict()          # (caminho, l, a) -> QPixmap (pode ser nulo)
        self._na_fila = set()                # (caminho, l, a) em processamento
        self._pool = QThreadPool.globalInstance()
        self._sinais = _Sinais(self)         # filho deste objeto: vive enquanto ele viver
        self._sinais.pronto.connect(self._guardar)

    @staticmethod
    def _chave(caminho, tamanho: QSize):
        return (str(caminho), tamanho.width(), tamanho.height())

    def _guardar(self, caminho: str, larg: int, alt: int, img: QImage):
        chave = (caminho, larg, alt)
        # QPixmap nulo e um resultado valido: significa "tentei e nao da"
        self._cache[chave] = QPixmap.fromImage(img) if not img.isNull() else QPixmap()
        self._cache.move_to_end(chave)
        while len(self._cache) > self.limite:
            self._cache.popitem(last=False)
        self._na_fila.discard(chave)
        self.chegou.emit(caminho)

    def pegar(self, caminho: Path, tamanho: QSize):
        """QPixmap para ESTE tamanho, ou None se ainda nao esta pronto.

        O tamanho faz parte da chave de proposito: devolver o pixmap de um
        tamanho antigo faria a imagem nunca reescalar quando a janela muda.
        Um QPixmap nulo devolvido significa arquivo ilegivel, nao "ainda nao".
        """
        if not tamanho.isValid() or tamanho.isEmpty():
            return None
        chave = self._chave(caminho, tamanho)
        pix = self._cache.get(chave)
        if pix is not None:
            self._cache.move_to_end(chave)
            return pix
        self.adiantar(caminho, tamanho)
        return None

    def adiantar(self, caminho: Path, tamanho: QSize):
        """Decodifica em segundo plano, sem bloquear a interface."""
        if not tamanho.isValid() or tamanho.isEmpty():
            return
        chave = self._chave(caminho, tamanho)
        if chave in self._na_fila or chave in self._cache:
            return
        self._na_fila.add(chave)
        self._pool.start(_Carga(caminho, tamanho, self._sinais))

    def limpar(self):
        self._cache.clear()
        self._na_fila.clear()
