"""CLI entry point for fototriagem."""

import argparse
import sys
from pathlib import Path

from PySide6.QtWidgets import QApplication

from . import __version__
from .biblioteca import Biblioteca
from .ui.janela import Janela


def parse_args(argv=None):
    parser = argparse.ArgumentParser(
        prog="fototriagem",
        description="Triagem ultra-minimalista de fotos e vídeos por teclado, otimizada para Google Takeout.",
    )
    parser.add_argument(
        "origem",
        type=Path,
        help="Pasta de origem contendo as fotos/vídeos (ex: 'Google Photos' do Takeout).",
    )
    parser.add_argument(
        "--destino",
        type=Path,
        default=None,
        help="Pasta de destino onde as novas pastas organizadas serão criadas.",
    )
    parser.add_argument(
        "--incluir-especiais",
        action="store_true",
        help="Inclui pastas especiais normalmente ignoradas (Trash, Archive, Locked Folder).",
    )
    parser.add_argument(
        "-v", "--version",
        action="version",
        version=f"%(prog)s {__version__}",
    )
    return parser.parse_args(argv)


def main(argv=None):
    args = parse_args(argv)
    origem = args.origem.expanduser().resolve()
    if not origem.is_dir():
        print(f"Erro: pasta de origem não encontrada: {origem}", file=sys.stderr)
        return 1

    destino = args.destino.expanduser().resolve() if args.destino else origem

    bib = Biblioteca(origem, destino, incluir_especiais=args.incluir_especiais)
    bib.carregar()

    if not bib.itens:
        print(f"Nenhum arquivo de foto ou vídeo suportado encontrado em: {origem}", file=sys.stderr)
        return 1

    for linha in bib.resumo():
        print(f"  {linha}")

    app = QApplication.instance() or QApplication(sys.argv)
    janela = Janela(bib)
    janela.show()
    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
