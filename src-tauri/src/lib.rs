pub mod actions;
pub mod dates;
pub mod library;
pub mod takeout;

use actions::{GerenciadorAcoes, Movimento};
use library::{carregar_biblioteca, Copia, MesResumo, MidiaItem, ScanResult, COPIAS, LIXO};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

pub struct AppState {
    pub origem: Mutex<Option<PathBuf>>,
    pub destino: Mutex<Option<PathBuf>>,
    pub itens: Mutex<Vec<MidiaItem>>,
    pub acoes: Mutex<Option<GerenciadorAcoes>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderInfo {
    pub name: String,
    pub path: String,
}

#[tauri::command]
fn scan_library(
    origem_path: String,
    destino_path: Option<String>,
    incluir_especiais: bool,
    state: State<'_, AppState>,
) -> Result<ScanResult, String> {
    let origem = PathBuf::from(&origem_path);
    if !origem.is_dir() {
        return Err(format!("Pasta de origem não encontrada: {}", origem_path));
    }

    let destino = if let Some(d) = destino_path {
        PathBuf::from(d)
    } else {
        origem.clone()
    };

    let scan = carregar_biblioteca(&origem, &destino, incluir_especiais);

    *state.origem.lock().unwrap() = Some(origem);
    *state.destino.lock().unwrap() = Some(destino.clone());
    *state.itens.lock().unwrap() = scan.itens.clone();
    *state.acoes.lock().unwrap() = Some(GerenciadorAcoes::new(destino));

    Ok(scan)
}

#[tauri::command]
fn get_months(state: State<'_, AppState>) -> Result<Vec<MesResumo>, String> {
    let itens_guard = state.itens.lock().unwrap();
    let mut meses_map: std::collections::HashMap<(i32, u32), Vec<&MidiaItem>> = std::collections::HashMap::new();

    for item in itens_guard.iter() {
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
        let nome_mes = if mes >= 1 && (mes as usize) < library::MESES_PT.len() {
            library::MESES_PT[mes as usize].to_string()
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

    Ok(meses)
}

#[tauri::command]
fn get_month_items(ano: i32, mes: u32, state: State<'_, AppState>) -> Result<Vec<MidiaItem>, String> {
    let itens_guard = state.itens.lock().unwrap();
    let filtrados: Vec<MidiaItem> = itens_guard
        .iter()
        .filter(|i| i.ano == ano && i.mes == mes)
        .cloned()
        .collect();
    Ok(filtrados)
}

#[tauri::command]
fn get_target_folders(state: State<'_, AppState>) -> Result<Vec<FolderInfo>, String> {
    let destino_opt = state.destino.lock().unwrap().clone();
    let destino = match destino_opt {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };

    if !destino.exists() {
        return Ok(Vec::new());
    }

    let mut folders = Vec::new();
    if let Ok(entries) = fs::read_dir(&destino) {
        for entry in entries.flatten() {
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.starts_with('.') && name != LIXO && name != COPIAS {
                        folders.push(FolderInfo {
                            name,
                            path: entry.path().to_string_lossy().to_string(),
                        });
                    }
                }
            }
        }
    }
    folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(folders)
}

#[tauri::command]
fn create_target_folder(name: String, state: State<'_, AppState>) -> Result<FolderInfo, String> {
    let destino_opt = state.destino.lock().unwrap().clone();
    let destino = destino_opt.ok_or("Destino não configurado")?;

    let clean_name = name.trim().trim_matches('/');
    if clean_name.is_empty() {
        return Err("Nome de pasta inválido".to_string());
    }

    let target = destino.join(clean_name);
    fs::create_dir_all(&target).map_err(|e| format!("Erro ao criar pasta: {}", e))?;

    Ok(FolderInfo {
        name: clean_name.to_string(),
        path: target.to_string_lossy().to_string(),
    })
}

#[tauri::command]
fn rename_target_folder(old_name: String, new_name: String, state: State<'_, AppState>) -> Result<FolderInfo, String> {
    let destino_opt = state.destino.lock().unwrap().clone();
    let destino = destino_opt.ok_or("Destino não configurado")?;

    let old_path = destino.join(&old_name);
    let new_path = destino.join(&new_name);

    if !old_path.exists() {
        return Err(format!("Pasta não existe: {}", old_name));
    }
    if new_path.exists() && new_path != old_path {
        return Err(format!("Já existe uma pasta chamada: {}", new_name));
    }

    fs::rename(&old_path, &new_path).map_err(|e| format!("Erro ao renomear pasta: {}", e))?;

    Ok(FolderInfo {
        name: new_name,
        path: new_path.to_string_lossy().to_string(),
    })
}

/// O que a varredura sabe sobre este arquivo: a data a carimbar, o sidecar a
/// levar junto e as copias identicas a parquear. A data so vai quando veio de
/// uma fonte de verdade; se a propria cadeia caiu no mtime, nao ha o que
/// corrigir e o arquivo segue como esta.
struct DadosDoItem {
    timestamp: Option<i64>,
    sidecar: Option<String>,
    copias: Vec<Copia>,
}

fn dados_do_item(item_path: &str, state: &State<'_, AppState>) -> DadosDoItem {
    let itens_guard = state.itens.lock().unwrap();
    match itens_guard.iter().find(|i| i.caminho == item_path) {
        Some(item) => DadosDoItem {
            timestamp: if item.origem_data == "mtime" { None } else { Some(item.timestamp) },
            sidecar: item.sidecar.clone(),
            copias: item.copias.clone(),
        },
        None => DadosDoItem { timestamp: None, sidecar: None, copias: Vec::new() },
    }
}

impl DadosDoItem {
    fn movimento<'a>(&'a self, caminho: &'a str) -> Movimento<'a> {
        Movimento {
            caminho,
            timestamp: self.timestamp,
            sidecar: self.sidecar.as_deref(),
            copias: &self.copias,
        }
    }
}

fn marcar_triada(item_path: &str, novo_caminho: &str, state: &State<'_, AppState>) {
    let mut itens_guard = state.itens.lock().unwrap();
    if let Some(item) = itens_guard.iter_mut().find(|i| i.caminho == item_path) {
        item.caminho = novo_caminho.to_string();
        item.triada = true;
        // O sidecar viajou junto e agora se chama <midia>.json ao lado dela.
        if item.sidecar.is_some() {
            if let Some(nome) = std::path::Path::new(novo_caminho).file_name().and_then(|n| n.to_str()) {
                item.sidecar = Some(format!("{}.json", nome));
            }
        }
    }
}

#[tauri::command]
fn move_item(item_path: String, target_folder_name: String, state: State<'_, AppState>) -> Result<String, String> {
    let destino = state.destino.lock().unwrap().clone().ok_or("Destino não configurado")?;
    let target_dir = destino.join(target_folder_name);

    let dados = dados_do_item(&item_path, &state);

    let novo_caminho = {
        let mut acoes_guard = state.acoes.lock().unwrap();
        let acoes = acoes_guard.as_mut().ok_or("Ações não inicializadas")?;
        acoes.mover(dados.movimento(&item_path), &target_dir)?
    };

    marcar_triada(&item_path, &novo_caminho, &state);

    Ok(novo_caminho)
}

#[tauri::command]
fn trash_item(item_path: String, state: State<'_, AppState>) -> Result<String, String> {
    let dados = dados_do_item(&item_path, &state);

    let novo_caminho = {
        let mut acoes_guard = state.acoes.lock().unwrap();
        let acoes = acoes_guard.as_mut().ok_or("Ações não inicializadas")?;
        acoes.para_o_lixo(dados.movimento(&item_path))?
    };

    marcar_triada(&item_path, &novo_caminho, &state);

    Ok(novo_caminho)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubPastaInfo {
    pub nome: String,
    pub caminho: String,
    pub total_fotos: usize,
    pub total_videos: usize,
    pub total_bytes: u64,
    pub subpastas_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemFolderOverview {
    pub caminho_atual: String,
    pub caminho_pai: Option<String>,
    pub subpastas: Vec<SubPastaInfo>,
    pub fotos_diretas: usize,
    pub videos_diretos: usize,
    pub bytes_diretos: u64,
}

#[tauri::command]
fn undo_last_action(state: State<'_, AppState>) -> Result<Option<MidiaItem>, String> {
    let mut acoes_guard = state.acoes.lock().unwrap();
    let acoes = acoes_guard.as_mut().ok_or("Ações não inicializadas")?;

    let res = acoes.desfazer()?;
    let item_restaurado = match res {
        Some(i) => i,
        None => return Ok(None),
    };

    let mut itens_guard = state.itens.lock().unwrap();
    if let Some(item) = itens_guard.iter_mut().find(|i| {
        i.caminho == item_restaurado.caminho_anterior 
        || i.caminho == item_restaurado.caminho_restaurado
        || i.caminho == item_restaurado.origem_original
    }) {
        item.caminho = item_restaurado.caminho_restaurado.clone();
        item.triada = false;
        if item.sidecar.is_some() {
            if let Some(nome) = item_restaurado
                .sidecar_restaurado
                .as_ref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .and_then(|n| n.to_str())
            {
                item.sidecar = Some(nome.to_string());
            }
        }
        return Ok(Some(item.clone()));
    }

    Ok(None)
}

/// Onde o explorador comeca quando ninguem disse por onde comecar.
///
/// No Android nao ha `$HOME` util: o que o usuario chama de "o armazenamento
/// do celular" e /storage/emulated/0, e e la que um Takeout descompactado vai
/// parar. No desktop continua sendo a pasta do usuario.
fn raiz_padrao() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        for cand in ["/storage/emulated/0", "/sdcard"] {
            let p = PathBuf::from(cand);
            if p.is_dir() {
                return p;
            }
        }
        return PathBuf::from("/");
    }

    #[cfg(not(target_os = "android"))]
    {
        match std::env::var("HOME") {
            Ok(h) if !h.is_empty() => PathBuf::from(h),
            _ => PathBuf::from("/"),
        }
    }
}

#[tauri::command]
fn list_system_folders(path: Option<String>) -> Result<SystemFolderOverview, String> {
    let base_path = match path {
        Some(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => {
            let raiz = raiz_padrao();
            let pics = raiz.join("Pictures");
            if pics.is_dir() { pics } else { raiz }
        }
    };

    if !base_path.is_dir() {
        return Err(format!("Diretório não encontrado: {}", base_path.display()));
    }

    let caminho_pai = base_path.parent().map(|p| p.to_string_lossy().to_string());
    let mut subpastas = Vec::new();
    let mut fotos_diretas = 0;
    let mut videos_diretos = 0;
    let mut bytes_diretos = 0;

    let exts_foto = ["jpg", "jpeg", "png", "webp", "heic", "gif", "avif", "raw", "dng", "cr2", "nef", "arw"];
    let exts_video = ["mp4", "mov", "avi", "mkv", "webm", "m4v", "3gp"];

    if let Ok(entries) = fs::read_dir(&base_path) {
        for entry in entries.flatten() {
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }

            if p.is_dir() {
                let mut f_count = 0;
                let mut v_count = 0;
                let mut bytes = 0;
                let mut sub_count = 0;

                if let Ok(sub_entries) = fs::read_dir(&p) {
                    for sub_e in sub_entries.flatten() {
                        let sp = sub_e.path();
                        let sname = sub_e.file_name().to_string_lossy().to_string();
                        if sname.starts_with('.') { continue; }

                        if sp.is_dir() {
                            sub_count += 1;
                        } else if let Some(ext) = sp.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) {
                            let len = sub_e.metadata().map(|m| m.len()).unwrap_or(0);
                            bytes += len;
                            if exts_foto.contains(&ext.as_str()) {
                                f_count += 1;
                            } else if exts_video.contains(&ext.as_str()) {
                                v_count += 1;
                            }
                        }
                    }
                }

                subpastas.push(SubPastaInfo {
                    nome: name,
                    caminho: p.to_string_lossy().to_string(),
                    total_fotos: f_count,
                    total_videos: v_count,
                    total_bytes: bytes,
                    subpastas_count: sub_count,
                });
            } else if let Some(ext) = p.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) {
                let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
                bytes_diretos += len;
                if exts_foto.contains(&ext.as_str()) {
                    fotos_diretas += 1;
                } else if exts_video.contains(&ext.as_str()) {
                    videos_diretos += 1;
                }
            }
        }
    }

    subpastas.sort_by(|a, b| a.nome.to_lowercase().cmp(&b.nome.to_lowercase()));

    Ok(SystemFolderOverview {
        caminho_atual: base_path.to_string_lossy().to_string(),
        caminho_pai,
        subpastas,
        fotos_diretas,
        videos_diretos,
        bytes_diretos,
    })
}

#[tauri::command]
fn create_system_folder(parent_path: String, folder_name: String) -> Result<String, String> {
    let parent = PathBuf::from(&parent_path);
    if !parent.is_dir() {
        return Err(format!("Diretório pai não existe: {}", parent_path));
    }
    let clean_name = folder_name.trim().trim_matches('/');
    if clean_name.is_empty() {
        return Err("Nome de pasta inválido".to_string());
    }
    let target = parent.join(clean_name);
    fs::create_dir_all(&target).map_err(|e| format!("Erro ao criar pasta: {}", e))?;
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
fn rename_system_folder(old_path: String, new_name: String) -> Result<String, String> {
    let old = PathBuf::from(&old_path);
    if !old.is_dir() {
        return Err(format!("Pasta não encontrada: {}", old_path));
    }
    let parent = old.parent().ok_or("Não é possível renomear diretório raiz")?;
    let clean_name = new_name.trim().trim_matches('/');
    if clean_name.is_empty() {
        return Err("Nome de pasta inválido".to_string());
    }
    let new_path = parent.join(clean_name);
    if new_path.exists() && new_path != old {
        return Err(format!("Já existe uma pasta chamada: {}", clean_name));
    }
    fs::rename(&old, &new_path).map_err(|e| format!("Erro ao renomear: {}", e))?;
    Ok(new_path.to_string_lossy().to_string())
}

#[tauri::command]
fn get_default_paths() -> Result<Vec<String>, String> {
    // Sugestoes de partida, nao caminhos garantidos: quem escolhe de verdade
    // e o explorador de pastas. No celular o Takeout descompactado costuma
    // cair em Download/; no desktop, na pasta de teste.
    let raiz = raiz_padrao();
    let candidatos: Vec<PathBuf> = if cfg!(target_os = "android") {
        vec![
            raiz.join("Download"),
            raiz.join("Pictures/Organizadas"),
            raiz.join("DCIM"),
            raiz.join("Takeout"),
            raiz.join("Pictures"),
        ]
    } else {
        vec![
            raiz.join("Pictures/Teste_Fototriagem/Google Photos"),
            raiz.join("Pictures/Teste_Fototriagem/Destino_Organizado"),
        ]
    };
    Ok(candidatos.into_iter().map(|p| p.to_string_lossy().to_string()).collect())
}

#[tauri::command]
fn load_image_data(path: String) -> Result<String, String> {
    use base64::Engine;
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err(format!("Arquivo não encontrado: {}", path));
    }
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("jpeg").to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "avif" => "image/avif",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "image/jpeg",
    };
    let bytes = fs::read(&p).map_err(|e| format!("Erro ao ler arquivo: {}", e))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            origem: Mutex::new(None),
            destino: Mutex::new(None),
            itens: Mutex::new(Vec::new()),
            acoes: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            scan_library,
            get_months,
            get_month_items,
            get_target_folders,
            create_target_folder,
            rename_target_folder,
            move_item,
            trash_item,
            undo_last_action,
            load_image_data,
            get_default_paths,
            list_system_folders,
            create_system_folder,
            rename_system_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running fototriagem tauri application");
}
