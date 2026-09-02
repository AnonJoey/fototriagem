"""Janela principal com interface ultra-minimalista."""

from pathlib import Path

from PySide6.QtCore import Qt, QTimer, QUrl
from PySide6.QtGui import QKeyEvent, QPixmap
from PySide6.QtMultimedia import QAudioOutput, QMediaPlayer
from PySide6.QtMultimediaWidgets import QVideoWidget
from PySide6.QtWidgets import (QDialog, QHBoxLayout, QLabel, QListWidget,
                               QMainWindow, QStackedWidget, QVBoxLayout, QWidget)

from ..acoes import Acoes
from ..biblioteca import LIXO, MESES_PT, Biblioteca
from ..datas import ROTULOS, VIDEOS
from .menu import MenuPastas
from .miniaturas import CacheDeImagens
from .theme import WINDOW_STYLE, STATUS_BAR_STYLE, MONTH_VIEW_STYLE


class Janela(QMainWindow):
    def __init__(self, biblioteca: Biblioteca):
        super().__init__()
        self.bib = biblioteca
        self.acoes = Acoes(biblioteca.destino)
        self.cache = CacheDeImagens()
        self.cache.chegou.connect(self._imagem_chegou)

        self.grupos = biblioteca.por_mes()
        self.fila = []
        self.pos = 0
        self.mes_aberto = None

        self.setWindowTitle("fototriagem")
        self.resize(1200, 820)
        self.setStyleSheet(WINDOW_STYLE + STATUS_BAR_STYLE + MONTH_VIEW_STYLE)

        self.paginas = QStackedWidget()
        self.setCentralWidget(self.paginas)
        self.paginas.addWidget(self._tela_meses())
        self.paginas.addWidget(self._tela_visor())
        self.paginas.setCurrentIndex(0)

    # ---------- telas

    def _atualizar_lista_meses(self):
        linha_antiga = self.lista_meses.currentRow()
        self.lista_meses.clear()
        for (ano, mes), itens in self.grupos.items():
            restantes = [i for i in itens if not i.triada]
            duvidosos = sum(1 for i in restantes if i.origem == "mtime")
            aviso = f"  ·  ⚠️ {duvidosos} sem data real" if duvidosos else ""
            if not restantes:
                status_txt = "✓  concluído"
            elif len(restantes) == len(itens):
                status_txt = f"{len(restantes)} itens"
            else:
                status_txt = f"{len(restantes)} restantes (de {len(itens)})"
            
            nome_mes = MESES_PT[mes].capitalize() if 1 <= mes <= 12 else str(mes)
            self.lista_meses.addItem(
                f"{nome_mes} de {ano}      {status_txt}{aviso}")
                
        if self.lista_meses.count():
            self.lista_meses.setCurrentRow(max(0, min(linha_antiga if linha_antiga >= 0 else 0, self.lista_meses.count() - 1)))

    def _tela_meses(self):
        w = QWidget()
        w.setObjectName("month_container")
        lay = QVBoxLayout(w)
        lay.setContentsMargins(48, 36, 48, 28)
        lay.setSpacing(16)

        header_lay = QVBoxLayout()
        header_lay.setSpacing(4)
        titulo = QLabel("fototriagem")
        titulo.setObjectName("header_title")
        subtitulo = QLabel("Escolha o mês para organizar suas fotos e vídeos")
        subtitulo.setObjectName("header_subtitle")
        header_lay.addWidget(titulo)
        header_lay.addWidget(subtitulo)

        self.lista_meses = QListWidget()
        self.lista_meses.setObjectName("month_list")
        self._atualizar_lista_meses()

        self.resumo = QLabel("\n".join(self.bib.resumo()))
        self.resumo.setObjectName("summary_text")

        dica = QLabel("↑↓ escolher mês   ·   ⏎ iniciar triagem   ·   esc sair")
        dica.setObjectName("footer_hints")

        lay.addLayout(header_lay)
        lay.addWidget(self.lista_meses, 1)
        if self.bib.resumo():
            lay.addWidget(self.resumo)
        lay.addWidget(dica)
        return w

    def _tela_visor(self):
        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(0, 0, 0, 0)
        lay.setSpacing(0)

        self.visor = QStackedWidget()
        self.img = QLabel(alignment=Qt.AlignCenter)
        self.img.setStyleSheet("background-color: #09090b;")
        self.video = QVideoWidget()
        self.video.setStyleSheet("background-color: #09090b;")
        self.visor.addWidget(self.img)
        self.visor.addWidget(self.video)

        self.player = QMediaPlayer()
        self.audio = QAudioOutput()
        self.player.setAudioOutput(self.audio)
        self.player.setVideoOutput(self.video)

        barra = QWidget()
        barra.setObjectName("bottom_bar")
        hl = QHBoxLayout(barra)
        hl.setContentsMargins(20, 10, 20, 10)
        hl.setSpacing(12)

        self.status_pos = QLabel("")
        self.status_pos.setObjectName("status_pos")
        
        self.status_name = QLabel("")
        self.status_name.setObjectName("status_name")

        self.status_date = QLabel("")
        self.status_date.setObjectName("status_date")

        self.status_tag = QLabel("")
        self.status_tag.setObjectName("status_tag")

        self.status_copies = QLabel("")
        self.status_copies.setObjectName("status_copies")
        self.status_copies.hide()

        self.status_msg = QLabel("")
        self.status_msg.setStyleSheet("color: #a1a1aa; font-size: 13px;")
        self.status = self.status_msg

        dica = QLabel("↑ lixo   ↓ mover   ←→ navegar   espaço play/pause   ctrl+z desfaz   esc voltar")
        dica.setObjectName("status_hints")

        hl.addWidget(self.status_pos)
        hl.addWidget(self.status_name)
        hl.addWidget(self.status_date)
        hl.addWidget(self.status_tag)
        hl.addWidget(self.status_copies)
        hl.addWidget(self.status_msg, 1)
        hl.addStretch(1)
        hl.addWidget(dica)

        lay.addWidget(self.visor, 1)
        lay.addWidget(barra)
        return w

    # ---------- exibicao

    def abrir_mes(self, chave):
        self.mes_aberto = chave
        self.fila = [m for m in self.grupos[chave] if not m.triada]
        self.pos = 0
        self.paginas.setCurrentIndex(1)
        self.mostrar()

    def _atual(self):
        if not self.fila:
            return None
        self.pos = max(0, min(self.pos, len(self.fila) - 1))
        return self.fila[self.pos]

    def mostrar(self):
        self.player.stop()
        midia = self._atual()
        if midia is None:
            self.status_pos.setText("")
            self.status_name.setText("")
            self.status_date.setText("")
            self.status_tag.hide()
            self.status_copies.hide()
            self.status_msg.setText("✓ Mês concluído! Pressione ESC para voltar aos meses.")
            self.img.setText("")
            self.img.setPixmap(QPixmap())
            self.visor.setCurrentIndex(0)
            return

        self.status_msg.setText("")
        self.status_pos.setText(f"{self.pos + 1} / {len(self.fila)}")
        self.status_name.setText(midia.nome)
        self.status_date.setText(midia.dt.strftime("%d/%m/%Y %H:%M"))
        
        rotulo = ROTULOS.get(midia.origem, midia.origem.upper())
        self.status_tag.setText(rotulo.upper())
        self.status_tag.show()

        if midia.copias:
            self.status_copies.setText(f"+{len(midia.copias)} cópia(s) na origem")
            self.status_copies.show()
        else:
            self.status_copies.hide()

        if midia.caminho.suffix.lower() in VIDEOS:
            self.visor.setCurrentIndex(1)
            self.player.setSource(QUrl.fromLocalFile(str(midia.caminho)))
            self.player.play()
        else:
            self.visor.setCurrentIndex(0)
            self._pintar(midia)
            self._adiantar_vizinhas()

    def _pintar(self, midia):
        pix = self.cache.pegar(midia.caminho, self.visor.size())
        if pix is None:
            self.img.setText("carregando...")
        elif pix.isNull():
            self.img.setText(f"não foi possível carregar a imagem\n{midia.nome}")
        else:
            self.img.setText("")
            self.img.setPixmap(pix)

    def _imagem_chegou(self, caminho: str):
        midia = self._atual()
        if midia and str(midia.caminho) == caminho:
            self._pintar(midia)

    def _adiantar_vizinhas(self):
        for delta in (1, -1, 2):
            n = self.pos + delta
            if 0 <= n < len(self.fila):
                p = self.fila[n].caminho
                if p.suffix.lower() not in VIDEOS:
                    self.cache.adiantar(p, self.visor.size())

    def resizeEvent(self, e):
        super().resizeEvent(e)
        if self.paginas.currentIndex() == 1:
            QTimer.singleShot(0, self._repintar)

    def _repintar(self):
        midia = self._atual()
        if midia and midia.caminho.suffix.lower() not in VIDEOS:
            self._pintar(midia)

    # ---------- acoes

    def _remover_da_fila(self):
        if self.fila:
            self.fila.pop(self.pos)
        self.mostrar()

    def _mover_para(self, pasta: Path):
        midia = self._atual()
        if midia is None:
            return
        self.player.stop()
        self.acoes.mover(midia, pasta)
        self._remover_da_fila()

    def _desfazer(self):
        midia = self.acoes.desfazer()
        if midia is None:
            self.status_msg.setText("Nada a desfazer.")
            return
        self.player.stop()
        if midia.mes == self.mes_aberto:
            self.fila.insert(self.pos, midia)
            self.mostrar()
        else:
            ano, mes = midia.mes
            self.status_msg.setText(
                f"{midia.nome} restaurado para {MESES_PT[mes]}/{ano}.")

    # ---------- teclado

    def keyPressEvent(self, e: QKeyEvent):
        k = e.key()

        if self.paginas.currentIndex() == 0:
            if k == Qt.Key_Escape:
                self.close()
            elif k in (Qt.Key_Return, Qt.Key_Enter):
                linha = self.lista_meses.currentRow()
                chaves = list(self.grupos.keys())
                if 0 <= linha < len(chaves):
                    self.abrir_mes(chaves[linha])
            elif k in (Qt.Key_Down, Qt.Key_Up):
                nova = self.lista_meses.currentRow() + (1 if k == Qt.Key_Down else -1)
                if 0 <= nova < self.lista_meses.count():
                    self.lista_meses.setCurrentRow(nova)
            return

        if k == Qt.Key_Escape:
            self.player.stop()
            self._atualizar_lista_meses()
            self.paginas.setCurrentIndex(0)
        elif k == Qt.Key_Right:
            self.pos += 1
            self.mostrar()
        elif k == Qt.Key_Left:
            self.pos -= 1
            self.mostrar()
        elif k == Qt.Key_Up:
            self._mover_para(self.bib.destino / LIXO)
        elif k == Qt.Key_Down:
            self.player.pause()
            menu = MenuPastas(self.bib.destino, self)
            if menu.exec() == QDialog.Accepted and menu.escolhida:
                self._mover_para(menu.escolhida)
            else:
                self.mostrar()
        elif k == Qt.Key_Space:
            midia = self._atual()
            if midia and midia.caminho.suffix.lower() in VIDEOS:
                if self.player.playbackState() == QMediaPlayer.PlayingState:
                    self.player.pause()
                else:
                    self.player.play()
        elif k == Qt.Key_Z and (e.modifiers() & Qt.ControlModifier):
            self._desfazer()
        else:
            super().keyPressEvent(e)
