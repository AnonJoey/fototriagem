use chrono::Utc;
use filetime::{set_file_mtime, FileTime};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::library::{Copia, COPIAS, LIXO};

pub const LOG_FILE: &str = ".triagem_log.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcaoRegistro {
    pub quando: String,
    pub acao: String,
    pub de: String,
    pub para: String,
    /// Preenchidos so quando a acao mexeu no sidecar ou na data do arquivo.
    /// Ficam fora do JSON quando nao ha o que dizer, para que o log continue
    /// legivel por quem ja lia o formato antigo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidecar_de: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidecar_para: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_gravada: Option<i64>,
    #[serde(default, skip_serializing_if = "eh_zero")]
    pub copias: usize,
}

fn eh_zero(n: &usize) -> bool {
    *n == 0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PilhaItem {
    pub origem: String,
    pub atual: String,
    /// Caminho completo do sidecar antes e depois do movimento. Guardados
    /// aos pares para que desfazer devolva o .json ao nome exato que tinha,
    /// mesmo tendo sido renomeado para acompanhar a midia.
    pub sidecar_origem: Option<String>,
    pub sidecar_atual: Option<String>,
    /// mtime que o arquivo tinha antes do carimbo, para que Ctrl+Z devolva
    /// tambem a data e nao so o lugar.
    pub mtime_anterior: Option<i64>,
    /// As copias parqueadas nesta mesma decisao, cada uma com de onde veio,
    /// onde esta, e o mtime que tinha. Ficam no mesmo item da pilha porque um
    /// Ctrl+Z desfaz a decisao inteira, nao um arquivo por vez.
    pub copias: Vec<CopiaMovida>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopiaMovida {
    pub origem: String,
    pub atual: String,
    pub sidecar_origem: Option<String>,
    pub sidecar_atual: Option<String>,
    pub mtime_anterior: Option<i64>,
}

/// Tudo que se sabe sobre um arquivo na hora de decidir o destino dele.
/// Junto num so lugar porque mover uma foto do Takeout nunca e mover um
/// arquivo: e mover a foto, o .json dela, as copias de album e as datas.
pub struct Movimento<'a> {
    pub caminho: &'a str,
    /// Data resolvida pela cadeia. None quando ela ja veio do proprio mtime.
    pub timestamp: Option<i64>,
    /// Nome do .json na pasta da midia.
    pub sidecar: Option<&'a str>,
    /// Copias identicas achadas noutros albuns.
    pub copias: &'a [Copia],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesfazerItem {
    pub origem_original: String,
    pub caminho_anterior: String,
    pub caminho_restaurado: String,
    /// Onde o sidecar voltou a ficar, quando havia um para devolver.
    pub sidecar_restaurado: Option<String>,
}

pub struct GerenciadorAcoes {
    pub destino: PathBuf,
    pub pilha: Vec<PilhaItem>,
}

fn mover_arquivo(de: &Path, para: &Path) -> Result<(), String> {
    if de == para {
        return Ok(());
    }
    if fs::rename(de, para).is_ok() {
        return Ok(());
    }
    match fs::copy(de, para) {
        Ok(_) => {
            if let Err(e) = fs::remove_file(de) {
                let _ = fs::remove_file(para);
                return Err(format!("Falha ao remover arquivo de origem após cópia: {}", e));
            }
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(para);
            Err(format!("Falha ao mover {} para {}: {}", de.display(), para.display(), e))
        }
    }
}

fn ler_mtime(caminho: &Path) -> Option<i64> {
    fs::metadata(caminho)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
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

    /// Escolhe o nome livre no destino **e o toma no mesmo ato**, criando o
    /// arquivo com `create_new`.
    ///
    /// `sem_colisao` sozinha so olha: entre o olhar e o `rename` cabe outro
    /// processo criando o mesmo nome, e no Linux o `rename` passa por cima
    /// dele sem avisar. Reservando, quem chega depois recebe AlreadyExists e
    /// vai para o proximo nome, e nada de ninguem e sobrescrito em silencio.
    ///
    /// O arquivo reservado fica vazio e e substituido pelo `rename` logo em
    /// seguida; se o movimento falhar, quem chamou apaga a reserva.
    pub fn reservar_nome(&self, pasta: &Path, nome: &str, ignorar: Option<&Path>) -> Result<PathBuf, String> {
        let _ = fs::create_dir_all(pasta);
        let path = Path::new(nome);
        let base = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let ext_com_ponto = if ext.is_empty() { String::new() } else { format!(".{}", ext) };

        let mut n = 0usize;
        loop {
            let cand = if n == 0 {
                pasta.join(nome)
            } else {
                pasta.join(format!("{}_{}{}", base, n + 1, ext_com_ponto))
            };

            // Mover um arquivo para o lugar onde ele ja esta nao e mover:
            // reservar aqui truncaria justamente o arquivo que se quer manter.
            if let Some(ign) = ignorar {
                if cand == *ign {
                    return Ok(cand);
                }
            }

            match OpenOptions::new().write(true).create_new(true).open(&cand) {
                Ok(_) => return Ok(cand),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
                Err(e) => return Err(format!("Falha ao reservar {}: {}", cand.display(), e)),
            }

            if n > 10_000 {
                return Err(format!("nomes demais em colisao para {}", nome));
            }
        }
    }

    pub fn registrar(&self, reg: AcaoRegistro) {
        let log_path = self.destino.join(LOG_FILE);
        if let Ok(json_line) = serde_json::to_string(&reg) {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
                let _ = writeln!(file, "{}", json_line);
            }
        }
    }

    fn registro(acao: &str, de: &Path, para: &Path) -> AcaoRegistro {
        AcaoRegistro {
            quando: Utc::now().to_rfc3339(),
            acao: acao.to_string(),
            de: de.to_string_lossy().to_string(),
            para: para.to_string_lossy().to_string(),
            sidecar_de: None,
            sidecar_para: None,
            data_gravada: None,
            copias: 0,
        }
    }

    /// Leva o sidecar da midia para junto dela, com o nome dela mais `.json`,
    /// de modo que no destino todo arquivo tenha o seu par obvio ao lado.
    /// Devolve (de onde saiu, para onde foi) quando houve o que mover.
    fn levar_sidecar(
        &self,
        midia_origem: &Path,
        midia_alvo: &Path,
        pasta_alvo: &Path,
        sidecar: Option<&str>,
    ) -> (Option<String>, Option<String>) {
        let nome = match sidecar {
            Some(n) => n,
            None => return (None, None),
        };
        let sc_origem = midia_origem.parent().unwrap_or(Path::new(".")).join(nome);
        if !sc_origem.is_file() {
            return (None, None);
        }

        let alvo_nome = midia_alvo.file_name().and_then(|n| n.to_str()).unwrap_or("midia");
        let sc_alvo = match self.reservar_nome(pasta_alvo, &format!("{}.json", alvo_nome), None) {
            Ok(p) => p,
            Err(_) => return (None, None),
        };

        // Se o .json falhar, a midia ja esta movida e nao se desfaz por causa
        // dele: o erro vira aviso no log e a triagem segue.
        match mover_arquivo(&sc_origem, &sc_alvo) {
            Ok(()) => (
                Some(sc_origem.to_string_lossy().to_string()),
                Some(sc_alvo.to_string_lossy().to_string()),
            ),
            Err(e) => {
                let mut aviso = Self::registro("sidecar-falhou", &sc_origem, &sc_alvo);
                aviso.sidecar_de = Some(e);
                self.registrar(aviso);
                (None, None)
            }
        }
    }

    fn carimbar(alvo: &Path, timestamp: Option<i64>) -> Option<i64> {
        match timestamp {
            Some(ts) if ts > 0 => match set_file_mtime(alvo, FileTime::from_unix_time(ts, 0)) {
                Ok(()) => Some(ts),
                Err(_) => None,
            },
            _ => None,
        }
    }

    /// Onde as copias de uma decisao vao parar: `_copias/<pasta escolhida>/`
    /// dentro do destino. A pasta espelha a decisao para que se saiba, depois,
    /// de que grupo cada copia parqueada era.
    fn pasta_de_copias(&self, pasta_alvo: &Path) -> PathBuf {
        let nome = pasta_alvo
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("sem-pasta");
        self.destino.join(COPIAS).join(nome)
    }

    /// Move a midia e, junto com ela, tudo que e dela: o sidecar do Takeout,
    /// a data real e as copias identicas que estavam noutros albuns.
    ///
    /// A data resolvida pela cadeia e gravada no mtime do arquivo movido,
    /// senao a pasta organizada acaba ordenada pela data em que o Google
    /// gerou o export em vez da data em que a foto foi tirada.
    ///
    /// Tudo isso entra num unico item da pilha: um Ctrl+Z devolve a decisao
    /// inteira ao que era antes, e nao um pedaco dela.
    pub fn mover(&mut self, mov: Movimento<'_>, pasta_alvo: &Path) -> Result<String, String> {
        let origem = PathBuf::from(mov.caminho);
        if !origem.exists() {
            return Err("Arquivo de origem não existe".to_string());
        }

        let _ = fs::create_dir_all(pasta_alvo);
        let file_name = origem.file_name().and_then(|n| n.to_str()).unwrap_or("midia");
        let alvo = self.reservar_nome(pasta_alvo, file_name, Some(&origem))?;

        let mtime_anterior = ler_mtime(&origem);
        mover_arquivo(&origem, &alvo)?;

        let (sidecar_origem, sidecar_atual) =
            self.levar_sidecar(&origem, &alvo, pasta_alvo, mov.sidecar);
        let data_gravada = Self::carimbar(&alvo, mov.timestamp);

        let copias = self.parquear_copias(mov.copias, pasta_alvo, mov.timestamp);

        let alvo_str = alvo.to_string_lossy().to_string();
        self.pilha.push(PilhaItem {
            origem: mov.caminho.to_string(),
            atual: alvo_str.clone(),
            sidecar_origem: sidecar_origem.clone(),
            sidecar_atual: sidecar_atual.clone(),
            mtime_anterior,
            copias: copias.clone(),
        });

        let mut reg = Self::registro("mover", &origem, &alvo);
        reg.sidecar_de = sidecar_origem;
        reg.sidecar_para = sidecar_atual;
        reg.data_gravada = data_gravada;
        reg.copias = copias.len();
        self.registrar(reg);

        Ok(alvo_str)
    }

    /// Manda cada copia identica para a quarentena, com o sidecar dela e a
    /// mesma data. Uma copia que falhar e simplesmente deixada onde estava:
    /// a decisao sobre a foto principal ja valeu e nao se perde por isso.
    fn parquear_copias(
        &mut self,
        copias: &[Copia],
        pasta_alvo: &Path,
        timestamp: Option<i64>,
    ) -> Vec<CopiaMovida> {
        if copias.is_empty() {
            return Vec::new();
        }
        let quarentena = self.pasta_de_copias(pasta_alvo);
        let _ = fs::create_dir_all(&quarentena);

        let mut movidas = Vec::new();
        for copia in copias {
            let de = PathBuf::from(&copia.caminho);
            if !de.is_file() {
                continue;
            }
            let nome = de.file_name().and_then(|n| n.to_str()).unwrap_or("copia");
            let para = match self.reservar_nome(&quarentena, nome, None) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let mtime_anterior = ler_mtime(&de);

            if let Err(e) = mover_arquivo(&de, &para) {
                let mut aviso = Self::registro("copia-falhou", &de, &para);
                aviso.sidecar_de = Some(e);
                self.registrar(aviso);
                continue;
            }

            let (sc_de, sc_para) =
                self.levar_sidecar(&de, &para, &quarentena, copia.sidecar.as_deref());
            Self::carimbar(&para, timestamp);

            let mut reg = Self::registro("parquear-copia", &de, &para);
            reg.sidecar_de = sc_de.clone();
            reg.sidecar_para = sc_para.clone();
            self.registrar(reg);

            movidas.push(CopiaMovida {
                origem: copia.caminho.clone(),
                atual: para.to_string_lossy().to_string(),
                sidecar_origem: sc_de,
                sidecar_atual: sc_para,
                mtime_anterior,
            });
        }
        movidas
    }

    pub fn para_o_lixo(&mut self, mov: Movimento<'_>) -> Result<String, String> {
        let lixo_pasta = self.destino.join(LIXO);
        self.mover(mov, &lixo_pasta)
    }

    pub fn desfazer(&mut self) -> Result<Option<DesfazerItem>, String> {
        let item = match self.pilha.pop() {
            Some(i) => i,
            None => return Ok(None),
        };

        let atual = PathBuf::from(&item.atual);
        let origem = PathBuf::from(&item.origem);

        if !atual.exists() {
            self.pilha.push(item);
            return Err("Arquivo movido não existe mais no destino".to_string());
        }

        if let Some(parent) = origem.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let orig_name = origem.file_name().and_then(|n| n.to_str()).unwrap_or("midia");
        let alvo_origem = if origem.exists() && origem != atual {
            let parent = origem.parent().unwrap_or(Path::new("."));
            self.reservar_nome(parent, orig_name, Some(&atual))?
        } else {
            origem
        };

        if let Err(e) = mover_arquivo(&atual, &alvo_origem) {
            self.pilha.push(item);
            return Err(e);
        }

        // Desfazer devolve tambem a data que o arquivo tinha antes do carimbo.
        if let Some(ts) = item.mtime_anterior {
            let _ = set_file_mtime(&alvo_origem, FileTime::from_unix_time(ts, 0));
        }

        let mut sidecar_de = None;
        let mut sidecar_para = None;
        if let (Some(sc_origem), Some(sc_atual)) = (&item.sidecar_origem, &item.sidecar_atual) {
            let de = PathBuf::from(sc_atual);
            let para = PathBuf::from(sc_origem);
            if de.is_file() {
                if let Some(parent) = para.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let sc_name = para.file_name().and_then(|n| n.to_str()).unwrap_or("sidecar.json");
                let alvo_sc = if para.exists() && para != de {
                    let parent = para.parent().unwrap_or(Path::new("."));
                    self.sem_colisao(parent, sc_name, Some(&de))
                } else {
                    para.clone()
                };
                if mover_arquivo(&de, &alvo_sc).is_ok() {
                    sidecar_de = Some(sc_atual.clone());
                    sidecar_para = Some(alvo_sc.to_string_lossy().to_string());
                }
            }
        }

        // As copias voltam para os albuns de onde sairam, com o nome e a data
        // que tinham. Desfazer nao deixa metade da decisao em pe.
        let mut copias_devolvidas = 0;
        for copia in &item.copias {
            let de = PathBuf::from(&copia.atual);
            let para = PathBuf::from(&copia.origem);
            if !de.is_file() {
                continue;
            }
            if let Some(parent) = para.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let copia_name = para.file_name().and_then(|n| n.to_str()).unwrap_or("copia");
            let alvo_copia = if para.exists() && para != de {
                let parent = para.parent().unwrap_or(Path::new("."));
                self.sem_colisao(parent, copia_name, Some(&de))
            } else {
                para.clone()
            };
            if mover_arquivo(&de, &alvo_copia).is_err() {
                continue;
            }
            if let Some(ts) = copia.mtime_anterior {
                let _ = set_file_mtime(&alvo_copia, FileTime::from_unix_time(ts, 0));
            }
            if let (Some(sc_de), Some(sc_para)) = (&copia.sidecar_atual, &copia.sidecar_origem) {
                let sc_de = PathBuf::from(sc_de);
                let sc_para = PathBuf::from(sc_para);
                if sc_de.is_file() {
                    let sc_c_name = sc_para.file_name().and_then(|n| n.to_str()).unwrap_or("sidecar.json");
                    let alvo_sc_c = if sc_para.exists() && sc_para != sc_de {
                        let parent = sc_para.parent().unwrap_or(Path::new("."));
                        self.sem_colisao(parent, sc_c_name, Some(&sc_de))
                    } else {
                        sc_para.clone()
                    };
                    let _ = mover_arquivo(&sc_de, &alvo_sc_c);
                }
            }
            copias_devolvidas += 1;
        }
        Self::limpar_vazias(&self.destino.join(COPIAS));

        let mut reg = Self::registro("desfazer", &atual, &alvo_origem);
        reg.sidecar_de = sidecar_de;
        reg.sidecar_para = sidecar_para.clone();
        reg.data_gravada = item.mtime_anterior;
        reg.copias = copias_devolvidas;
        self.registrar(reg);

        Ok(Some(DesfazerItem {
            origem_original: item.origem,
            caminho_anterior: item.atual,
            caminho_restaurado: alvo_origem.to_string_lossy().to_string(),
            sidecar_restaurado: sidecar_para,
        }))
    }

    /// Depois de um desfazer, uma pasta de quarentena que ficou vazia so
    /// atrapalha quem for conferir o que sobrou. Some com ela, e com a
    /// `_copias/` inteira se ela tambem esvaziou. Nunca remove nada com
    /// conteudo dentro.
    fn limpar_vazias(raiz: &Path) {
        if !raiz.is_dir() {
            return;
        }
        if let Ok(entries) = fs::read_dir(raiz) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let _ = fs::remove_dir(entry.path());
                }
            }
        }
        let _ = fs::remove_dir(raiz);
    }
}
