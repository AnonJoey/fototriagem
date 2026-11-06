use chrono::{DateTime, Datelike, Local, Utc};
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
/// Quarentena das copias identicas. Elas nao sao apagadas nem misturadas com
/// a foto escolhida: vao para `_copias/<pasta que voce escolheu>/`, de modo
/// que a origem esvazie e sobre uma pasta so para conferir e apagar no fim.
pub const COPIAS: &str = "_copias";
pub const CACHE_FILE: &str = ".triagem_cache.json";
pub const RELATORIO_COPIAS: &str = "COPIAS-NAO-APAGADAS.txt";

/// Uma copia byte a byte identica da foto escolhida, achada noutro album.
/// Carrega o proprio sidecar porque cada album do Takeout tem o seu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Copia {
    pub caminho: String,
    #[serde(default)]
    pub sidecar: Option<String>,
}

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
    pub copias: Vec<Copia>,
    pub triada: bool,
    pub eh_video: bool,
    /// Nome do .json do Takeout que deu a data, na mesma pasta da midia.
    /// None quando a data veio de EXIF, do container ou do mtime.
    #[serde(default)]
    pub sidecar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheItem {
    pub sig: String,
    pub ts: i64,
    pub src: String,
    /// Nome do sidecar. `default` para que um cache escrito antes desta
    /// versao continue legivel em vez de invalidar a varredura inteira.
    #[serde(default)]
    pub sc: Option<String>,
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

pub fn normalizar_caminho(p: &Path) -> std::path::PathBuf {
    if let Ok(c) = p.canonicalize() {
        return c;
    }
    let mut restantes = Vec::new();
    let mut atual = p;
    while !atual.exists() {
        if let Some(nome) = atual.file_name() {
            restantes.push(nome);
        }
        match atual.parent() {
            Some(pai) if pai != atual => atual = pai,
            _ => break,
        }
    }
    let mut base = atual.canonicalize().unwrap_or_else(|_| atual.to_path_buf());
    for parte in restantes.into_iter().rev() {
        base.push(parte);
    }
    let mut limpo = std::path::PathBuf::new();
    for comp in base.components() {
        match comp {
            std::path::Component::CurDir => {},
            std::path::Component::ParentDir => { limpo.pop(); },
            outro => limpo.push(outro),
        }
    }
    limpo
}

/// Valida se os caminhos de origem e destino sao seguros para uso.
///
/// Recusa:
/// 1. Origem inexistente ou nao sendo diretorio.
/// 2. Origem e destino apontando para a mesma pasta.
/// 3. Destino sendo subpasta dentro da origem (sobreposicao).
/// 4. Origem sendo subpasta dentro do destino (sobreposicao).
pub fn validar_caminhos_origem_destino(origem: &Path, destino: &Path) -> Result<(), String> {
    if !origem.is_dir() {
        return Err(format!("Pasta de origem não encontrada: {}", origem.display()));
    }

    let norm_origem = normalizar_caminho(origem);
    let norm_destino = normalizar_caminho(destino);

    if norm_origem == norm_destino {
        return Err("A pasta de origem e a pasta de destino não podem ser a mesma pasta.".to_string());
    }

    if norm_destino.starts_with(&norm_origem) {
        return Err("A pasta de destino não pode ser uma subpasta da origem (sobreposição de diretórios).".to_string());
    }

    if norm_origem.starts_with(&norm_destino) {
        return Err("A pasta de origem não pode ser uma subpasta do destino (sobreposição de diretórios).".to_string());
    }

    Ok(())
}

pub fn carregar_biblioteca(
    origem: &Path,
    destino: &Path,
    incluir_especiais: bool,
) -> ScanResult {
    // A validacao de sobreposicao entre origem e destino NAO mora aqui. Ela e
    // uma regra de MOVIMENTO, e varrer nao move nada: existe uso legitimo de
    // varrer uma pasta passando ela mesma como origem e destino, so para
    // inspecionar o que ha dentro.
    //
    // Ela vivia aqui e devolvia uma varredura vazia, o que quebrou
    // `o_que_a_varredura_achou_e_o_que_chega_no_destino`: a segunda metade do
    // teste revarre a pasta de destino para conferir o que chegou, e recebia
    // zero itens com o aviso "a pasta de origem e a pasta de destino nao podem
    // ser a mesma pasta". Pior que falhar: uma varredura vazia e
    // indistinguivel de uma pasta vazia.
    //
    // O guarda esta em `lib.rs`, no comando que inicia a triagem, ANTES de
    // chamar esta funcao, que e a fronteira onde a regra vale.
    let mut especiais_ignoradas = 0;
    let mut caminhos_candidatos = Vec::new();
    let destino_lixo = destino.join(LIXO);
    let destino_copias = destino.join(COPIAS);

    let walker = WalkDir::new(origem).into_iter().filter_entry(|e| {
        if e.depth() == 0 {
            return true;
        }
        let file_name = e.file_name();
        let s = file_name.to_string_lossy();
        if s.starts_with('.') && s != "." && s != ".." {
            return false;
        }
        if e.file_type().is_dir() {
            if e.path() == destino_lixo || e.path() == destino_copias {
                return false;
            }
            if !incluir_especiais {
                let s_clean = s.trim().to_lowercase();
                if PASTAS_ESPECIAIS.contains(&s_clean.as_str()) {
                    return false;
                }
            }
        }
        true
    });

    for entry in walker.filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_file() || !eh_suportado(p) {
            continue;
        }
        if p.starts_with(&destino_lixo) || p.starts_with(&destino_copias) {
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

    let total_arquivos = caminhos_candidatos.len();
    let cache_path = destino.join(CACHE_FILE);
    let mut cache_map: HashMap<String, CacheItem> = if cache_path.exists() {
        fs::read_to_string(&cache_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    let mut cache_alterado = false;

    let mut leitor = LeitorDeDatas::new();
    let mut itens_brutos = Vec::with_capacity(total_arquivos);

    for p in caminhos_candidatos {
        let meta = match fs::metadata(&p) {
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
        // "v2": desde 06/11/2026 o EXIF vira instante pelo fuso local. Entradas do
        // cache gravadas antes (EXIF lido como UTC) nao casam e sao lidas de novo.
        let sig = format!("v2:{}:{}", size, mtime);

        let path_str = match p.into_os_string().into_string() {
            Ok(s) => s,
            Err(os) => os.to_string_lossy().into_owned(),
        };

        let cache_valido = cache_map
            .get(&path_str)
            .filter(|c| c.sig == sig)
            .map(|c| (c.ts, c.src.clone(), c.sc.clone()));

        let path_ref = Path::new(&path_str);
        let (ts, origem_data, sidecar) = match cache_valido {
            Some(triple) => triple,
            None => {
                let lida = leitor.ler(path_ref);
                cache_map.insert(
                    path_str.clone(),
                    CacheItem {
                        sig: sig.clone(),
                        ts: lida.ts,
                        src: lida.fonte.clone(),
                        sc: lida.sidecar.clone(),
                    },
                );
                cache_alterado = true;
                (lida.ts, lida.fonte, lida.sidecar)
            }
        };

        // O mes e a hora mostrados sao os do relogio local: uma foto das 23:30 de
        // 31/03 e de marco. Em UTC (Brasil, -3 h) ela caia em abril.
        let dt = DateTime::from_timestamp(ts, 0)
            .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
            .with_timezone(&Local);
        let data_formatada = dt.format("%d/%m/%Y %H:%M").to_string();
        let nome = path_ref.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let is_vid = eh_video(path_ref);

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
            sidecar,
        });
    }

    let (itens_dedup, duplicatas_removidas) = deduplicar(itens_brutos);

    if cache_alterado {
        let _ = fs::create_dir_all(destino);
        if let Ok(json_str) = serde_json::to_string_pretty(&cache_map) {
            let _ = fs::write(&cache_path, json_str);
        }
    }

    escrever_relatorio_copias(origem, destino, &itens_dedup);

    let mut itens = itens_dedup;
    itens.sort_by_key(|i| i.timestamp);
    itens.shrink_to_fit();

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
            "Aviso: {} cópias idênticas ({}) seguirão a decisão para {}/. Detalhes em {}",
            duplicatas_removidas,
            formatar_tamanho(total_bytes),
            COPIAS,
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
        total_arquivos,
        duplicatas_removidas,
        especiais_ignoradas,
        sem_data_real,
        meses,
        avisos,
    }
}

fn deduplicar(mut itens: Vec<MidiaItem>) -> (Vec<MidiaItem>, usize) {
    if itens.is_empty() {
        return (itens, 0);
    }

    itens.sort_unstable_by_key(|i| i.tamanho_bytes);

    let mut saida = Vec::with_capacity(itens.len());
    let mut duplicatas_removidas = 0;

    let get_ordem = |origem: &str| -> i32 {
        match origem {
            "takeout" => 0,
            "takeout~" => 1,
            "exif" => 2,
            "video" => 3,
            _ => 9,
        }
    };

    let mut itens_iter = itens.into_iter().peekable();

    while let Some(primeiro) = itens_iter.next() {
        let tamanho = primeiro.tamanho_bytes;
        if tamanho == 0 || itens_iter.peek().map_or(true, |prox| prox.tamanho_bytes != tamanho) {
            saida.push(primeiro);
            continue;
        }

        let mut grupo = vec![primeiro];
        while let Some(prox) = itens_iter.next_if(|p| p.tamanho_bytes == tamanho) {
            grupo.push(prox);
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
            vencedora.copias = iguais
                .into_iter()
                .map(|i| Copia { caminho: i.caminho, sidecar: i.sidecar })
                .collect();
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
    lines.push(format!("gerado em {}", Local::now().format("%d/%m/%Y %H:%M")));
    lines.push(format!("origem: {}", origem.display()));
    lines.push("".to_string());
    lines.push("Nada foi apagado. A triagem mostra apenas a primeira de cada grupo.".to_string());
    lines.push(format!(
        "Ao decidir o destino de uma foto, as copias dela vao para {}/<pasta escolhida>/,",
        COPIAS
    ));
    lines.push("de onde podem ser conferidas e apagadas de uma vez no fim. Ctrl+Z devolve".to_string());
    lines.push("a foto e as copias dela aos lugares de origem.".to_string());
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
            lines.push(format!("    copia    : {}", c.caminho));
        }
        lines.push("".to_string());
    }

    let _ = fs::write(rel_path, lines.join("\n"));
}
