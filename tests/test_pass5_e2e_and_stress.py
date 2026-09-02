"""Test suite for Pass 5: End-to-End (E2E) Realistic Simulation & Extreme Stress Testing."""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from datetime import datetime
from pathlib import Path
from PIL import Image
from PIL.ExifTags import IFD

os.environ["QT_QPA_PLATFORM"] = "offscreen"

from PySide6.QtCore import Qt, QCoreApplication, QEvent
from PySide6.QtGui import QKeyEvent
from PySide6.QtWidgets import QApplication

from fototriagem.biblioteca import Biblioteca, LIXO, RELATORIO_COPIAS
from fototriagem.ui.janela import Janela
from fototriagem.ui.menu import MenuPastas


class TestE2EAndStress(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="test_e2e_"))
        self.origem = self.tmp / "Takeout" / "Google Photos"
        self.destino = self.tmp / "Fotos_Organizadas"
        self.origem.mkdir(parents=True)
        self.destino.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _send_key(self, widget, key, modifier=Qt.NoModifier, text=""):
        event = QKeyEvent(QEvent.KeyPress, key, modifier, text)
        widget.keyPressEvent(event)
        QCoreApplication.processEvents()

    def _create_mock_takeout(self):
        # 1. Year 2023 folder
        y23 = self.origem / "Photos from 2023"
        y23.mkdir(parents=True)
        img23 = y23 / "ano_novo.jpg"
        Image.new("RGB", (30, 30), color="blue").save(img23)
        (y23 / "ano_novo.jpg.json").write_text(json.dumps({
            "title": "ano_novo.jpg",
            "photoTakenTime": {"timestamp": str(int(datetime(2023, 1, 1, 10, 0).timestamp()))}
        }), encoding="utf-8")

        # 2. Year 2024 folder
        y24 = self.origem / "Photos from 2024"
        y24.mkdir(parents=True)
        
        # Photo A (Canonical)
        img_a = y24 / "praia.jpg"
        Image.new("RGB", (30, 30), color="yellow").save(img_a)
        (y24 / "praia.jpg.json").write_text(json.dumps({
            "title": "praia.jpg",
            "photoTakenTime": {"timestamp": str(int(datetime(2024, 7, 10, 15, 0).timestamp()))}
        }), encoding="utf-8")

        # Photo B (EXIF-based)
        img_b = y24 / "parque.jpg"
        exif_b = Image.Exif()
        exif_b.get_ifd(IFD.Exif)[36867] = "2024:07:12 11:30:00"
        Image.new("RGB", (30, 30), color="green").save(img_b, exif=exif_b)

        # Photo C (To be trashed)
        img_c = y24 / "borrada.jpg"
        Image.new("RGB", (30, 30), color="black").save(img_c)
        (y24 / "borrada.jpg.json").write_text(json.dumps({
            "title": "borrada.jpg",
            "photoTakenTime": {"timestamp": str(int(datetime(2024, 7, 15, 9, 0).timestamp()))}
        }), encoding="utf-8")

        # 3. Album Folders with duplicate of Photo A
        album1 = self.origem / "Viagem 2024"
        album1.mkdir(parents=True)
        shutil.copy2(img_a, album1 / "praia.jpg")

        album2 = self.origem / "Favoritas"
        album2.mkdir(parents=True)
        shutil.copy2(img_a, album2 / "praia.jpg")

        # 4. Special Folders (Trash, Archive)
        trash = self.origem / "Trash"
        trash.mkdir(parents=True)
        Image.new("RGB", (30, 30), color="gray").save(trash / "lixeira.jpg")

    def test_full_takeout_simulation_and_triage(self):
        self._create_mock_takeout()

        # Step 1: Scan and load library
        bib = Biblioteca(self.origem, self.destino, incluir_especiais=False)
        itens = bib.carregar(aviso=lambda _: None)

        self.assertEqual(len(itens), 4)
        self.assertEqual(bib.duplicatas_removidas, 2)
        self.assertEqual(bib.especiais_ignoradas, 1)

        grupos = bib.por_mes()
        self.assertIn((2023, 1), grupos)
        self.assertIn((2024, 7), grupos)
        self.assertEqual(len(grupos[(2024, 7)]), 3)

        # Step 2: Open UI and triage July 2024
        janela = Janela(bib)
        janela.show()
        QCoreApplication.processEvents()

        janela.lista_meses.setCurrentRow(1)
        self._send_key(janela, Qt.Key_Return)
        self.assertEqual(janela.paginas.currentIndex(), 1)
        self.assertEqual(len(janela.fila), 3)

        # Move item 1 to "Viagem_Praia"
        target_folder = self.destino / "Viagem_Praia"
        janela._mover_para(target_folder)
        self.assertEqual(len(janela.fila), 2)
        self.assertTrue((target_folder / "praia.jpg").exists())

        # Move item 2 to "Viagem_Praia"
        janela._mover_para(target_folder)
        self.assertEqual(len(janela.fila), 1)
        self.assertTrue((target_folder / "parque.jpg").exists())

        # Move item 3 to trash using Up arrow
        self._send_key(janela, Qt.Key_Up)
        self.assertEqual(len(janela.fila), 0)
        self.assertTrue((self.destino / LIXO / "borrada.jpg").exists())
        self.assertTrue("concluído" in janela.status.text().lower() or "concluido" in janela.status.text().lower())

        # Return to months list with ESC
        self._send_key(janela, Qt.Key_Escape)
        self.assertEqual(janela.paginas.currentIndex(), 0)

        # Verify month 2024-07 is marked as concluded
        self.assertTrue("concluído" in janela.lista_meses.item(1).text().lower() or "concluido" in janela.lista_meses.item(1).text().lower())

        # Step 3: Verify disk reports and logs
        self.assertTrue((self.destino / RELATORIO_COPIAS).exists())
        self.assertTrue((self.destino / ".triagem_log.jsonl").exists())
        
        log_lines = [json.loads(l) for l in (self.destino / ".triagem_log.jsonl").read_text().splitlines()]
        self.assertEqual(len(log_lines), 3)
        janela.close()

    def test_cli_entrypoint_arguments(self):
        # Test --help
        r = subprocess.run([sys.executable, "-m", "fototriagem", "--help"], capture_output=True, text=True, cwd=str(Path(__file__).parent.parent))
        self.assertEqual(r.returncode, 0)
        self.assertIn("fototriagem", r.stdout)

        # Test non-existent directory
        r_err = subprocess.run([sys.executable, "-m", "fototriagem", "/non/existent/path"], capture_output=True, text=True, cwd=str(Path(__file__).parent.parent))
        self.assertEqual(r_err.returncode, 1)
        self.assertIn("não encontrada", r_err.stderr)

        # Test missing --destino path
        r_dest_err = subprocess.run([sys.executable, "-m", "fototriagem", str(self.origem), "--destino"], capture_output=True, text=True, cwd=str(Path(__file__).parent.parent))
        self.assertNotEqual(r_dest_err.returncode, 0)

    def test_stress_empty_and_special_paths(self):
        empty_origem = self.tmp / "empty_dir"
        empty_origem.mkdir()
        bib_empty = Biblioteca(empty_origem, self.destino)
        itens = bib_empty.carregar(aviso=lambda _: None)
        self.assertEqual(len(itens), 0)
        self.assertEqual(len(bib_empty.resumo()), 0)

        accent_origem = self.tmp / "Pasta com Espaços e Acentos (São Paulo)"
        accent_origem.mkdir()
        img = accent_origem / "fotografia de são joão.jpg"
        Image.new("RGB", (10, 10), color="cyan").save(img)
        
        bib_accent = Biblioteca(accent_origem, self.destino)
        itens_accent = bib_accent.carregar(aviso=lambda _: None)
        self.assertEqual(len(itens_accent), 1)
        self.assertEqual(itens_accent[0].nome, "fotografia de são joão.jpg")


if __name__ == "__main__":
    unittest.main()
