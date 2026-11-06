//! O que este teste cobre: o cliente do Immich contra um servidor HTTP falso que
//! responde no formato do Immich v3.3.6, conferido no codigo da imagem
//! `ghcr.io/immich-app/immich-server:release` em 06/11/2026:
//! - `assets.nextPage` e texto ("2") ou null, nunca numero;
//! - a busca aceita `size` de 1 a 1000 e `visibility`, e nao tem `isArchived`;
//! - `POST /api/trash/restore` restaura a lixeira INTEIRA; um item so volta por
//!   `POST /api/trash/restore/assets`.
//! O dublê recusa o que o servidor real recusaria (numero em nextPage nao existe,
//! `size` acima de 1000 vira 400), para nao deixar verde um caminho que falharia.

use fototriagem_lib::immich::{
    data_local, immich_restore_item, immich_scan_library, payload_pagina, proxima_pagina,
    url_restaurar, ImmichSearchResponse, TAMANHO_PAGINA,
};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone)]
struct Pedido {
    metodo: String,
    caminho: String,
    corpo: String,
}

fn ler_pedido(stream: &mut TcpStream) -> Option<Pedido> {
    let mut leitor = BufReader::new(stream.try_clone().ok()?);
    let mut linha = String::new();
    leitor.read_line(&mut linha).ok()?;
    let mut partes = linha.split_whitespace();
    let metodo = partes.next()?.to_string();
    let caminho = partes.next()?.to_string();
    let mut tamanho = 0usize;
    loop {
        let mut h = String::new();
        leitor.read_line(&mut h).ok()?;
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
            tamanho = v.trim().parse().unwrap_or(0);
        }
    }
    let mut corpo = vec![0u8; tamanho];
    leitor.read_exact(&mut corpo).ok()?;
    Some(Pedido { metodo, caminho, corpo: String::from_utf8_lossy(&corpo).to_string() })
}

fn responder(stream: &mut TcpStream, status: u16, corpo: &str) {
    let texto = if status == 200 { "OK" } else { "Bad Request" };
    let resp = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status, texto, corpo.len(), corpo
    );
    let _ = stream.write_all(resp.as_bytes());
}

fn asset(i: usize, ldt: &str) -> String {
    format!(
        r#"{{"id":"00000000-0000-4000-8000-{:012}","originalFileName":"IMG_{:05}.jpg","originalPath":"/mnt/gdrive-fotos/_entrada/IMG_{:05}.jpg","fileCreatedAt":"2024-04-01T02:30:00.000Z","fileModifiedAt":"2024-04-01T02:30:00.000Z","localDateTime":"{}","type":"IMAGE","isFavorite":false,"isArchived":false,"isTrashed":false,"visibility":"timeline"}}"#,
        i, i, i, ldt
    )
}

/// Servidor falso: 1503 fotos em duas paginas (1000 + 503), nextPage como texto.
fn subir_servidor(total: usize) -> (String, Arc<Mutex<Vec<Pedido>>>) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", ouvinte.local_addr().unwrap());
    let pedidos = Arc::new(Mutex::new(Vec::new()));
    let reg = pedidos.clone();
    thread::spawn(move || {
        for conexao in ouvinte.incoming() {
            let Ok(mut s) = conexao else { continue };
            let Some(p) = ler_pedido(&mut s) else { continue };
            reg.lock().unwrap().push(p.clone());
            if p.metodo == "POST" && p.caminho == "/api/search/metadata" {
                let v: serde_json::Value = serde_json::from_str(&p.corpo).unwrap();
                let size = v["size"].as_u64().unwrap_or(250) as usize;
                if !(1..=1000).contains(&size) {
                    responder(&mut s, 400, r#"{"message":"size must be <= 1000"}"#);
                    continue;
                }
                let page = v["page"].as_u64().unwrap_or(1) as usize;
                let ini = (page - 1) * size;
                let fim = (ini + size).min(total);
                let itens: Vec<String> = (ini..fim)
                    .map(|i| asset(i, "2024-03-31T23:30:00.000Z"))
                    .collect();
                let next = if fim < total { format!("\"{}\"", page + 1) } else { "null".into() };
                let corpo = format!(
                    r#"{{"albums":{{"total":0,"count":0,"items":[],"facets":[],"nextPage":null,"nextCursor":null}},"assets":{{"total":{},"count":{},"items":[{}],"facets":[],"nextPage":{},"nextCursor":null}}}}"#,
                    itens.len(), itens.len(), itens.join(","), next
                );
                responder(&mut s, 200, &corpo);
            } else if p.metodo == "POST" && p.caminho.starts_with("/api/trash/restore") {
                responder(&mut s, 200, "");
            } else {
                responder(&mut s, 400, r#"{"message":"rota desconhecida no dublê"}"#);
            }
        }
    });
    (base, pedidos)
}

#[test]
fn next_page_texto_e_desserializado_e_seguido() {
    let json = format!(
        r#"{{"assets":{{"total":1,"count":1,"items":[{}],"facets":[],"nextPage":"2","nextCursor":null}}}}"#,
        asset(1, "2024-01-01T10:00:00.000Z")
    );
    let r: ImmichSearchResponse = serde_json::from_str(&json).expect("formato real do Immich v3");
    let c = r.assets.unwrap();
    assert_eq!(proxima_pagina(&c, 1), Some(2));

    let fim = r#"{"assets":{"total":0,"count":0,"items":[],"facets":[],"nextPage":null,"nextCursor":null}}"#;
    let r: ImmichSearchResponse = serde_json::from_str(fim).unwrap();
    assert_eq!(proxima_pagina(&r.assets.unwrap(), 5), None);
}

#[test]
fn next_page_que_nao_avanca_nao_vira_laco() {
    let json = r#"{"assets":{"items":[],"nextPage":"3"}}"#;
    let r: ImmichSearchResponse = serde_json::from_str(json).unwrap();
    assert_eq!(proxima_pagina(&r.assets.unwrap(), 3), None);
}

#[test]
fn varredura_pega_as_duas_paginas() {
    let (base, pedidos) = subir_servidor(1503);
    let r = immich_scan_library(&base, "chave-de-teste").expect("varredura");
    assert_eq!(r.total_arquivos, 1503, "precisa ler as duas paginas");
    let buscas: Vec<_> = pedidos
        .lock()
        .unwrap()
        .iter()
        .filter(|p| p.caminho == "/api/search/metadata")
        .cloned()
        .collect();
    assert_eq!(buscas.len(), 2);
    let corpo: serde_json::Value = serde_json::from_str(&buscas[0].corpo).unwrap();
    assert!(corpo.get("isArchived").is_none(), "isArchived nao existe no pedido do v3");
    assert_eq!(corpo["visibility"], "timeline");
}

#[test]
fn foto_da_noite_de_31_fica_em_marco() {
    let (base, _p) = subir_servidor(3);
    let r = immich_scan_library(&base, "chave-de-teste").unwrap();
    assert_eq!(r.meses.len(), 1);
    assert_eq!((r.meses[0].ano, r.meses[0].mes), (2024, 3), "localDateTime 31/03 23:30 e marco");
    assert_eq!(r.itens[0].data_formatada, "31/03/2024 23:30");
}

#[test]
fn data_local_aceita_com_e_sem_fracao() {
    let a = data_local("2024-03-31T23:30:00.000Z").unwrap();
    let b = data_local("2024-03-31T23:30:00Z").unwrap();
    assert_eq!(a, b);
    assert!(data_local("lixo").is_none());
}

#[test]
fn payload_respeita_os_limites_do_servidor() {
    let p = payload_pagina(7);
    assert_eq!(p["page"], 7);
    let size = p["size"].as_u64().unwrap() as usize;
    assert!((1..=1000).contains(&size));
    assert_eq!(size, TAMANHO_PAGINA);
}

#[test]
fn restaurar_um_item_nao_restaura_a_lixeira_inteira() {
    assert!(url_restaurar("http://x/").ends_with("/api/trash/restore/assets"));
    let (base, pedidos) = subir_servidor(0);
    immich_restore_item(&base, "chave-de-teste", "00000000-0000-4000-8000-000000000001").unwrap();
    let p = pedidos.lock().unwrap();
    let restauros: Vec<_> = p.iter().filter(|x| x.caminho.starts_with("/api/trash")).collect();
    assert_eq!(restauros.len(), 1);
    assert_eq!(restauros[0].caminho, "/api/trash/restore/assets");
    assert!(restauros[0].corpo.contains("00000000-0000-4000-8000-000000000001"));
}
