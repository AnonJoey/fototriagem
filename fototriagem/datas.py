"""Descobre a data real de uma midia.

Ordem, da fonte mais confiavel para a menos:

    1. photoTakenTime do sidecar do Takeout
    2. DateTimeOriginal do EXIF
    3. creation_time do container, via ffprobe
    4. mtime do arquivo

O sidecar vem primeiro porque e o registro do proprio Google sobre a foto:
sobrevive a edicao, a conversao e a remocao de metadado, e existe ate para HEIC,
que a Pillow nao abre. O mtime e o pior caso porque num Takeout recem extraido
ele e a data da extracao, o que jogaria o acervo inteiro num mes so.
"""

import json
import subprocess
from datetime import datetime
from pathlib import Path

from . import takeout

IMAGENS = {".jpg", ".jpeg", ".png", ".heic", ".heif", ".webp", ".gif",
           ".bmp", ".tif", ".tiff", ".avif", ".jxl"}
VIDEOS = {".mp4", ".mov", ".m4v", ".avi", ".mkv", ".webm", ".3gp", ".mts", ".m2ts"}
SUPORTADOS = IMAGENS | VIDEOS

# rotulos mostrados na barra de status; 'mtime!' leva exclamacao de proposito
ROTULOS = {
    "takeout": "Takeout",
    "takeout~": "Takeout(upload)",
    "exif": "EXIF",
    "video": "video",
    "mtime": "mtime!",
}


class LeitorDeDatas:
    """Guarda o indice de sidecars por pasta, para nao reler a cada arquivo."""

    def __init__(self):
        self._indices = {}
        self._arquivos = {}

    def _contexto(self, pasta: Path):
        if pasta not in self._indices:
            self._indices[pasta] = takeout.indexar_pasta(pasta)
            try:
                self._arquivos[pasta] = {p.name for p in pasta.iterdir() if p.is_file()}
            except OSError:
                self._arquivos[pasta] = set()
        return self._indices[pasta], self._arquivos[pasta]

    # ---------- fontes

    def do_sidecar(self, caminho: Path):
        indice, arquivos = self._contexto(caminho.parent)
        if not indice and not arquivos:
            return None
        reg = takeout.achar_sidecar(caminho, indice, arquivos)
        if not reg:
            return None
        return datetime.fromtimestamp(reg["ts"]), ("takeout" if reg["exato"] else "takeout~")

    @staticmethod
    def do_exif(caminho: Path):
        try:
            from PIL import Image
            from PIL.ExifTags import IFD
            with Image.open(caminho) as im:
                exif = im.getexif()
                if not exif:
                    return None
                try:
                    exif_ifd = exif.get_ifd(IFD.Exif)
                except Exception:
                    exif_ifd = {}
                for tag in (36867, 36868, 306):        # DateTimeOriginal, DateTimeDigitized, DateTime
                    bruto = exif_ifd.get(tag) or exif.get(tag)
                    if bruto:
                        try:
                            txt = str(bruto).strip()[:19]
                            return datetime.strptime(
                                txt, "%Y:%m:%d %H:%M:%S"), "exif"
                        except ValueError:
                            continue
        except Exception:
            pass
        return None

    @staticmethod
    def do_container(caminho: Path):
        try:
            r = subprocess.run(
                ["ffprobe", "-v", "quiet", "-print_format", "json",
                 "-show_format", str(caminho)],
                capture_output=True, text=True, timeout=20)
            if r.returncode != 0:
                return None
            tags = json.loads(r.stdout).get("format", {}).get("tags", {})
            bruto = (tags.get("creation_time")
                     or tags.get("com.apple.quicktime.creationdate"))
            if bruto:
                try:
                    return datetime.fromisoformat(
                        str(bruto).replace("Z", "+00:00")).replace(tzinfo=None), "video"
                except ValueError:
                    pass
        except Exception:
            pass
        return None

    # ---------- cadeia

    def ler(self, caminho: Path):
        """Devolve (datetime, origem). Nunca falha: no pior caso usa o mtime."""
        ext = caminho.suffix.lower()
        if ext in IMAGENS:
            cadeia = (self.do_sidecar, self.do_exif, self.do_container)
        else:
            cadeia = (self.do_sidecar, self.do_container, self.do_exif)
        for fonte in cadeia:
            achado = fonte(caminho)
            if achado:
                return achado
        return datetime.fromtimestamp(caminho.stat().st_mtime), "mtime"
