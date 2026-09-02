"""fototriagem — triagem ultra-minimalista de fotos e vídeos por teclado."""

__version__ = "1.0.0"
__all__ = ["Biblioteca", "Janela", "Acoes", "LeitorDeDatas"]

from .biblioteca import Biblioteca
from .acoes import Acoes
from .datas import LeitorDeDatas
from .ui.janela import Janela
