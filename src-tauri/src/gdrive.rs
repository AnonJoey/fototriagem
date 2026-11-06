//! Cliente minimo da Google Drive API v3 para o GS PhotoTriage (item 12.8.3).
//!
//! Cria pastas e move arquivos no proprio Drive, sem baixar nada: mover e trocar
//! o pai do arquivo (`PATCH files/{id}?addParents=..&removeParents=..`).
//!
//! Os formatos dos pedidos foram conferidos em 06/11/2026 contra o trafego real
//! do rclone 1.75.2 com o mesmo client OAuth (projeto personal-databank):
//! - pasta raiz: `GET /drive/v3/files/root?fields=id`;
//! - procurar: `GET /drive/v3/files?q=trashed=false and ('<pai>' in parents)
//!   and (name='<nome>') and (mimeType='application/vnd.google-apps.folder')`;
//! - criar pasta: `POST /drive/v3/files` com `{"mimeType":..folder,"name":..,"parents":[..]}`;
//! - mover: `PATCH /drive/v3/files/<id>?addParents=<novo>&removeParents=<antigo>`.
//!
//! O token de acesso vem de fora (login OAuth do app, item 12.8.2). Este modulo
//! nao le, guarda nem renova credencial.

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const MIME_PASTA: &str = "application/vnd.google-apps.folder";
const CAMPOS_ITEM: &str = "id,name,mimeType,parents";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ItemDrive {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub parents: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListaDrive {
    #[serde(default)]
    files: Vec<ItemDrive>,
    #[serde(default)]
    next_page_token: Option<String>,
}

/// Escapa um valor para dentro das aspas simples de uma consulta `q` do Drive:
/// barra invertida vira `\\` e aspas simples viram `\'`.
pub fn escapar_q(valor: &str) -> String {
    valor.replace('\\', "\\\\").replace('\'', "\\'")
}

/// Consulta das pastas filhas de `pai`, opcionalmente com um nome exato.
pub fn consulta_pastas(pai: &str, nome: Option<&str>) -> String {
    let mut q = format!("trashed=false and ('{}' in parents)", escapar_q(pai));
    if let Some(n) = nome {
        q.push_str(&format!(" and (name='{}')", escapar_q(n)));
    }
    q.push_str(&format!(" and (mimeType='{}')", MIME_PASTA));
    q
}

pub struct ClienteDrive {
    base: String,
    token: String,
    http: Client,
}

impl ClienteDrive {
    pub fn novo(token: &str) -> Result<Self, String> {
        Self::com_base("https://www.googleapis.com", token)
    }

    /// `base` existe para os testes apontarem para um servidor local.
    pub fn com_base(base: &str, token: &str) -> Result<Self, String> {
        if token.trim().is_empty() {
            return Err("Sem token de acesso do Google Drive".to_string());
        }
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { base: base.trim_end_matches('/').to_string(), token: token.to_string(), http })
    }

    fn url(&self, caminho: &str) -> String {
        format!("{}/drive/v3/{}", self.base, caminho)
    }

    fn checar(r: reqwest::blocking::Response, acao: &str) -> Result<reqwest::blocking::Response, String> {
        let status = r.status();
        if status.is_success() {
            return Ok(r);
        }
        let corpo = r.text().unwrap_or_default();
        let resumo: String = corpo.chars().take(300).collect();
        Err(format!("Drive recusou {} (HTTP {}): {}", acao, status, resumo))
    }

    pub fn id_da_raiz(&self) -> Result<String, String> {
        let r = self
            .http
            .get(self.url("files/root"))
            .bearer_auth(&self.token)
            .query(&[("fields", "id")])
            .send()
            .map_err(|e| format!("Falha ao falar com o Drive: {}", e))?;
        let item: ItemDrive = Self::checar(r, "a leitura da pasta raiz")?
            .json()
            .map_err(|e| format!("Resposta inesperada do Drive: {}", e))?;
        Ok(item.id)
    }

    /// Todas as pastas filhas de `pai` (com `nome`, so as desse nome), seguindo `nextPageToken`.
    pub fn listar_pastas(&self, pai: &str, nome: Option<&str>) -> Result<Vec<ItemDrive>, String> {
        let q = consulta_pastas(pai, nome);
        let campos = format!("files({}),nextPageToken", CAMPOS_ITEM);
        let mut todas = Vec::new();
        let mut pagina: Option<String> = None;
        loop {
            let mut req = self
                .http
                .get(self.url("files"))
                .bearer_auth(&self.token)
                .query(&[
                    ("q", q.as_str()),
                    ("fields", campos.as_str()),
                    ("pageSize", "1000"),
                    ("supportsAllDrives", "true"),
                    ("includeItemsFromAllDrives", "true"),
                ]);
            if let Some(t) = &pagina {
                req = req.query(&[("pageToken", t.as_str())]);
            }
            let r = req.send().map_err(|e| format!("Falha ao falar com o Drive: {}", e))?;
            let lista: ListaDrive = Self::checar(r, "a listagem de pastas")?
                .json()
                .map_err(|e| format!("Resposta inesperada do Drive: {}", e))?;
            todas.extend(lista.files);
            match lista.next_page_token.filter(|t| !t.is_empty()) {
                Some(t) if Some(&t) != pagina.as_ref() => pagina = Some(t),
                _ => break,
            }
        }
        Ok(todas)
    }

    /// A pasta `nome` dentro de `pai`, se existir. Com nomes repetidos, a primeira.
    pub fn achar_pasta(&self, pai: &str, nome: &str) -> Result<Option<ItemDrive>, String> {
        Ok(self.listar_pastas(pai, Some(nome))?.into_iter().next())
    }

    pub fn criar_pasta(&self, pai: &str, nome: &str) -> Result<ItemDrive, String> {
        let corpo = serde_json::json!({ "mimeType": MIME_PASTA, "name": nome, "parents": [pai] });
        let r = self
            .http
            .post(self.url("files"))
            .bearer_auth(&self.token)
            .query(&[("fields", CAMPOS_ITEM), ("supportsAllDrives", "true")])
            .json(&corpo)
            .send()
            .map_err(|e| format!("Falha ao falar com o Drive: {}", e))?;
        Self::checar(r, "a criacao da pasta")?
            .json()
            .map_err(|e| format!("Resposta inesperada do Drive: {}", e))
    }

    /// Acha ou cria cada segmento de `caminho` (ex.: "Fotos/_entrada") a partir da raiz.
    /// Devolve o id da ultima pasta.
    pub fn garantir_caminho(&self, caminho: &str) -> Result<String, String> {
        let mut atual = self.id_da_raiz()?;
        for seg in caminho.split('/').map(str::trim).filter(|s| !s.is_empty()) {
            atual = match self.achar_pasta(&atual, seg)? {
                Some(p) => p.id,
                None => self.criar_pasta(&atual, seg)?.id,
            };
        }
        Ok(atual)
    }

    /// Move `arquivo_id` de `de_pai` para `para_pai` sem baixar o arquivo.
    /// Mover para o mesmo pai nao faz nada e nao chama a API.
    pub fn mover(&self, arquivo_id: &str, de_pai: &str, para_pai: &str) -> Result<Option<ItemDrive>, String> {
        if de_pai == para_pai {
            return Ok(None);
        }
        let r = self
            .http
            .patch(self.url(&format!("files/{}", arquivo_id)))
            .bearer_auth(&self.token)
            .query(&[
                ("addParents", para_pai),
                ("removeParents", de_pai),
                ("fields", CAMPOS_ITEM),
                ("supportsAllDrives", "true"),
            ])
            .json(&serde_json::json!({}))
            .send()
            .map_err(|e| format!("Falha ao falar com o Drive: {}", e))?;
        Self::checar(r, "a movimentacao do arquivo")?
            .json()
            .map(Some)
            .map_err(|e| format!("Resposta inesperada do Drive: {}", e))
    }
}
