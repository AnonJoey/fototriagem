"""Test suite for Pass 3: UI, Menu, Navigation, State Machine and Keyboard Ergonomics."""

import os
import shutil
import tempfile
import unittest
from datetime import datetime
from pathlib import Path
from PIL import Image

os.environ["QT_QPA_PLATFORM"] = "offscreen"

from PySide6.QtCore import Qt, QCoreApplication, QEvent
from PySide6.QtGui import QKeyEvent
from PySide6.QtWidgets import QApplication, QDialog

from fototriagem.biblioteca import Biblioteca, Midia, LIXO
from fototriagem.ui.menu import MenuPastas
from fototriagem.ui.janela import Janela


class TestUIAndNavigation(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="test_ui_"))
        self.origem = self.tmp / "origem"
        self.destino = self.tmp / "destino"
        self.origem.mkdir()
        self.destino.mkdir()

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _send_key(self, widget, key, modifier=Qt.NoModifier, text=""):
        event = QKeyEvent(QEvent.KeyPress, key, modifier, text)
        widget.keyPressEvent(event)
        QCoreApplication.processEvents()

    def test_menu_pastas_creation_and_renaming(self):
        (self.destino / "Praia").mkdir()
        
        menu = MenuPastas(self.destino)
        menu.show()
        QCoreApplication.processEvents()
        
        # 1. Navigation down to Praia
        self.assertEqual(menu.modo, "principal")
        self.assertEqual(menu.lista.count(), 3)  # [+ nova, ~ renomear, Praia]
        self.assertEqual(menu.lista.currentRow(), 2)  # Default on first folder
        
        # 2. Select create new folder
        menu.lista.setCurrentRow(0)
        self._send_key(menu, Qt.Key_Return)
        self.assertEqual(menu.modo, "digitando")
        self.assertFalse(menu.campo.isHidden())
        
        # Type folder name
        menu.campo.setText("Viagem 2024")
        self._send_key(menu, Qt.Key_Return)
        
        # New folder created and chosen
        self.assertEqual(menu.result(), QDialog.Accepted)
        self.assertEqual(menu.escolhida, self.destino / "Viagem 2024")
        self.assertTrue((self.destino / "Viagem 2024").is_dir())
        menu.close()

    def test_menu_pastas_rename_flow_and_collision(self):
        (self.destino / "Antiga").mkdir()
        (self.destino / "Existente").mkdir()
        menu = MenuPastas(self.destino)
        menu.show()
        QCoreApplication.processEvents()
        
        # Select "~ renomear uma pasta" (index 1)
        menu.lista.setCurrentRow(1)
        self._send_key(menu, Qt.Key_Return)
        self.assertEqual(menu.modo, "renomear_alvo")
        
        # Select "Antiga" to rename
        menu.lista.setCurrentRow(0)
        self._send_key(menu, Qt.Key_Return)
        self.assertEqual(menu.modo, "digitando")
        self.assertEqual(menu.alvo_renomear, "Antiga")
        
        # Try renaming to already existing name "Existente" -> should show warning
        menu.campo.setText("Existente")
        self._send_key(menu, Qt.Key_Return)
        self.assertTrue("já existe" in menu.dica.text().lower() or "ja existe" in menu.dica.text().lower())
        self.assertEqual(menu.modo, "digitando")

        # Now type a unique new name "Nova"
        menu.campo.setText("Nova")
        self._send_key(menu, Qt.Key_Return)
        
        # Back to main menu with "Nova" selected
        self.assertEqual(menu.modo, "principal")
        self.assertTrue((self.destino / "Nova").is_dir())
        self.assertFalse((self.destino / "Antiga").exists())
        menu.close()

    def test_janela_navigation_and_month_triage_flow(self):
        img_dir = self.origem / "Photos from 2024"
        img_dir.mkdir(parents=True)
        
        f1 = img_dir / "foto1.jpg"
        f2 = img_dir / "foto2.jpg"
        Image.new("RGB", (30, 30), color="blue").save(f1)
        Image.new("RGB", (30, 30), color="red").save(f2)
        
        ts = int(datetime(2024, 5, 15, 12, 0).timestamp())
        os.utime(f1, (ts, ts))
        os.utime(f2, (ts, ts))

        bib = Biblioteca(self.origem, self.destino)
        bib.carregar(aviso=lambda _: None)

        janela = Janela(bib)
        janela.show()
        QCoreApplication.processEvents()

        # Page 0: Months list
        self.assertEqual(janela.paginas.currentIndex(), 0)
        self.assertEqual(janela.lista_meses.count(), 1)
        
        # Press ENTER to open May 2024
        self._send_key(janela, Qt.Key_Return)
        self.assertEqual(janela.paginas.currentIndex(), 1)
        self.assertEqual(len(janela.fila), 2)
        self.assertEqual(janela.pos, 0)
        
        # Press RIGHT to inspect second photo
        self._send_key(janela, Qt.Key_Right)
        self.assertEqual(janela.pos, 1)

        # Press CIMA (Up) to trash the second photo
        self._send_key(janela, Qt.Key_Up)
        self.assertEqual(len(janela.fila), 1)
        self.assertTrue((self.destino / LIXO / "foto2.jpg").exists())

        # Press CTRL+Z to undo
        self._send_key(janela, Qt.Key_Z, modifier=Qt.ControlModifier)
        self.assertEqual(len(janela.fila), 2)
        self.assertTrue(f2.exists())

        # Trash both photos to complete month
        self._send_key(janela, Qt.Key_Up)
        self._send_key(janela, Qt.Key_Up)
        self.assertEqual(len(janela.fila), 0)
        self.assertTrue("concluído" in janela.status.text().lower() or "terminado" in janela.status.text().lower())

        # Press ESC to return to months list
        self._send_key(janela, Qt.Key_Escape)
        self.assertEqual(janela.paginas.currentIndex(), 0)
        
        # Verify month is updated to "concluído"
        txt = janela.lista_meses.item(0).text().lower()
        self.assertTrue("concluído" in txt or "concluido" in txt)
        janela.close()


if __name__ == "__main__":
    unittest.main()
