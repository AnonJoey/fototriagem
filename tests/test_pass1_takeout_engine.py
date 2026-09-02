"""Test suite for Pass 1: Takeout parsing, sidecars, and date engine."""

import json
import os
import shutil
import tempfile
import unittest
from datetime import datetime
from pathlib import Path
from PIL import Image
from PIL.ExifTags import IFD

from fototriagem import takeout
from fototriagem.datas import LeitorDeDatas, SUPORTADOS, IMAGENS, VIDEOS, ROTULOS


class TestTakeoutEngine(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="test_takeout_"))

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_unicode_normalization(self):
        nfd_str = "Joa\u0303o_fe\u0301rias.jpg"
        nfc_str = "João_férias.jpg"
        self.assertEqual(takeout.nfc(nfd_str), nfc_str)
        self.assertEqual(takeout.nfc(nfc_str), nfc_str)

    def test_sidecar_exact_match(self):
        media = self.tmp / "foto.jpg"
        media.touch()
        sidecar = self.tmp / "foto.jpg.json"
        sidecar.write_text(json.dumps({
            "title": "foto.jpg",
            "photoTakenTime": {"timestamp": "1700000000"}
        }), encoding="utf-8")

        indice = takeout.indexar_pasta(self.tmp)
        arquivos = {p.name for p in self.tmp.iterdir()}
        res = takeout.achar_sidecar(media, indice, arquivos)
        self.assertIsNotNone(res)
        self.assertEqual(res["ts"], 1700000000)
        self.assertTrue(res["exato"])

    def test_sidecar_51_char_truncation(self):
        # 51 chars limit including .json means 46 chars of media name
        long_name = "a" * 60 + ".jpg"
        media = self.tmp / long_name
        media.touch()
        truncated_json_name = long_name[:46] + ".json"
        sidecar = self.tmp / truncated_json_name
        sidecar.write_text(json.dumps({
            "title": long_name,
            "photoTakenTime": {"timestamp": "1710000000"}
        }), encoding="utf-8")

        indice = takeout.indexar_pasta(self.tmp)
        arquivos = {p.name for p in self.tmp.iterdir()}
        res = takeout.achar_sidecar(media, indice, arquivos)
        self.assertIsNotNone(res)
        self.assertEqual(res["ts"], 1710000000)

    def test_sidecar_bracket_swap(self):
        # image(11).jpg -> image.jpg(11).json
        media = self.tmp / "viagem(11).jpg"
        media.touch()
        sidecar = self.tmp / "viagem.jpg(11).json"
        sidecar.write_text(json.dumps({
            "title": "viagem(11).jpg",
            "photoTakenTime": {"timestamp": "1720000000"}
        }), encoding="utf-8")

        indice = takeout.indexar_pasta(self.tmp)
        arquivos = {p.name for p in self.tmp.iterdir()}
        res = takeout.achar_sidecar(media, indice, arquivos)
        self.assertIsNotNone(res)
        self.assertEqual(res["ts"], 1720000000)

    def test_sidecar_bracket_swap_multiple_brackets(self):
        # image(1)(2).jpg -> image(1).jpg(2).json
        media = self.tmp / "image(1)(2).jpg"
        media.touch()
        sidecar = self.tmp / "image(1).jpg(2).json"
        sidecar.write_text(json.dumps({
            "title": "image(1)(2).jpg",
            "photoTakenTime": {"timestamp": "1725000000"}
        }), encoding="utf-8")

        indice = takeout.indexar_pasta(self.tmp)
        arquivos = {p.name for p in self.tmp.iterdir()}
        res = takeout.achar_sidecar(media, indice, arquivos)
        self.assertIsNotNone(res)
        self.assertEqual(res["ts"], 1725000000)

    def test_sidecar_edited_suffixes_multilingual(self):
        test_cases = [
            ("praia-edited.jpg", "praia.jpg.json"),
            ("montanha-modifié.jpg", "montanha.jpg.json"),
            ("festa-bearbeitet.jpg", "festa.jpg.json"),
            ("familia-editado.jpg", "familia.jpg.json"),
            ("tokyo-編集済み.jpg", "tokyo.jpg.json"),
            ("viagem-edytowane.jpg", "viagem.jpg.json"),
            ("show-bewerkt.jpg", "show.jpg.json"),
            ("roma-modificato.jpg", "roma.jpg.json"),
            ("madrid-ha editado.jpg", "madrid.jpg.json"),
            ("barcelona-editat.jpg", "barcelona.jpg.json"),
            ("efeito-effects.jpg", "efeito.jpg.json"),
            ("sorriso-smile.jpg", "sorriso.jpg.json"),
            ("musica-mix.jpg", "musica.jpg.json"),
        ]
        for media_name, json_name in test_cases:
            media = self.tmp / media_name
            media.touch()
            sidecar = self.tmp / json_name
            sidecar.write_text(json.dumps({
                "title": json_name.replace(".json", ""),
                "photoTakenTime": {"timestamp": "1730000000"}
            }), encoding="utf-8")

            indice = takeout.indexar_pasta(self.tmp)
            arquivos = {p.name for p in self.tmp.iterdir()}
            res = takeout.achar_sidecar(media, indice, arquivos)
            self.assertIsNotNone(res, f"Failed for {media_name} -> {json_name}")
            self.assertEqual(res["ts"], 1730000000)

    def test_sidecar_supplemental_metadata(self):
        media = self.tmp / "evento.png"
        media.touch()
        sidecar = self.tmp / "evento.png.supplemental-metadata.json"
        sidecar.write_text(json.dumps({
            "title": "evento.png",
            "creationTime": {"timestamp": "1740000000"}
        }), encoding="utf-8")

        indice = takeout.indexar_pasta(self.tmp)
        arquivos = {p.name for p in self.tmp.iterdir()}
        res = takeout.achar_sidecar(media, indice, arquivos)
        self.assertIsNotNone(res)
        self.assertEqual(res["ts"], 1740000000)
        self.assertFalse(res["exato"])

    def test_sidecar_folder_level_jsons_ignored(self):
        for fname in takeout.JSON_DE_PASTA:
            f = self.tmp / fname
            f.write_text(json.dumps({"title": fname, "photoTakenTime": {"timestamp": "1700000000"}}))
        
        indice = takeout.indexar_pasta(self.tmp)
        self.assertEqual(len(indice), 0)

    def test_sidecar_title_prefix_fallback(self):
        # Real file has long title and JSON has truncated title in 'title' field
        media = self.tmp / "a_very_long_photo_title_from_camera_2024.jpg"
        media.touch()
        sidecar = self.tmp / "random_name.json"
        sidecar.write_text(json.dumps({
            "title": "a_very_long_photo_title_from_camera_",
            "photoTakenTime": {"timestamp": "1750000000"}
        }), encoding="utf-8")

        indice = takeout.indexar_pasta(self.tmp)
        arquivos = {p.name for p in self.tmp.iterdir()}
        res = takeout.achar_sidecar(media, indice, arquivos)
        self.assertIsNotNone(res)
        self.assertEqual(res["ts"], 1750000000)

    def test_folder_classification(self):
        google_photos = self.tmp / "Google Photos"
        google_photos.mkdir()
        year_folder = google_photos / "Photos from 2024"
        year_folder.mkdir()
        album_folder = google_photos / "Viagem a Paris"
        album_folder.mkdir()
        trash_folder = google_photos / "Trash"
        trash_folder.mkdir()

        self.assertTrue(takeout.eh_pasta_de_ano(year_folder))
        self.assertFalse(takeout.eh_pasta_de_ano(album_folder))

        self.assertTrue(takeout.eh_pasta_de_album(album_folder))
        self.assertFalse(takeout.eh_pasta_de_album(year_folder))
        self.assertFalse(takeout.eh_pasta_de_album(trash_folder))

        self.assertTrue(takeout.eh_pasta_especial(trash_folder))
        self.assertFalse(takeout.eh_pasta_especial(album_folder))


class TestDateExtractionEngine(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="test_dates_"))
        self.leitor = LeitorDeDatas()

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_supported_extensions(self):
        self.assertIn(".jpg", SUPORTADOS)
        self.assertIn(".heic", SUPORTADOS)
        self.assertIn(".mp4", SUPORTADOS)
        self.assertIn(".mov", SUPORTADOS)
        self.assertIn(".avif", SUPORTADOS)
        self.assertIn(".jxl", SUPORTADOS)

    def test_exif_datetime_original(self):
        img_path = self.tmp / "test_exif.jpg"
        im = Image.new("RGB", (20, 20), color="blue")
        exif = Image.Exif()
        exif_ifd = exif.get_ifd(IFD.Exif)
        exif_ifd[36867] = "2024:06:15 14:30:45"
        im.save(img_path, "jpeg", exif=exif)

        res = self.leitor.do_exif(img_path)
        self.assertIsNotNone(res)
        dt, tag = res
        self.assertEqual(tag, "exif")
        self.assertEqual(dt, datetime(2024, 6, 15, 14, 30, 45))

    def test_date_cascade_priority(self):
        img_path = self.tmp / "test_prio.jpg"
        im = Image.new("RGB", (20, 20), color="red")
        exif = Image.Exif()
        exif.get_ifd(IFD.Exif)[36867] = "2024:01:01 00:00:00"
        im.save(img_path, "jpeg", exif=exif)

        sidecar = self.tmp / "test_prio.jpg.json"
        sidecar.write_text(json.dumps({
            "title": "test_prio.jpg",
            "photoTakenTime": {"timestamp": "1720000000"}
        }), encoding="utf-8")

        dt, origem = self.leitor.ler(img_path)
        self.assertEqual(origem, "takeout")
        self.assertEqual(dt, datetime.fromtimestamp(1720000000))

    def test_mtime_fallback_for_plain_files(self):
        img_path = self.tmp / "plain.png"
        im = Image.new("RGB", (10, 10), color="green")
        im.save(img_path, "png")

        target_ts = 1600000000
        os.utime(img_path, (target_ts, target_ts))

        dt, origem = self.leitor.ler(img_path)
        self.assertEqual(origem, "mtime")
        self.assertEqual(int(dt.timestamp()), target_ts)


if __name__ == "__main__":
    unittest.main()
