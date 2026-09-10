use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use unicode_normalization::UnicodeNormalization;

pub const SUFIXOS_EDITADO: &[&str] = &[
    "-edited", "-effects", "-smile", "-mix", // EN
    "-edytowane",                            // PL
    "-bearbeitet",                           // DE
    "-bewerkt",                              // NL
    "-編集済み",                               // JA
    "-modificato",                           // IT
    "-modifié",                              // FR
    "-ha editado",                           // ES
    "-editat",                               // CA
    "-editado",                              // PT
];

pub const JSON_DE_PASTA: &[&str] = &[
    "metadata.json",
    "print-subscriptions.json",
    "shared_album_comments.json",
    "user-generated-memory-titles.json",
];

pub const PASTAS_ESPECIAIS: &[&str] = &[
    "archive", "trash", "bin", "locked folder", "lixeira", "arquivo",
];

const LIMITE_NOME_JSON: usize = 51;

pub fn nfc(s: &str) -> String {
    s.nfc().collect()
}

pub fn eh_pasta_de_ano(pasta: &Path) -> bool {
    if let Some(name) = pasta.file_name().and_then(|n| n.to_str()) {
        lazy_static_or_local_re_ano(name)
    } else {
        false
    }
}

fn lazy_static_or_local_re_ano(name: &str) -> bool {
    let re = Regex::new(r"^Photos from (20|19|18)\d{2}$").unwrap();
    re.is_match(name)
}

pub fn eh_pasta_especial(pasta: &Path) -> bool {
    if let Some(name) = pasta.file_name().and_then(|n| n.to_str()) {
        let clean = name.trim().to_lowercase();
        PASTAS_ESPECIAIS.contains(&clean.as_str())
    } else {
        false
    }
}

pub fn eh_pasta_de_album(pasta: &Path) -> bool {
    if eh_pasta_de_ano(pasta) || eh_pasta_especial(pasta) {
        return false;
    }
    if let Some(parent) = pasta.parent() {
        if let Ok(entries) = fs::read_dir(parent) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() && eh_pasta_de_ano(&entry.path()) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

pub fn variantes_do_nome(nome: &str) -> Vec<String> {
    let mut candidatos = Vec::new();
    let nome_nfc = nfc(nome);
    let path = Path::new(&nome_nfc);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let ext_with_dot = if ext.is_empty() { String::new() } else { format!(".{}", ext) };

    // 1. Exato
    candidatos.push(nome_nfc.clone());

    // 2. Truncagem em 51 caracteres contando .json (46 do nome)
    if nome_nfc.chars().count() + 5 > LIMITE_NOME_JSON {
        let truncado: String = nome_nfc.chars().take(46).collect();
        candidatos.push(truncado);
    }

    // 3. Parentese trocado de lugar: image(11).jpg -> image.jpg(11)
    let re_paren = Regex::new(r"\(\d+\)\.").unwrap();
    if let Some(m) = re_paren.find_iter(&nome_nfc).last() {
        let paren_with_dot = m.as_str();
        let paren = paren_with_dot.replace('.', "");
        if let Some(pos) = nome_nfc.rfind(&paren) {
            let transformed = format!("{}{}{}", &nome_nfc[..pos], &nome_nfc[pos + paren.len()..], paren);
            candidatos.push(transformed);
        }
    }

    // 4. Copia editada em 10 idiomas
    let stem_lower = stem.to_lowercase();
    for &sufixo in SUFIXOS_EDITADO {
        if stem_lower.ends_with(sufixo) {
            let base_len = stem.len() - sufixo.len();
            candidatos.push(format!("{}{}", &stem[..base_len], ext_with_dot));
            break;
        }
    }

    // 5. Enviado sem extensão original
    candidatos.push(stem.to_string());

    // 6. Regex sufixo extra
    let re_extra = Regex::new(r"(-[A-Za-zÀ-ÖØ-öø-ÿ]+(\(\d\))?)\.\w+$").unwrap();
    if let Some(caps) = re_extra.captures(&nome_nfc) {
        if let Some(matched) = caps.get(1) {
            candidatos.push(nome_nfc.replace(matched.as_str(), ""));
        }
    }

    // 7. Duplicata (1). vira .
    let re_digito = Regex::new(r"\(\d\)\.").unwrap();
    if re_digito.is_match(&nome_nfc) {
        candidatos.push(re_digito.replace(&nome_nfc, ".").to_string());
    }

    candidatos
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarEntry {
    pub ts: i64,
    pub exato: bool,
    pub json_file: String,
}

pub fn indexar_pasta(pasta: &Path) -> HashMap<String, SidecarEntry> {
    let mut indice = HashMap::new();
    let entries = match fs::read_dir(pasta) {
        Ok(e) => e,
        Err(_) => return indice,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase() == "json" {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if JSON_DE_PASTA.contains(&file_name) {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(title) = json_val.get("title").and_then(|t| t.as_str()) {
                        let photo_time = json_val.get("photoTakenTime");
                        let creation_time = json_val.get("creationTime");
                        let stamp = photo_time.or(creation_time);

                        if let Some(stamp_obj) = stamp {
                            if let Some(ts_val) = stamp_obj.get("timestamp") {
                                let ts_opt = ts_val.as_str().and_then(|s| s.parse::<i64>().ok())
                                    .or_else(|| ts_val.as_i64());

                                if let Some(ts) = ts_opt {
                                    let is_exact = photo_time.is_some();
                                    indice.entry(nfc(title).to_lowercase()).or_insert(SidecarEntry {
                                        ts,
                                        exato: is_exact,
                                        json_file: file_name.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    indice
}

pub fn achar_sidecar(
    midia_path: &Path,
    indice: &HashMap<String, SidecarEntry>,
    arquivos_da_pasta: &HashSet<String>,
) -> Option<SidecarEntry> {
    let file_name = midia_path.file_name()?.to_str()?;
    let parent = midia_path.parent()?;

    for cand in variantes_do_nome(file_name) {
        let chave = nfc(&cand).to_lowercase();
        if let Some(entry) = indice.get(&chave) {
            return Some(entry.clone());
        }

        for ext_cand in &[format!("{}.json", cand), format!("{}.supplemental-metadata.json", cand)] {
            if arquivos_da_pasta.contains(ext_cand) {
                let json_path = parent.join(ext_cand);
                if let Ok(content) = fs::read_to_string(&json_path) {
                    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&content) {
                        let photo_time = json_val.get("photoTakenTime");
                        let creation_time = json_val.get("creationTime");
                        let stamp = photo_time.or(creation_time);
                        if let Some(stamp_obj) = stamp {
                            if let Some(ts_val) = stamp_obj.get("timestamp") {
                                let ts_opt = ts_val.as_str().and_then(|s| s.parse::<i64>().ok())
                                    .or_else(|| ts_val.as_i64());
                                if let Some(ts) = ts_opt {
                                    return Some(SidecarEntry {
                                        ts,
                                        exato: photo_time.is_some(),
                                        json_file: ext_cand.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let alvo = nfc(file_name).to_lowercase();
    for (titulo, reg) in indice {
        if titulo.chars().count() >= 12 {
            let mut chars = titulo.chars();
            chars.next_back();
            let prefixo: String = chars.collect();
            if alvo.starts_with(&prefixo) {
                return Some(reg.clone());
            }
        }
    }

    None
}
