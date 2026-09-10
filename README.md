# GS PhotoTriage

> **Organizador e triador ultra-leve de fotos e vídeos por teclado, otimizado para Google Takeout.**

---

## ⚡ Visão Geral

O **GS PhotoTriage** combina o poder de um backend nativo em **Rust + Tauri 2** (binário final de apenas **7.6 MB**) com uma interface ultra-minimalista, fluida e 100% controlada por teclado.

O app resolve as maiores dores de exportações do Google Takeout:
- Reconhece e consolida metadados de arquivos sidecar `.json` usando **7 estratégias avançadas** (incluindo truncagem de 51 caracteres, inversão de parênteses e sufixos multilíngues).
- Cascata inteligente de datas: `Takeout JSON` ➔ `EXIF (ExifIFD)` ➔ `Container Vídeo (ffprobe)` ➔ `mtime (com aviso)`.
- **Grava a data verdadeira no arquivo.** Ao mover, o `mtime` recebe a data resolvida pela cadeia, senão a pasta organizada acaba ordenada pela data em que o Google gerou o export.
- **Leva o sidecar junto.** O `.json` viaja com a mídia e chega com o nome dela (`IMG_0001.jpg.json`), de modo que o destino continue datável sozinho, sem depender da árvore do Takeout.
- Deduplica fotos de álbuns: a cópia idêntica segue a decisão para `_copias/<pasta escolhida>/` e o inventário fica em `COPIAS-NAO-APAGADAS.txt`.
- Rede de proteção com lixeira reversível (`_lixo/`), resolução automática de colisões de nome e log auditável em `.triagem_log.jsonl`.

### O que uma decisão move

Apertar uma tecla não move um arquivo, move tudo que é dele. Um `Ctrl+Z` desfaz a decisão inteira, incluindo a data carimbada, e nunca metade dela.

| O que | Vai para |
| :--- | :--- |
| A foto ou vídeo | a pasta escolhida, ou `_lixo/` |
| O `.json` do Takeout | ao lado dela, renomeado para acompanhar |
| As cópias idênticas de outros álbuns | `_copias/<pasta escolhida>/`, cada uma com o próprio `.json` |
| A data de quando a foto foi tirada | o `mtime` do arquivo movido |

Nada é apagado em momento algum. No fim da triagem, `_copias/` e `_lixo/` são duas pastas para conferir e apagar de uma vez, se você quiser.

---

## 🖼️ Estrutura da Interface

```
                           GS PhotoTriage
        ┌──────────┐    ┌────────────────────┐    ┌──────────┐
        │          │    │                    │    │          │
        │ ANTERIOR │    │    FOTO ATIVA      │    │ PRÓXIMO  │
        │ (◄ Left) │    │  (Borda Destaque)  │    │ (► Right)│
        │          │    │                    │    │          │
        └──────────┘    └────────────────────┘    └──────────┘

                                ┌───┐
                                │ ▲ │  (LIXO)
                            ┌───┼───┼───┐
             (ANTERIOR)     │ ◄ │ ▼ │ ► │  (PRÓXIMO)
                            └───┴───┴───┘
                                  │
                               (PASTAS)

        [Ctrl+Z] DESFAZER        [↓] PASTAS        [ESC] MENU PRINCIPAL
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

### 2. Conferir um Takeout antes de confiar a galeria a ele

Varre e relata sem abrir janela e sem mover nada: quantos arquivos têm sidecar, de onde
cada data veio, e quais ficariam sem data confiável.

```bash
cd src-tauri && cargo run --example varrer -- "/caminho/para/Google Photos"
```

### 3. Versão CLI Python (legado)

Anterior à reescrita em Rust e mantida só como referência. Não grava a data no arquivo,
não move o sidecar e não parqueia as cópias.

```bash
python3 python/triar.py "/caminho/para/Google Photos" --destino "/caminho/para/Destino"
```

---

## 🛠️ Arquitetura Técnica

- **Backend:** Rust (Tauri 2, Rayon, Kamadak-Exif, Filetime, Serde, Chrono, SHA-256).
- **Frontend:** Vanilla HTML5 / CSS3 / ES Modules com Vite (bundle de apenas ~35 KB).
- **Consumo de Memória:** ~40 MB RAM (em comparação com >300 MB de aplicações Electron).

---

## 📄 Licença

Distribuído sob a licença [MIT](LICENSE).
