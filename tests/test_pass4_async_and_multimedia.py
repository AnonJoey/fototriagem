"""Test suite for Pass 4: Async Concurrency, Thumbnail Cache and Multimedia Lifecycle."""

import os
import shutil
import tempfile
import time
import unittest
from pathlib import Path
from PIL import Image

os.environ["QT_QPA_PLATFORM"] = "offscreen"

from PySide6.QtCore import QSize, QThreadPool, QCoreApplication
from PySide6.QtGui import QImage, QPixmap
from PySide6.QtWidgets import QApplication

from fototriagem.ui.miniaturas import CacheDeImagens


class TestAsyncAndMultimedia(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="test_async_"))

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _wait_for_cache(self, cache, key, timeout_sec=2.0):
        start = time.time()
        while time.time() - start < timeout_sec:
            QCoreApplication.processEvents()
            pix = cache._cache.get(key)
            if pix is not None:
                return pix
            time.sleep(0.02)
        return None

    def test_cache_async_loading_and_retrieval(self):
        img_path = self.tmp / "sample.jpg"
        Image.new("RGB", (100, 80), color="yellow").save(img_path)
        
        cache = CacheDeImagens(limite=5)
        size = QSize(50, 40)
        
        # First query: should trigger async load and return None initially
        pix = cache.pegar(img_path, size)
        self.assertIsNone(pix)
        
        # Wait for thread pool to finish decoding
        key = cache._chave(img_path, size)
        pix_loaded = self._wait_for_cache(cache, key)
        
        self.assertIsNotNone(pix_loaded)
        self.assertFalse(pix_loaded.isNull())
        self.assertTrue(pix_loaded.width() <= 50)
        self.assertTrue(pix_loaded.height() <= 40)

        # Second query: instant retrieval from memory cache
        pix2 = cache.pegar(img_path, size)
        self.assertIsNotNone(pix2)
        self.assertFalse(pix2.isNull())

    def test_cache_resizing_key_dimensions(self):
        img_path = self.tmp / "sample_resize.jpg"
        Image.new("RGB", (200, 200), color="purple").save(img_path)
        
        cache = CacheDeImagens(limite=5)
        size1 = QSize(100, 100)
        size2 = QSize(50, 50)
        
        cache.pegar(img_path, size1)
        self._wait_for_cache(cache, cache._chave(img_path, size1))
        
        pix_size2 = cache.pegar(img_path, size2)
        self.assertIsNone(pix_size2)
        pix_size2_loaded = self._wait_for_cache(cache, cache._chave(img_path, size2))
        self.assertIsNotNone(pix_size2_loaded)
        self.assertEqual(pix_size2_loaded.width(), 50)

    def test_cache_lru_eviction(self):
        cache = CacheDeImagens(limite=3)
        size = QSize(30, 30)
        
        paths = []
        for i in range(5):
            p = self.tmp / f"img_{i}.png"
            Image.new("RGB", (20, 20), color="blue").save(p)
            paths.append(p)
            cache.pegar(p, size)
            self._wait_for_cache(cache, cache._chave(p, size))

        self.assertEqual(len(cache._cache), 3)
        self.assertNotIn(cache._chave(paths[0], size), cache._cache)
        self.assertNotIn(cache._chave(paths[1], size), cache._cache)
        self.assertIn(cache._chave(paths[4], size), cache._cache)

    def test_invalid_size_queries(self):
        cache = CacheDeImagens()
        p = self.tmp / "dummy.jpg"
        p.touch()
        
        # Invalid / zero sizes should be ignored gracefully
        self.assertIsNone(cache.pegar(p, QSize(0, 0)))
        self.assertIsNone(cache.pegar(p, QSize(-1, -1)))
        self.assertEqual(len(cache._cache), 0)

    def test_corrupted_file_handling_no_infinite_loading(self):
        corrupt_path = self.tmp / "corrupted.jpg"
        corrupt_path.write_bytes(b"NOT A VALID JPEG FILE AT ALL")
        
        cache = CacheDeImagens()
        size = QSize(40, 40)
        
        cache.pegar(corrupt_path, size)
        key = cache._chave(corrupt_path, size)
        pix = self._wait_for_cache(cache, key)
        
        self.assertIsNotNone(pix)
        self.assertTrue(pix.isNull())
        
        cached_pix = cache.pegar(corrupt_path, size)
        self.assertIsNotNone(cached_pix)
        self.assertTrue(cached_pix.isNull())


if __name__ == "__main__":
    unittest.main()
