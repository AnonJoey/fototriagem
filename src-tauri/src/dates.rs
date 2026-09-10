use chrono::{DateTime, NaiveDateTime, Utc};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
#[cfg(not(target_os = "android"))]
use std::process::Command;

use crate::takeout::{achar_sidecar, indexar_pasta, SidecarEntry};

pub const EXTENSOES_IMAGEM: &[&str] = &[
    "jpg", "jpeg", "png", "heic", "heif", "webp", "gif", "bmp", "tif", "tiff", "avif", "jxl",
];

pub const EXTENSOES_VIDEO: &[&str] = &[
    "mp4", "mov", "m4v", "avi", "mkv", "webm", "3gp", "mts", "m2ts",
];

pub fn eh_suportado(p: &Path) -> bool {
    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        EXTENSOES_IMAGEM.contains(&ext_lower.as_str()) || EXTENSOES_VIDEO.contains(&ext_lower.as_str())
    } else {
        false
    }
}

pub fn eh_video(p: &Path) -> bool {
    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
        EXTENSOES_VIDEO.contains(&ext.to_lowercase().as_str())
    } else {
        false
    }
}

/// Data resolvida para um arquivo, mais a proveniencia.
/// `sidecar` e o nome do .json do Takeout que deu a data, quando foi ele
/// que deu, sempre relativo a pasta onde a midia esta.
#[derive(Debug, Clone)]
pub struct DataLida {
    pub ts: i64,
    pub fonte: String,
    pub sidecar: Option<String>,
}

pub struct LeitorDeDatas {
    indices: HashMap<PathBuf, HashMap<String, SidecarEntry>>,
    arquivos_pasta: HashMap<PathBuf, Vec<String>>,
}

impl Default for LeitorDeDatas {
    fn default() -> Self {
        Self::new()
    }
}

impl LeitorDeDatas {
    pub fn new() -> Self {
        Self {
            indices: HashMap::new(),
            arquivos_pasta: HashMap::new(),
        }
    }

    fn ensure_context(&mut self, pasta: &Path) {
        if !self.indices.contains_key(pasta) {
            let indice = indexar_pasta(pasta);
            let mut arquivos = Vec::new();
            if let Ok(entries) = fs::read_dir(pasta) {
                for entry in entries.flatten() {
                    if let Ok(name) = entry.file_name().into_string() {
                        arquivos.push(name);
                    }
                }
            }
            self.indices.insert(pasta.to_path_buf(), indice);
            self.arquivos_pasta.insert(pasta.to_path_buf(), arquivos);
        }
    }

    pub fn do_sidecar(&mut self, caminho: &Path) -> Option<DataLida> {
        let parent = caminho.parent()?;
        self.ensure_context(parent);
        let indice = self.indices.get(parent)?;
        let arquivos = self.arquivos_pasta.get(parent)?;
        let reg = achar_sidecar(caminho, indice, arquivos)?;
        let tag = if reg.exato { "takeout" } else { "takeout~" };
        Some(DataLida {
            ts: reg.ts,
            fonte: tag.to_string(),
            sidecar: Some(reg.json_file),
        })
    }

    pub fn do_exif(caminho: &Path) -> Option<(i64, String)> {
        let file = File::open(caminho).ok()?;
        let mut bufreader = BufReader::new(&file);
        let exif_reader = exif::Reader::new();
        let exif_data = exif_reader.read_from_container(&mut bufreader).ok()?;

        for &tag in &[exif::Tag::DateTimeOriginal, exif::Tag::DateTimeDigitized, exif::Tag::DateTime] {
            if let Some(field) = exif_data.get_field(tag, exif::In::PRIMARY) {
                let s = field.display_value().to_string();
                let clean = s.trim().trim_matches('"');
                if clean.len() >= 19 {
                    let date_str = &clean[..19];
                    if let Ok(naive) = NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S")
                        .or_else(|_| NaiveDateTime::parse_from_str(date_str, "%Y:%m:%d %H:%M:%S"))
                    {
                        let dt: DateTime<Utc> = DateTime::from_naive_utc_and_offset(naive, Utc);
                        return Some((dt.timestamp(), "exif".to_string()));
                    }
                }
            }
        }
        None
    }

    /// Le a data do container do video chamando o ffprobe.
    ///
    /// No Android nao ha ffprobe nem PATH onde procura-lo, e tentar rodar um
    /// processo externo so gera erro por arquivo. La este elo da cadeia
    /// simplesmente nao existe: a data cai para EXIF e depois para o mtime,
    /// e `origem_data` diz isso na tela em vez de mentir.
    #[cfg(target_os = "android")]
    pub fn do_container(_caminho: &Path) -> Option<(i64, String)> {
        None
    }

    #[cfg(not(target_os = "android"))]
    pub fn do_container(caminho: &Path) -> Option<(i64, String)> {
        let output = Command::new("ffprobe")
            .args(&[
                "-v", "quiet",
                "-print_format", "json",
                "-show_format",
                caminho.to_str().unwrap_or(""),
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let json_val: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        let tags = json_val.get("format")?.get("tags")?;
        
        let creation_time = tags.get("creation_time")
            .or_else(|| tags.get("com.apple.quicktime.creationdate"))
            .and_then(|v| v.as_str())?;

        let clean = creation_time.trim().replace('Z', "+00:00");
        if let Ok(dt) = DateTime::parse_from_rfc3339(&clean) {
            return Some((dt.timestamp(), "video".to_string()));
        }
        if let Ok(naive) = NaiveDateTime::parse_from_str(&clean[..19.min(clean.len())], "%Y-%m-%d %H:%M:%S") {
            let dt = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
            return Some((dt.timestamp(), "video".to_string()));
        }

        None
    }

    pub fn ler(&mut self, caminho: &Path) -> DataLida {
        let sem_sidecar = |par: (i64, String)| DataLida {
            ts: par.0,
            fonte: par.1,
            sidecar: None,
        };

        if eh_video(caminho) {
            if let Some(res) = self.do_sidecar(caminho) {
                return res;
            }
            if let Some(res) = Self::do_container(caminho) {
                return sem_sidecar(res);
            }
            if let Some(res) = Self::do_exif(caminho) {
                return sem_sidecar(res);
            }
        } else {
            if let Some(res) = self.do_sidecar(caminho) {
                return res;
            }
            if let Some(res) = Self::do_exif(caminho) {
                return sem_sidecar(res);
            }
            if let Some(res) = Self::do_container(caminho) {
                return sem_sidecar(res);
            }
        }

        let mtime = fs::metadata(caminho)
            .and_then(|m| m.modified())
            .map(|t| {
                let duration = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                duration.as_secs() as i64
            })
            .unwrap_or(0);

        DataLida {
            ts: mtime,
            fonte: "mtime".to_string(),
            sidecar: None,
        }
    }
}
