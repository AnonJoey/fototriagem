"""Test suite for Pass 2: Storage, Scanning, Deduplication, File Actions and Undo."""

import json
import os
import shutil
import tempfile
import unittest
from datetime import datetime
from pathlib import Path
from PIL import Image

from fototriagem.biblioteca import Biblioteca, Midia, LIXO, RELATORIO_COPIAS
from fototriagem.acoes import Acoes, LOG


class TestStorageAndActions(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="test_storage_"))
        self.origem = self.tmp / "origem"
        self.destino = self.tmp / "destino"
        self.origem.mkdir()
        self.destino.mkdir()

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _create_image(self, path: Path, content: bytes = b"test", dt_ts: int = 1700000000):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        os.utime(path, (dt_ts, dt_ts))

    def test_scanning_and_special_folders(self):
        self._create_image(self.origem / "Photos from 2024" / "f1.jpg", content=b"content_f1")
        self._create_image(self.origem / "Album A" / "f2.png", content=b"content_f2")
        
        self._create_image(self.origem / "Trash" / "f3.jpg", content=b"content_f3")
        self._create_image(self.origem / "Archive" / "sub" / "f4.jpg", content=b"content_f4")
        self._create_image(self.origem / "Locked folder" / "f5.jpg", content=b"content_f5")

        bib = Biblioteca(self.origem, self.destino, incluir_especiais=False)
        itens = bib.carregar(aviso=lambda _: None)

        self.assertEqual(len(itens), 2)
        self.assertEqual(bib.especiais_ignoradas, 3)

        bib_all = Biblioteca(self.origem, self.destino, incluir_especiais=True)
        itens_all = bib_all.carregar(aviso=lambda _: None)
        self.assertEqual(len(itens_all), 5)

    def test_deduplication_and_album_copies_report(self):
        content = b"unique_photo_bytes_12345"
        self._create_image(self.origem / "Photos from 2024" / "photo.jpg", content=content)
        self._create_image(self.origem / "Vacation 2024" / "photo.jpg", content=content)
        self._create_image(self.origem / "Favorites" / "photo.jpg", content=content)

        bib = Biblioteca(self.origem, self.destino)
        itens = bib.carregar(aviso=lambda _: None)

        self.assertEqual(len(itens), 1)
        self.assertEqual(bib.duplicatas_removidas, 2)
        
        vencedora = itens[0]
        self.assertEqual(vencedora.caminho.parent.name, "Photos from 2024")
        self.assertEqual(len(vencedora.copias), 2)

        relatorio = self.destino / RELATORIO_COPIAS
        self.assertTrue(relatorio.exists())
        texto = relatorio.read_text(encoding="utf-8")
        self.assertIn("COPIAS IDENTICAS ENCONTRADAS NA ORIGEM", texto)
        self.assertIn("Vacation 2024", texto)
        self.assertIn("Favorites", texto)

        # Test resumo output
        resumo = bib.resumo()
        self.assertTrue(any("copias identicas" in l for l in resumo))

    def test_library_cache_persistence_and_invalidation(self):
        f1 = self.origem / "Photos from 2024" / "f1.jpg"
        self._create_image(f1, dt_ts=1700000000)

        bib1 = Biblioteca(self.origem, self.destino)
        bib1.carregar(aviso=lambda _: None)

        cache_file = self.destino / ".triagem_cache.json"
        self.assertTrue(cache_file.exists())
        cache_data = json.loads(cache_file.read_text())
        self.assertIn(str(f1), cache_data)

        # Load again with warm cache
        bib2 = Biblioteca(self.origem, self.destino)
        itens2 = bib2.carregar(aviso=lambda _: None)
        self.assertEqual(len(itens2), 1)

        # Corrupt cache file and ensure graceful recovery
        cache_file.write_text("{corrupted json!!!")
        bib3 = Biblioteca(self.origem, self.destino)
        itens3 = bib3.carregar(aviso=lambda _: None)
        self.assertEqual(len(itens3), 1)

    def test_size_formatter(self):
        self.assertEqual(Biblioteca._tamanho(500), "500 B")
        self.assertEqual(Biblioteca._tamanho(2048), "2.00 KB")
        self.assertEqual(Biblioteca._tamanho(5 * 1024 * 1024), "5.00 MB")
        self.assertEqual(Biblioteca._tamanho(3 * 1024 * 1024 * 1024), "3.00 GB")

    def test_file_actions_move_collision_and_undo(self):
        f1 = self.origem / "pasta_a" / "pic.jpg"
        self._create_image(f1, content=b"first")
        midia1 = Midia(caminho=f1, dt=datetime(2024, 5, 10), origem="exif", tamanho=len(b"first"))

        acoes = Acoes(self.destino)
        target_dir = self.destino / "Viagem"
        
        # 1. Move to target dir
        alvo1 = acoes.mover(midia1, target_dir)
        self.assertTrue(alvo1.exists())
        self.assertFalse(f1.exists())
        self.assertEqual(midia1.caminho, alvo1)
        self.assertTrue(midia1.triada)

        # 2. Move another file with same name from pasta_b to test destination collision handling (_2)
        f2 = self.origem / "pasta_b" / "pic.jpg"
        self._create_image(f2, content=b"second")
        midia2 = Midia(caminho=f2, dt=datetime(2024, 5, 11), origem="exif", tamanho=len(b"second"))
        alvo2 = acoes.mover(midia2, target_dir)
        self.assertEqual(alvo2.name, "pic_2.jpg")
        self.assertTrue(alvo2.exists())

        # 3. Move to trash
        f3 = self.origem / "trash_me.jpg"
        self._create_image(f3, content=b"garbage")
        midia3 = Midia(caminho=f3, dt=datetime(2024, 5, 12), origem="exif", tamanho=len(b"garbage"))
        alvo3 = acoes.para_o_lixo(midia3)
        self.assertTrue(alvo3.is_relative_to(self.destino / LIXO))

        # 4. Undo operations (LIFO)
        self.assertTrue(acoes.tem_o_que_desfazer)
        restaurado3 = acoes.desfazer()
        self.assertEqual(restaurado3.caminho, f3)
        self.assertTrue(f3.exists())
        self.assertFalse(restaurado3.triada)

        restaurado2 = acoes.desfazer()
        self.assertEqual(restaurado2.caminho, f2)
        self.assertTrue(f2.exists())

        restaurado1 = acoes.desfazer()
        self.assertEqual(restaurado1.caminho, f1)
        self.assertTrue(f1.exists())

        self.assertFalse(acoes.tem_o_que_desfazer)
        self.assertIsNone(acoes.desfazer())

        # Check log integrity
        log_file = self.destino / LOG
        self.assertTrue(log_file.exists())
        lines = [json.loads(l) for l in log_file.read_text(encoding="utf-8").strip().splitlines()]
        self.assertEqual(len(lines), 6)  # 3 moves + 3 undos


if __name__ == "__main__":
    unittest.main()
