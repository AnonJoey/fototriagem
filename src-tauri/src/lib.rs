pub mod actions;
pub mod dates;
pub mod library;
pub mod takeout;

use actions::GerenciadorAcoes;
use library::{carregar_biblioteca, MesResumo, MidiaItem, ScanResult, LIXO};
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
                    if !name.starts_with('.') && name != LIXO {
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

#[tauri::command]
fn move_item(item_path: String, target_folder_name: String, state: State<'_, AppState>) -> Result<String, String> {
    let destino = state.destino.lock().unwrap().clone().ok_or("Destino não configurado")?;
    let target_dir = destino.join(target_folder_name);

    let mut acoes_guard = state.acoes.lock().unwrap();
    let acoes = acoes_guard.as_mut().ok_or("Ações não inicializadas")?;

    let novo_caminho = acoes.mover(&item_path, &target_dir)?;

    let mut itens_guard = state.itens.lock().unwrap();
    if let Some(item) = itens_guard.iter_mut().find(|i| i.caminho == item_path) {
        item.caminho = novo_caminho.clone();
        item.triada = true;
    }

    Ok(novo_caminho)
}

#[tauri::command]
fn trash_item(item_path: String, state: State<'_, AppState>) -> Result<String, String> {
    let mut acoes_guard = state.acoes.lock().unwrap();
    let acoes = acoes_guard.as_mut().ok_or("Ações não inicializadas")?;

    let novo_caminho = acoes.para_o_lixo(&item_path)?;

    let mut itens_guard = state.itens.lock().unwrap();
    if let Some(item) = itens_guard.iter_mut().find(|i| i.caminho == item_path) {
        item.caminho = novo_caminho.clone();
        item.triada = true;
    }

    Ok(novo_caminho)
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
    if let Some(item) = itens_guard.iter_mut().find(|i| i.caminho == item_restaurado.atual || i.caminho == item_restaurado.origem) {
        item.caminho = item_restaurado.origem.clone();
        item.triada = false;
        return Ok(Some(item.clone()));
    }

    Ok(None)
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
            undo_last_action
        ])
        .run(tauri::generate_context!())
        .expect("error while running fototriagem tauri application");
}
