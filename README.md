# GS PHOTO TRIAGE

> **Organizador e triador ultra-leve de fotos e vídeos por teclado, otimizado para Google Takeout com estética inspirada em *Take Me Back to Eden* (Sleep Token).**

---

## 🌒 Visão Geral

O **GS PHOTO TRIAGE** combina o poder de um backend nativo em **Rust + Tauri 2** (binário final de apenas **7.6 MB**) com uma interface ultra-minimalista, imersiva e 100% controlada por teclado.

O app resolve as maiores dores de exportações do Google Takeout:
- Reconhece e consolida metadados de arquivos sidecar `.json` usando **7 estratégias avançadas** (incluindo truncagem de 51 caracteres, inversão de parênteses e sufixos multilíngues).
- Cascata inteligente de datas: `Takeout JSON` ➔ `EXIF (ExifIFD)` ➔ `Container Vídeo (ffprobe)` ➔ `mtime (com aviso)`.
- Deduplica fotos de álbuns preservando cópias na origem e gerando [`COPIAS-NAO-APAGADAS.txt`](COPIAS-NAO-APAGADAS.txt).
- Rede de proteção com lixeira reversível (`_lixo/`), resolução automática de colisões de nome e log auditável em [`.triagem_log.jsonl`](.triagem_log.jsonl).

---

## 🎨 Design & Estrutura da Interface

Baseado na estética ritualística de *Take Me Back to Eden* e na estrutura de layout de 3 cartões:

```
                          GS PHOTO TRIAGE
       ┌──────────┐    ┌────────────────────┐    ┌──────────┐
       │          │    │                    │    │          │
       │   PREV   │    │    FOTO ATIVA      │    │   NEXT   │
       │ (◄ Left) │    │  (Borda Destaque)  │    │ (► Right)│
       │          │    │                    │    │          │
       └──────────┘    └────────────────────┘    └──────────┘

                                ┌───┐
                                │ ▲ │  (TRASH / LIXO)
                            ┌───┼───┼───┐
             (PREV)         │ ◄ │ ▼ │ ► │  (NEXT)
                            └───┴───┴───┘
                                  │
                              (FOLDERS)

       [Z] UNDUE             [↓] FOLDERS             [ESC] MAIN MENU
```

- **Carrossel Central:** Card ativo proeminente no centro, flanqueado por prévias semitransparentes da foto anterior e próxima.
- **Controlador Tátil de Teclas:** 4 keycaps mecânicos estilizados que respondem e se animam em tempo real conforme as teclas físicas são pressionadas.
- **Paleta de Cores Eden:** Fundo obsidiana profundo (`#0c0c0f`), tipografia alabastro/osso (`#f4f2ea`), acentos em ouro rúnico (`#e2b714`) e carmesim (`#e11d48`).

---

## ⌨️ Controles e Atalhos

### Modo Triagem (Visor)
| Tecla | Ação |
| :--- | :--- |
| `▲` *(Cima)* | Mover para o lixo (`_lixo/`, reversível) |
| `▼` *(Baixo)* | Abrir menu de pastas de destino |
| `◄` *(Esquerda)* | Voltar para a foto anterior |
| `►` *(Direita)* | Avançar para a próxima foto |
| `Espaço` | Play / Pause em vídeos |
| `Ctrl + Z` | Desfazer a última movimentação |
| `Esc` | Voltar para o menu de meses |

### Modo Menu & Seleção de Meses
| Tecla | Ação |
| :--- | :--- |
| `▲` / `▼` | Navegar entre itens |
| `Enter` | Selecionar opção / Confirmar |
| `Esc` | Cancelar / Voltar |

---

## 🚀 Como Executar

### 1. Aplicativo Desktop Tauri (Recomendado - Ultra Leve)

```bash
# Entrar no diretório do projeto
cd /home/joey/Projects/fototriagem

# Executar o binário compilado em modo Release (7.6 MB)
./src-tauri/target/release/fototriagem

# Ou iniciar em modo de desenvolvimento com hot-reload:
pnpm tauri dev
```

### 2. Versão CLI Python (Opcional)

```bash
python3 python/triar.py "/caminho/para/Google Photos" --destino "/caminho/para/Destino"
```

---

## 🛠️ Arquitetura Técnica

- **Backend:** Rust (Tauri 2, Rayon, Kamadak-Exif, Serde, Chrono, SHA-256).
- **Frontend:** Vanilla HTML5 / CSS3 / ES Modules com Vite (bundle de apenas ~35 KB).
- **Consumo de Memória:** ~40 MB RAM (em comparação com >300 MB de aplicações Electron).

---

## 📄 Licença

Distribuído sob a licença [MIT](LICENSE).
