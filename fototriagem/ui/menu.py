"""Menu modal ultra-minimalista em coluna unica."""

from pathlib import Path
from PySide6.QtCore import Qt
from PySide6.QtGui import QKeyEvent
from PySide6.QtWidgets import (QDialog, QLabel, QLineEdit, QListWidget,
                               QListWidgetItem, QVBoxLayout)

from ..biblioteca import LIXO
from .theme import MENU_DIALOG_STYLE


class MenuPastas(QDialog):
    ACAO_NOVA = "__nova__"
    ACAO_RENOMEAR = "__renomear__"

    def __init__(self, destino: Path, parent=None):
        super().__init__(parent)
        self.destino = destino
        self.escolhida = None
        self.modo = "principal"
        self.alvo_renomear = None
        
        self.setWindowTitle("Mover para...")
        self.setModal(True)
        self.resize(460, 400)
        self.setStyleSheet(MENU_DIALOG_STYLE)

        lay = QVBoxLayout(self)
        lay.setContentsMargins(18, 16, 18, 16)
        lay.setSpacing(12)

        self.titulo = QLabel("")
        self.titulo.setObjectName("menu_title")
        
        self.lista = QListWidget()
        self.lista.setObjectName("menu_list")
        
        self.campo = QLineEdit()
        self.campo.setObjectName("menu_input")
        self.campo.setPlaceholderText("Nome da pasta...")
        self.campo.hide()
        
        self.dica = QLabel("")
        self.dica.setObjectName("menu_hints")
        
        for w in (self.titulo, self.lista, self.campo, self.dica):
            lay.addWidget(w)
            
        self._principal()

    # ---------- telas

    def _pastas(self):
        if not self.destino.exists() or not self.destino.is_dir():
            return []
        try:
            return [p.name for p in sorted(self.destino.iterdir())
                    if p.is_dir() and not p.name.startswith(".") and p.name != LIXO]
        except OSError:
            return []

    def _item(self, texto, dado):
        it = QListWidgetItem(texto)
        it.setData(Qt.UserRole, dado)
        self.lista.addItem(it)

    def _principal(self, selecionar=None):
        self.modo = "principal"
        self.campo.hide()
        self.campo.clear()
        self.lista.clear()
        self.titulo.setText("Mover para...")
        
        pastas = self._pastas()
        self._item("+  Criar nova pasta", self.ACAO_NOVA)
        if pastas:
            self._item("~  Renomear uma pasta", self.ACAO_RENOMEAR)
        for nome in pastas:
            self._item(f"📁  {nome}", nome)
            
        alvo = 0
        if selecionar:
            for i in range(self.lista.count()):
                if self.lista.item(i).data(Qt.UserRole) == selecionar:
                    alvo = i
                    break
        elif pastas:
            alvo = 2
            
        self.lista.setCurrentRow(alvo)
        self.lista.setFocus()
        self.dica.setText("↑↓ navegar  ·  ⏎ selecionar  ·  esc voltar")

    def _escolher_renomear(self):
        self.modo = "renomear_alvo"
        self.campo.hide()
        self.lista.clear()
        self.titulo.setText("Qual pasta deseja renomear?")
        for nome in self._pastas():
            self._item(f"📁  {nome}", nome)
        if self.lista.count():
            self.lista.setCurrentRow(0)
        self.lista.setFocus()
        self.dica.setText("↑↓ escolher  ·  ⏎ renomear  ·  esc voltar")

    def _digitar(self, inicial=""):
        self.modo = "digitando"
        self.titulo.setText("Renomear pasta" if self.alvo_renomear
                            else "Nome da nova pasta")
        self.campo.setText(inicial)
        self.campo.show()
        self.campo.setFocus()
        if inicial:
            self.campo.selectAll()
        self.dica.setText("digite o nome  ·  ⏎ confirmar  ·  esc voltar")

    # ---------- confirmacao

    def _confirmar(self):
        nome = self.campo.text().strip().strip("/")
        if not nome:
            self._voltar()
            return
        if self.alvo_renomear:
            antiga = self.destino / self.alvo_renomear
            nova = self.destino / nome
            if nova.exists() and nova != antiga:
                self.dica.setText("Já existe uma pasta com esse nome.")
                return
            try:
                if nova != antiga:
                    antiga.rename(nova)
            except OSError as err:
                self.dica.setText(f"Erro ao renomear: {err}")
                return
            self.alvo_renomear = None
            self._principal(selecionar=nome)
        else:
            alvo = self.destino / nome
            alvo.mkdir(parents=True, exist_ok=True)
            self.escolhida = alvo
            self.accept()

    def _voltar(self):
        if self.modo == "digitando":
            if self.alvo_renomear:
                self.alvo_renomear = None
                self._escolher_renomear()
            else:
                self._principal()
        elif self.modo == "renomear_alvo":
            self._principal()
        else:
            self.reject()

    # ---------- teclado

    def keyPressEvent(self, e: QKeyEvent):
        k = e.key()

        if self.modo == "digitando":
            if k in (Qt.Key_Return, Qt.Key_Enter):
                self._confirmar()
            elif k == Qt.Key_Escape:
                self._voltar()
            else:
                self.campo.keyPressEvent(e)
            return

        if k == Qt.Key_Escape:
            self._voltar()
        elif k in (Qt.Key_Down, Qt.Key_Up):
            nova = self.lista.currentRow() + (1 if k == Qt.Key_Down else -1)
            if 0 <= nova < self.lista.count():
                self.lista.setCurrentRow(nova)
        elif k in (Qt.Key_Return, Qt.Key_Enter):
            item = self.lista.currentItem()
            if not item:
                return
            dado = item.data(Qt.UserRole)
            if self.modo == "renomear_alvo":
                self.alvo_renomear = dado
                self._digitar(dado)
            elif dado == self.ACAO_NOVA:
                self.alvo_renomear = None
                self._digitar()
            elif dado == self.ACAO_RENOMEAR:
                self._escolher_renomear()
            else:
                self.escolhida = self.destino / dado
                self.accept()
        else:
            super().keyPressEvent(e)
