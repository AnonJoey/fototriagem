use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;

use crate::dates::{eh_suportado, eh_video, LeitorDeDatas};
use crate::takeout::{eh_pasta_de_ano, PASTAS_ESPECIAIS};

pub const LIXO: &str = "_lixo";
pub const CACHE_FILE: &str = ".triagem_cache.json";
pub const RELATORIO_COPIAS: &str = "COPIAS-NAO-APAGADAS.txt";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiaItem {
    pub caminho: String,
    pub nome: String,
    pub timestamp: i64,
    pub data_formatada: String,
    pub ano: i32,
    pub mes: u32,
    pub origem_data: String,
    pub tamanho_bytes: u64,
    pub copias: Vec<String>,
    pub triada: bool,
    pub eh_video: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheItem {
    pub sig: String,
    pub ts: i64,
    pub src: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MesResumo {
    pub ano: i32,
    pub mes: u32,
    pub nome_mes: String,
    pub total: usize,
    pub restantes: usize,
    pub sem_data_real: usize,
    pub concluido: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub itens: Vec<MidiaItem>,
    pub total_arquivos: usize,
    pub duplicatas_removidas: usize,
    pub especiais_ignoradas: usize,
    pub sem_data_real: usize,
    pub meses: Vec<MesResumo>,
    pub avisos: Vec<String>,
}

pub fn calcular_hash_sha256(caminho: &Path) -> Option<String> {
    let mut file = File::open(caminho).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

pub fn formatar_tamanho(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub const MESES_PT: &[&str] = &[
    "", "Janeiro", "Fevereiro", "Março", "Abril", "Maio", "Junho",
    "Julho", "Agosto", "Setembro", "Outubro", "Novembro", "Dezembro",
];

pub fn carregar_biblioteca(
    origem: &Path,
    destino: &Path,
    incluir_especiais: bool,
) -> ScanResult {
    let mut especiais_ignoradas = 0;
    let mut caminhos_candidatos = Vec::new();
    let destino_lixo = destino.join(LIXO);

    for entry in WalkDir::new(origem).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_file() || !eh_suportado(p) {
            continue;
        }
        if p.starts_with(&destino_lixo) {
            continue;
        }

        if !incluir_especiais {
            if let Ok(rel) = p.strip_prefix(origem) {
                let mut especial = false;
                for comp in rel.parent().map(|p| p.components()).into_iter().flatten() {
                    let s = comp.as_os_str().to_str().unwrap_or("").trim().to_lowercase();
                    if PASTAS_ESPECIAIS.contains(&s.as_str()) {
                        especial = true;
                        break;
                    }
                }
                if especial {
                    especiais_ignoradas += 1;
                    continue;
                }
            }
        }
        caminhos_candidatos.push(p.to_path_buf());
    }

    let cache_path = destino.join(CACHE_FILE);
    let mut cache_map: HashMap<String, CacheItem> = if cache_path.exists() {
        fs::read_to_string(&cache_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        HashMap::new()
    };

    let mut leitor = LeitorDeDatas::new();
    let mut itens_brutos = Vec::new();

    for p in &caminhos_candidatos {
        let meta = match fs::metadata(p) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let size = meta.len();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let sig = format!("{}:{}", size, mtime);
        let path_str = p.to_string_lossy().to_string();

        let (ts, origem_data) = if let Some(cached) = cache_map.get(&path_str) {
            if cached.sig == sig {
                (cached.ts, cached.src.clone())
            } else {
                let (t, s) = leitor.ler(p);
                cache_map.insert(path_str.clone(), CacheItem { sig: sig.clone(), ts: t, src: s.clone() });
                (t, s)
            }
        } else {
            let (t, s) = leitor.ler(p);
            cache_map.insert(path_str.clone(), CacheItem { sig: sig.clone(), ts: t, src: s.clone() });
            (t, s)
        };

        let dt = DateTime::from_timestamp(ts, 0).unwrap_or(DateTime::<Utc>::UNIX_EPOCH);
        let data_formatada = dt.format("%d/%m/%Y %H:%M").to_string();
        let nome = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let is_vid = eh_video(p);

        itens_brutos.push(MidiaItem {
            caminho: path_str,
            nome,
            timestamp: ts,
            data_formatada,
            ano: dt.year(),
            mes: dt.month(),
            origem_data,
            tamanho_bytes: size,
            copias: Vec::new(),
            triada: false,
            eh_video: is_vid,
        });
    }

    let (itens_dedup, duplicatas_removidas) = deduplicar(itens_brutos);

    let _ = fs::create_dir_all(destino);
    if let Ok(json_str) = serde_json::to_string_pretty(&cache_map) {
        let _ = fs::write(&cache_path, json_str);
    }

    escrever_relatorio_copias(origem, destino, &itens_dedup);

    let mut itens = itens_dedup;
    itens.sort_by_key(|i| i.timestamp);

    let sem_data_real = itens.iter().filter(|i| i.origem_data == "mtime").count();

    let mut meses_map: HashMap<(i32, u32), Vec<&MidiaItem>> = HashMap::new();
    for item in &itens {
        meses_map.entry((item.ano, item.mes)).or_default().push(item);
    }

    let mut chaves: Vec<(i32, u32)> = meses_map.keys().cloned().collect();
    chaves.sort();

    let mut meses = Vec::new();
    for (ano, mes) in chaves {
        let list = &meses_map[&(ano, mes)];
        let total = list.len();
        let restantes = list.iter().filter(|i| !i.triada).count();
        let duvidosos = list.iter().filter(|i| i.origem_data == "mtime").count();
        let nome_mes = if mes >= 1 && (mes as usize) < MESES_PT.len() {
            MESES_PT[mes as usize].to_string()
        } else {
            mes.to_string()
        };

        meses.push(MesResumo {
            ano,
            mes,
            nome_mes,
            total,
            restantes,
            sem_data_real: duvidosos,
            concluido: restantes == 0,
        });
    }

    let mut avisos = Vec::new();
    if duplicatas_removidas > 0 {
        let copias_com = itens.iter().filter(|i| !i.copias.is_empty());
        let total_bytes: u64 = copias_com.map(|i| i.tamanho_bytes * (i.copias.len() as u64)).sum();
        avisos.push(format!(
            "Aviso: {} cópias idênticas preservadas na origem ({}). Detalhes em {}",
            duplicatas_removidas,
            formatar_tamanho(total_bytes),
            RELATORIO_COPIAS
        ));
    }
    if especiais_ignoradas > 0 {
        avisos.push(format!("{} arquivos ignorados em pastas especiais (Trash/Archive)", especiais_ignoradas));
    }
    if sem_data_real > 0 {
        avisos.push(format!("{} arquivos sem data real (usando timestamp mtime)", sem_data_real));
    }

    ScanResult {
        itens,
        total_arquivos: caminhos_candidatos.len(),
        duplicatas_removidas,
        especiais_ignoradas,
        sem_data_real,
        meses,
        avisos,
    }
}

fn deduplicar(itens: Vec<MidiaItem>) -> (Vec<MidiaItem>, usize) {
    let mut por_tamanho: HashMap<u64, Vec<MidiaItem>> = HashMap::new();
    for item in itens {
        por_tamanho.entry(item.tamanho_bytes).or_default().push(item);
    }

    let mut saida = Vec::new();
    let mut duplicatas_removidas = 0;

    for (_tamanho, grupo) in por_tamanho {
        if grupo.len() == 1 {
            saida.push(grupo.into_iter().next().unwrap());
            continue;
        }

        let mut por_hash: HashMap<String, Vec<MidiaItem>> = HashMap::new();
        for item in grupo {
            let path = Path::new(&item.caminho);
            if let Some(hash) = calcular_hash_sha256(path) {
                por_hash.entry(hash).or_default().push(item);
            } else {
                saida.push(item);
            }
        }

        for (_hash, mut iguais) in por_hash {
            if iguais.len() == 1 {
                saida.push(iguais.remove(0));
                continue;
            }

            let get_ordem = |origem: &str| -> i32 {
                match origem {
                    "takeout" => 0,
                    "takeout~" => 1,
                    "exif" => 2,
                    "video" => 3,
                    _ => 9,
                }
            };

            iguais.sort_by(|a, b| {
                let p_a = Path::new(&a.caminho);
                let p_b = Path::new(&b.caminho);
                let is_year_a = p_a.parent().map(eh_pasta_de_ano).unwrap_or(false);
                let is_year_b = p_b.parent().map(eh_pasta_de_ano).unwrap_or(false);

                get_ordem(&a.origem_data)
                    .cmp(&get_ordem(&b.origem_data))
                    .then_with(|| (!is_year_a).cmp(&(!is_year_b)))
                    .then_with(|| a.nome.len().cmp(&b.nome.len()))
                    .then_with(|| a.caminho.cmp(&b.caminho))
            });

            let mut vencedora = iguais.remove(0);
            vencedora.copias = iguais.into_iter().map(|i| i.caminho).collect();
            duplicatas_removidas += vencedora.copias.len();
            saida.push(vencedora);
        }
    }

    (saida, duplicatas_removidas)
}

fn escrever_relatorio_copias(origem: &Path, destino: &Path, itens: &[MidiaItem]) {
    let com_copias: Vec<&MidiaItem> = itens.iter().filter(|i| !i.copias.is_empty()).collect();
    let rel_path = destino.join(RELATORIO_COPIAS);

    if com_copias.is_empty() {
        let _ = fs::remove_file(rel_path);
        return;
    }

    let mut lines = Vec::new();
    lines.push("COPIAS IDENTICAS ENCONTRADAS NA ORIGEM".to_string());
    lines.push(format!("gerado em {}", Utc::now().format("%d/%m/%Y %H:%M")));
    lines.push(format!("origem: {}", origem.display()));
    lines.push("".to_string());
    lines.push("Nada foi apagado. A triagem mostra apenas a primeira de cada grupo;".to_string());
    lines.push("as demais continuam exatamente onde estao, listadas abaixo.".to_string());
    lines.push("".to_string());

    let total_copias: usize = com_copias.iter().map(|i| i.copias.len()).sum();
    let total_bytes: u64 = com_copias.iter().map(|i| i.tamanho_bytes * (i.copias.len() as u64)).sum();
    lines.push(format!(
        "{} arquivos tem copia, somando {} copias ({} ocupados a mais)",
        com_copias.len(),
        total_copias,
        formatar_tamanho(total_bytes)
    ));
    lines.push("".to_string());

    for item in com_copias {
        lines.push(format!("{}  {}", item.data_formatada, item.nome));
        lines.push(format!("    mostrada : {}", item.caminho));
        for c in &item.copias {
            lines.push(format!("    copia    : {}", c));
        }
        lines.push("".to_string());
    }

    let _ = fs::write(rel_path, lines.join("\n"));
}
