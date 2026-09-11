"""Testes do plugin Android de aviso ao MediaStore.

O que estes testes NAO fazem: rodar o Kotlin. Nao ha JVM Android nesta
maquina, entao `MediaScannerConnection.scanFile` so pode ser exercitado com o
aparelho na mao. Isso esta assumido, nao disfarcado.

O que eles fazem e cobrir as duas coisas que quebram esta ponte em silencio:

1. **A injecao.** `src-tauri/gen/android/` e regerado por `tauri android init`
   e ignorado pelo git. Se `scripts/android_plugin_midia.py` parar de copiar o
   Kotlin para o lugar certo, o APK sai sem a classe e o aviso morre. O teste
   roda o script de verdade contra uma arvore de mentira e confere o resultado.

2. **A concordancia de nomes.** Rust e Kotlin se acham por texto: o Tauri faz
   `Class.forName("<pacote>.<classe>")` e o `PluginHandle` do Android procura
   o metodo pelo nome do comando. Nada disso e verificado por compilador
   nenhum: renomear um lado compila verde nos dois e so falha no aparelho.
   Os testes de nome comparam os arquivos entre si.

Rodar:

    python3 -m unittest tests.test_android_plugin_midia -v
"""

import re
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(RAIZ / "scripts"))

import android_plugin_midia as injetor  # noqa: E402

KOTLIN = RAIZ / "src-tauri/android/MidiaPlugin.kt"
PROGUARD = RAIZ / "src-tauri/android/proguard-midia.pro"
RUST = RAIZ / "src-tauri/src/midia_android.rs"


def constante_do_rust(nome: str) -> str:
    """Le o valor de uma `pub const <nome>: &str = "..."` em midia_android.rs."""
    texto = RUST.read_text(encoding="utf-8")
    achado = re.search(
        r'pub const ' + re.escape(nome) + r'\s*:\s*&str\s*=\s*"([^"]+)"', texto
    )
    if not achado:
        raise AssertionError(f"{RUST.name} nao declara a constante {nome}")
    return achado.group(1)


class TestInjecaoNoProjetoGerado(unittest.TestCase):
    """O script e o unico motivo de o Kotlin sobreviver a um `android init`."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="gen_android_"))
        (self.tmp / "app/src/main/java").mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_kotlin_cai_no_diretorio_que_espelha_o_pacote(self):
        fonte = KOTLIN.read_text(encoding="utf-8")
        pacote = injetor.pacote_do_kotlin(fonte)

        injetor.injetar(self.tmp)

        esperado = self.tmp.joinpath(
            "app/src/main/java", *pacote.split("."), "MidiaPlugin.kt"
        )
        self.assertTrue(
            esperado.is_file(),
            f"o Gradle so compila Kotlin cujo diretorio espelha o pacote; "
            f"nada em {esperado}",
        )
        self.assertEqual(esperado.read_text(encoding="utf-8"), fonte)

    def test_proguard_vai_para_dentro_de_app(self):
        # O build.gradle.kts gerado varre `**/*.pro` a partir de `app/`.
        injetor.injetar(self.tmp)
        destino = self.tmp / "app/proguard-midia.pro"
        self.assertTrue(destino.is_file(), f"nada em {destino}")
        self.assertEqual(
            destino.read_text(encoding="utf-8"), PROGUARD.read_text(encoding="utf-8")
        )

    def test_segunda_passada_nao_reescreve_nada(self):
        primeira = injetor.injetar(self.tmp)
        self.assertTrue(primeira, "a primeira passada deveria ter escrito algo")
        self.assertEqual(
            injetor.injetar(self.tmp),
            [],
            "rodar de novo nao pode reescrever: o script roda junto com o de permissoes",
        )

    def test_copia_em_pacote_antigo_e_removida(self):
        orfao = self.tmp / "app/src/main/java/com/exemplo/velho/MidiaPlugin.kt"
        orfao.parent.mkdir(parents=True)
        orfao.write_text("package com.exemplo.velho\n", encoding="utf-8")

        injetor.injetar(self.tmp)

        self.assertFalse(
            orfao.exists(),
            "duas MidiaPlugin no APK: o Gradle compila as duas e o Class.forName "
            "pode achar a errada",
        )

    def test_fonte_sem_package_e_erro_explicito(self):
        falsa = Path(tempfile.mkdtemp(prefix="fonte_ruim_"))
        try:
            (falsa / "MidiaPlugin.kt").write_text("class MidiaPlugin\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                injetor.injetar(self.tmp, fonte=falsa)
        finally:
            shutil.rmtree(falsa, ignore_errors=True)


class TestNomesQueSoFalhamNoAparelho(unittest.TestCase):
    """Rust e Kotlin se acham por string. Compilador nenhum confere isso."""

    def test_pacote_do_kotlin_bate_com_a_constante_do_rust(self):
        pacote = injetor.pacote_do_kotlin(KOTLIN.read_text(encoding="utf-8"))
        self.assertEqual(
            pacote,
            constante_do_rust("PACOTE_KOTLIN"),
            "register_android_plugin faz Class.forName com este pacote; "
            "divergir da NoClassDefFoundError so no Pixel",
        )

    def test_nome_da_classe_bate_com_a_constante_do_rust(self):
        texto = KOTLIN.read_text(encoding="utf-8")
        achado = re.search(r"^\s*class\s+(\w+)\s*\(", texto, re.MULTILINE)
        self.assertIsNotNone(achado, "MidiaPlugin.kt nao declara classe alguma")
        self.assertEqual(achado.group(1), constante_do_rust("CLASSE_KOTLIN"))

    def test_classe_tem_a_anotacao_que_o_plugin_manager_procura(self):
        texto = KOTLIN.read_text(encoding="utf-8")
        self.assertIn(
            "@TauriPlugin",
            texto,
            "sem @TauriPlugin a regra de keep do tauri-android nao pega a classe "
            "e o R8 a apaga no build de release",
        )
        self.assertRegex(
            texto,
            r"class\s+\w+\s*\([^)]*\)\s*:\s*Plugin\(",
            "o Tauri instancia a classe com o construtor (Activity) e a passa "
            "como app.tauri.plugin.Plugin",
        )

    def test_comando_chamado_pelo_rust_existe_e_esta_anotado_no_kotlin(self):
        rust = RUST.read_text(encoding="utf-8")
        achado = re.search(r'run_mobile_plugin_async::<\w+>\(\s*"(\w+)"', rust)
        self.assertIsNotNone(achado, "o Rust nao chama comando nenhum do plugin")
        comando = achado.group(1)

        kotlin = KOTLIN.read_text(encoding="utf-8")
        self.assertRegex(
            kotlin,
            r"@Command\s+fun\s+" + re.escape(comando) + r"\s*\(",
            f"o PluginHandle do Android indexa os metodos @Command pelo nome; "
            f"o Rust pede '{comando}' e o Kotlin nao oferece",
        )

    def test_chave_do_payload_e_a_mesma_dos_dois_lados(self):
        rust = RUST.read_text(encoding="utf-8")
        self.assertRegex(
            rust,
            r"struct PedidoScan\s*\{\s*caminhos:",
            "o campo serializado vira a chave do JSON que o Kotlin le",
        )
        self.assertIn(
            'optJSONArray("caminhos")',
            KOTLIN.read_text(encoding="utf-8"),
            "o Kotlin le outra chave: a lista chegaria vazia e nada seria indexado",
        )

    def test_regra_de_proguard_nomeia_a_classe_de_verdade(self):
        pacote = injetor.pacote_do_kotlin(KOTLIN.read_text(encoding="utf-8"))
        classe = constante_do_rust("CLASSE_KOTLIN")
        self.assertIn(
            f"{pacote}.{classe}",
            PROGUARD.read_text(encoding="utf-8"),
            "a regra de keep aponta para outro nome: no release o R8 apaga a classe",
        )


class TestFallbackSeguroSemPanico(unittest.TestCase):
    """Garante que a ausencia do plugin ou execucao no desktop degrada graciosamente."""

    def test_setup_devolve_ok_mesmo_quando_registro_falha(self):
        texto = RUST.read_text(encoding="utf-8")
        # Se o registro falhar, setup nao pode retornar Err, pois isso abortaria a subida do app.
        achado = re.search(
            r"Err\(e\)\s*=>\s*\{.*?eprintln!.*?\}.*?Ok\(\(\)",
            texto,
            re.DOTALL,
        )
        self.assertIsNotNone(
            achado,
            "falha de registro do plugin deve apenas registrar log e retornar Ok(()).",
        )

    def test_avisar_scanner_verifica_ponte_antes_de_chamar(self):
        texto = RUST.read_text(encoding="utf-8")
        self.assertIn(
            "let Some(ponte) = PONTE.get() else {",
            texto,
            "avisar_scanner no Android precisa validar se PONTE foi inicializada.",
        )

    def test_codigo_rust_nao_contem_unwrap_ou_expect(self):
        linhas = RUST.read_text(encoding="utf-8").splitlines()
        for i, linha in enumerate(linhas, 1):
            limpa = linha.strip()
            if limpa.startswith("//") or limpa.startswith("/*") or limpa.startswith("*"):
                continue
            self.assertNotIn(
                ".unwrap()",
                limpa,
                f"encontrado unwrap() na linha {i} de {RUST.name}, risco de panico/SIGABRT.",
            )
            self.assertNotIn(
                ".expect(",
                limpa,
                f"encontrado expect() na linha {i} de {RUST.name}, risco de panico/SIGABRT.",
            )

    def test_desktop_possui_implementacao_inerte(self):
        texto = RUST.read_text(encoding="utf-8")
        self.assertIn(
            '#[cfg(not(target_os = "android"))]\npub fn avisar_scanner(_caminhos: &[String]) {}',
            texto,
            "no desktop avisar_scanner deve ser um no-op sem efeitos colaterais.",
        )


if __name__ == "__main__":
    unittest.main()
