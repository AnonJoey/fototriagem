use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::library::LIXO;

pub const LOG_FILE: &str = ".triagem_log.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcaoRegistro {
    pub quando: String,
    pub acao: String,
    pub de: String,
    pub para: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PilhaItem {
    pub origem: String,
    pub atual: String,
}

pub struct GerenciadorAcoes {
    pub destino: PathBuf,
    pub pilha: Vec<PilhaItem>,
}

impl GerenciadorAcoes {
    pub fn new(destino: PathBuf) -> Self {
        Self {
            destino,
            pilha: Vec::new(),
        }
    }

    pub fn sem_colisao(&self, pasta: &Path, nome: &str, ignorar: Option<&Path>) -> PathBuf {
        let alvo = pasta.join(nome);
        if !alvo.exists() {
            return alvo;
        }
        if let Some(ign) = ignorar {
            if alvo == ign {
                return alvo;
            }
        }

        let path = Path::new(nome);
        let base = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let ext_with_dot = if ext.is_empty() { String::new() } else { format!(".{}", ext) };

        let mut n = 2;
        loop {
            let cand = pasta.join(format!("{}_{}{}", base, n, ext_with_dot));
            if !cand.exists() {
                return cand;
            }
            if let Some(ign) = ignorar {
                if cand == ign {
                    return cand;
                }
            }
            n += 1;
        }
    }

    pub fn registrar(&self, acao: &str, de: &Path, para: &Path) {
        let log_path = self.destino.join(LOG_FILE);
        let reg = AcaoRegistro {
            quando: Utc::now().to_rfc3339(),
            acao: acao.to_string(),
            de: de.to_string_lossy().to_string(),
            para: para.to_string_lossy().to_string(),
        };

        if let Ok(json_line) = serde_json::to_string(&reg) {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
                let _ = writeln!(file, "{}", json_line);
            }
        }
    }

    pub fn mover(&mut self, origem_str: &str, pasta_alvo: &Path) -> Result<String, String> {
        let origem = PathBuf::from(origem_str);
        if !origem.exists() {
            return Err("Arquivo de origem não existe".to_string());
        }

        let _ = fs::create_dir_all(pasta_alvo);
        let file_name = origem.file_name().and_then(|n| n.to_str()).unwrap_or("midia");
        let alvo = self.sem_colisao(pasta_alvo, file_name, Some(&origem));

        if origem != alvo {
            fs::rename(&origem, &alvo).or_else(|_| fs::copy(&origem, &alvo).map(|_| { let _ = fs::remove_file(&origem); }))
                .map_err(|e| format!("Falha ao mover arquivo: {}", e))?;
        }

        let alvo_str = alvo.to_string_lossy().to_string();
        self.pilha.push(PilhaItem {
            origem: origem_str.to_string(),
            atual: alvo_str.clone(),
        });
        self.registrar("mover", &origem, &alvo);

        Ok(alvo_str)
    }

    pub fn para_o_lixo(&mut self, origem_str: &str) -> Result<String, String> {
        let lixo_pasta = self.destino.join(LIXO);
        self.mover(origem_str, &lixo_pasta)
    }

    pub fn desfazer(&mut self) -> Result<Option<PilhaItem>, String> {
        let item = match self.pilha.pop() {
            Some(i) => i,
            None => return Ok(None),
        };

        let atual = PathBuf::from(&item.atual);
        let origem = PathBuf::from(&item.origem);

        if !atual.exists() {
            return Err("Arquivo movido não existe mais no destino".to_string());
        }

        if let Some(parent) = origem.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let orig_name = origem.file_name().and_then(|n| n.to_str()).unwrap_or("midia");
        let alvo_origem = if origem.exists() && origem != atual {
            let parent = origem.parent().unwrap_or(Path::new("."));
            self.sem_colisao(parent, orig_name, Some(&atual))
        } else {
            origem
        };

        if atual != alvo_origem {
            fs::rename(&atual, &alvo_origem)
                .or_else(|_| fs::copy(&atual, &alvo_origem).map(|_| { let _ = fs::remove_file(&atual); }))
                .map_err(|e| format!("Falha ao restaurar arquivo: {}", e))?;
        }

        self.registrar("desfazer", &atual, &alvo_origem);

        Ok(Some(PilhaItem {
            origem: item.origem,
            atual: alvo_origem.to_string_lossy().to_string(),
        }))
    }
}
