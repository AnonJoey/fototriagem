# fototriagem

> **Triagem ultra-minimalista de fotos e vídeos por teclado, otimizada para o Google Takeout.**

---

## ⚡ Por que o fototriagem?

Após a descontinuação dos escopos de leitura da Google Photos Library API em 2025, o **Google Takeout** tornou-se a única forma viável de exportar seu acervo na íntegra. No entanto, o Takeout entrega:
- Metadados fragmentados em arquivos sidecar `.json` com truncagens e caracteres especiais.
- Dezenas de cópias duplicadas espalhadas entre álbuns e pastas de anos.
- Datas de extração incorretas sobrescrevendo a data de modificação dos arquivos (`mtime`).

O **fototriagem** resolve tudo isso com uma experiência de usuário **zen, sem distrações e 100% controlada por teclado**.

---

## 🖥️ Filosofia de Interface (Ultra-Minimalista)

- **Foco absoluto na imagem**: 95%+ da tela dedicada à mídia, com fundo obsidiana profundo (`#09090b`).
- **Menu em coluna única**: Todas as ações são visíveis em lista vertical direta (estilo menu de videogame). Nada de atalhos misteriosos para decorar.
- **`ESC` sempre volta**: Sobe um nível na navegação. Do topo, encerra a aplicação.
- **Rede de proteção total**:
  - Mídias descartadas vão para `_lixo/` (reversível).
  - `CTRL + Z` devolve os arquivos ao local original exato.
  - Resolução automática de colisões de nome (`foto_2.jpg`, nunca sobrescreve).
  - Log auditável estruturado em [`.triagem_log.jsonl`](.triagem_log.jsonl).
  - Cópias de álbuns são preservadas na origem e catalogadas em [`COPIAS-NAO-APAGADAS.txt`](COPIAS-NAO-APAGADAS.txt).

---

## ⌨️ Controles de Teclado

### Visor de Fotos e Vídeos
| Tecla | Ação |
| :--- | :--- |
| `↑` *(Cima)* | Mover para o lixo (`_lixo/`, reversível) |
| `↓` *(Baixo)* | Abrir menu de pastas de destino |
| `←` / `→` | Navegar entre mídias sem decidir |
| `Espaço` | Play / Pause em vídeos |
| `Ctrl + Z` | Desfazer a última ação |
| `Esc` | Voltar para a seleção de meses |

### Menus e Seleção de Meses
| Tecla | Ação |
| :--- | :--- |
| `↑` / `↓` | Mover cursor |
| `Enter` | Selecionar opção / Confirmar |
| `Esc` | Cancelar / Voltar um nível |

*Durante a digitação do nome de uma nova pasta, o teclado digita normalmente sem disparar atalhos.*

---

## 🧩 Extração Inteligente de Metadados

1. **7 Estratégias de Sidecar JSON**:
   - Casamento exato (`foto.jpg.json`)
   - Truncagem de 51 caracteres do Google
   - Inversão de parênteses (`foto(1).jpg` ↔ `foto.jpg(1).json`)
   - Sufixos de cópias editadas em 10 idiomas (`-edited`, `-modifié`, `-bearbeitet`, etc.)
   - Arquivos enviados sem extensão original
   - Regex de sufixos de edição adicionais
   - Remoção de dígitos duplicados
2. **Normalização Unicode NFC**: Evita quebras com caracteres acentuados no macOS / Linux.
3. **Cadeia de 4 Níveis de Confiança para Datas**:
   $$\text{Takeout JSON} \longrightarrow \text{EXIF (ExifIFD)} \longrightarrow \text{Container Video (ffprobe)} \longrightarrow \text{mtime (com alerta!)}$$

---

## 📦 Instalação

```bash
git clone https://github.com/AnonJoey/fototriagem.git
cd fototriagem

# Instalação em modo editável ou padrão
pip install -e .
```

---

## 🚀 Como Usar

```bash
# Executando via comando direto
fototriagem "/caminho/para/Google Photos" --destino "/caminho/para/Destino_Organizado"

# Ou via módulo Python
python3 -m fototriagem "/caminho/para/Google Photos" --destino "/caminho/para/Destino_Organizado"
```

> **Dica**: Recomenda-se apontar `--destino` para uma pasta separada da origem para evitar misturar as pastas originais do Takeout (`Photos from 2024`, álbuns) com suas pastas organizadas.

---

## 🧪 Testes

O projeto conta com suíte completa de testes unitários e de integração cobrindo 5 perspectivas:

```bash
python3 tests/run_all_passes.py
```

---

## 📄 Licença

Distribuído sob a licença [MIT](LICENSE).
