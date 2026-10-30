use chrono::{DateTime, Datelike, Utc};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::library::{MesResumo, MidiaItem, ScanResult};
use crate::FolderInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImmichConfig {
    pub server_url: String,
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImmichAssetDto {
    pub id: String,
    #[serde(default)]
    pub original_file_name: Option<String>,
    #[serde(default)]
    pub original_path: Option<String>,
    #[serde(default)]
    pub file_created_at: Option<String>,
    #[serde(default)]
    pub file_modified_at: Option<String>,
    #[serde(rename = "type", default)]
    pub asset_type: Option<String>,
    #[serde(default)]
    pub is_favorite: Option<bool>,
    #[serde(default)]
    pub is_archived: Option<bool>,
    #[serde(default)]
    pub is_trashed: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImmichSearchResponse {
    #[serde(default)]
    pub assets: Option<ImmichAssetsContainer>,
}

#[derive(Debug, Deserialize)]
pub struct ImmichAssetsContainer {
    #[serde(default)]
    pub items: Vec<ImmichAssetDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImmichAlbumDto {
    pub id: String,
    pub album_name: String,
    #[serde(default)]
    pub asset_count: Option<usize>,
}

fn create_client(api_key: &str) -> Result<Client, String> {
    let mut headers = HeaderMap::new();
    if !api_key.is_empty() {
        let val = HeaderValue::from_str(api_key).map_err(|e| e.to_string())?;
        headers.insert("x-api-key", val);
        headers.insert("Accept", HeaderValue::from_static("application/json"));
    }
    Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())
}

pub fn immich_ping(server_url: &str) -> Result<bool, String> {
    let client = create_client("")?;
    let base = server_url.trim_end_matches('/');
    let url = format!("{}/api/server/ping", base);
    let res = client.get(&url).send();
    match res {
        Ok(r) if r.status().is_success() => Ok(true),
        _ => {
            // Fallback para /api/server-info/ping
            let url_old = format!("{}/api/server-info/ping", base);
            let res_old = client.get(&url_old).send().map_err(|e| format!("Falha ao conectar no Immich: {}", e))?;
            if res_old.status().is_success() {
                Ok(true)
            } else {
                Err(format!("Status de resposta inválido: {}", res_old.status()))
            }
        }
    }
}

pub fn immich_get_user(server_url: &str, api_key: &str) -> Result<String, String> {
    let client = create_client(api_key)?;
    let url = format!("{}/api/users/me", server_url.trim_end_matches('/'));
    let res = client.get(&url).send().map_err(|e| format!("Erro ao obter usuário: {}", e))?;
    if !res.status().is_success() {
        return Err(format!("Chave de API inválida ou não autorizada (HTTP {})", res.status()));
    }
    let body: serde_json::Value = res.json().map_err(|e| e.to_string())?;
    let name = body.get("name")
        .and_then(|v| v.as_str())
        .or_else(|| body.get("email").and_then(|v| v.as_str()))
        .unwrap_or("Usuário Immich");
    Ok(name.to_string())
}

pub fn immich_scan_library(server_url: &str, api_key: &str) -> Result<ScanResult, String> {
    let client = create_client(api_key)?;
    let base = server_url.trim_end_matches('/');
    
    let url = format!("{}/api/search/metadata", base);
    let payload = serde_json::json!({
        "take": 5000,
        "order": "desc",
        "isArchived": false,
        "isTrashed": false
    });
    
    let mut assets = Vec::new();
    let res = client.post(&url).json(&payload).send();
    
    match res {
        Ok(r) if r.status().is_success() => {
            if let Ok(search_res) = r.json::<ImmichSearchResponse>() {
                if let Some(container) = search_res.assets {
                    assets = container.items;
                }
            }
        }
        _ => {
            let fallback_url = format!("{}/api/assets", base);
            if let Ok(r) = client.get(&fallback_url).send() {
                if r.status().is_success() {
                    if let Ok(list) = r.json::<Vec<ImmichAssetDto>>() {
                        assets = list;
                    }
                }
            }
        }
    }

    if assets.is_empty() {
        let _ = immich_get_user(server_url, api_key)?;
    }

    let nomes_meses = [
        "", "Janeiro", "Fevereiro", "Março", "Abril", "Maio", "Junho",
        "Julho", "Agosto", "Setembro", "Outubro", "Novembro", "Dezembro",
    ];

    let mut itens: Vec<MidiaItem> = Vec::new();
    let mut mapa_meses: HashMap<(i32, u32), (usize, usize)> = HashMap::new();

    for a in assets {
        if a.is_trashed.unwrap_or(false) || a.is_archived.unwrap_or(false) {
            continue;
        }

        let id = a.id;
        let nome = a.original_file_name.unwrap_or_else(|| format!("{}.jpg", &id));
        let eh_video = a.asset_type.as_deref() == Some("VIDEO");

        let (ano, mes, timestamp, data_formatada) = if let Some(dt_str) = a.file_created_at.or(a.file_modified_at) {
            if let Ok(dt) = DateTime::parse_from_rfc3339(&dt_str) {
                let utc_dt = dt.with_timezone(&Utc);
                (
                    utc_dt.year(),
                    utc_dt.month(),
                    utc_dt.timestamp(),
                    utc_dt.format("%d/%m/%Y %H:%M").to_string(),
                )
            } else {
                let now = Utc::now();
                (now.year(), now.month(), now.timestamp(), "Data desconhecida".to_string())
            }
        } else {
            let now = Utc::now();
            (now.year(), now.month(), now.timestamp(), "Data desconhecida".to_string())
        };

        let entry = mapa_meses.entry((ano, mes)).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += 1;

        itens.push(MidiaItem {
            caminho: format!("immich://{}", id),
            nome,
            timestamp,
            data_formatada,
            ano,
            mes,
            origem_data: "Immich API".to_string(),
            tamanho_bytes: 0,
            copias: Vec::new(),
            triada: false,
            eh_video,
            sidecar: None,
        });
    }

    let mut chaves_ordenadas: Vec<(i32, u32)> = mapa_meses.keys().cloned().collect();
    chaves_ordenadas.sort_by(|a, b| b.cmp(a));

    let meses: Vec<MesResumo> = chaves_ordenadas
        .into_iter()
        .map(|(ano, mes)| {
            let (total, restantes) = mapa_meses[&(ano, mes)];
            let nome_mes = if (1..=12).contains(&mes) {
                format!("{} {}", nomes_meses[mes as usize], ano)
            } else {
                format!("Mês {} {}", mes, ano)
            };
            MesResumo {
                ano,
                mes,
                nome_mes,
                total,
                restantes,
                sem_data_real: 0,
                concluido: restantes == 0,
            }
        })
        .collect();

    let total = itens.len();
    Ok(ScanResult {
        itens,
        total_arquivos: total,
        duplicatas_removidas: 0,
        especiais_ignoradas: 0,
        sem_data_real: 0,
        meses,
        avisos: Vec::new(),
    })
}

pub fn immich_trash_item(server_url: &str, api_key: &str, asset_id: &str) -> Result<(), String> {
    let client = create_client(api_key)?;
    let base = server_url.trim_end_matches('/');
    let url = format!("{}/api/assets", base);
    let payload = serde_json::json!({
        "ids": [ asset_id ]
    });
    let res = client.delete(&url).json(&payload).send().map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Erro ao descartar item no Immich (HTTP {})", res.status()))
    }
}

pub fn immich_restore_item(server_url: &str, api_key: &str, asset_id: &str) -> Result<(), String> {
    let client = create_client(api_key)?;
    let base = server_url.trim_end_matches('/');
    let url = format!("{}/api/trash/restore", base);
    let payload = serde_json::json!({
        "ids": [ asset_id ]
    });
    let res = client.post(&url).json(&payload).send().map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Erro ao restaurar item no Immich (HTTP {})", res.status()))
    }
}

pub fn immich_get_albums(server_url: &str, api_key: &str) -> Result<Vec<FolderInfo>, String> {
    let client = create_client(api_key)?;
    let base = server_url.trim_end_matches('/');
    let url = format!("{}/api/albums", base);
    let res = client.get(&url).send().map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("Erro ao listar álbuns no Immich (HTTP {})", res.status()));
    }
    let list: Vec<ImmichAlbumDto> = res.json().map_err(|e| e.to_string())?;
    let folders = list
        .into_iter()
        .map(|alb| FolderInfo {
            name: alb.album_name,
            path: alb.id,
        })
        .collect();
    Ok(folders)
}

pub fn immich_create_album(server_url: &str, api_key: &str, album_name: &str) -> Result<FolderInfo, String> {
    let client = create_client(api_key)?;
    let base = server_url.trim_end_matches('/');
    let url = format!("{}/api/albums", base);
    let payload = serde_json::json!({
        "albumName": album_name
    });
    let res = client.post(&url).json(&payload).send().map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("Erro ao criar álbum no Immich (HTTP {})", res.status()));
    }
    let alb: ImmichAlbumDto = res.json().map_err(|e| e.to_string())?;
    Ok(FolderInfo {
        name: alb.album_name,
        path: alb.id,
    })
}

pub fn immich_move_to_album(server_url: &str, api_key: &str, asset_id: &str, album_id: &str) -> Result<(), String> {
    let client = create_client(api_key)?;
    let base = server_url.trim_end_matches('/');
    let url = format!("{}/api/albums/{}/assets", base, album_id);
    let payload = serde_json::json!({
        "ids": [ asset_id ]
    });
    let res = client.put(&url).json(&payload).send().map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Erro ao adicionar ao álbum no Immich (HTTP {})", res.status()))
    }
}
