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
    classificar, data_local, immich_remove_from_album, immich_restore_item, immich_scan_library,
    payload_pagina, proxima_pagina, url_restaurar, EstadoTriagem, ImmichAssetDto,
    ImmichSearchResponse, TAMANHO_PAGINA,
};
use std::collections::HashSet;
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
    asset_em(i, ldt, "_entrada")
}

fn asset_em(i: usize, ldt: &str, pasta: &str) -> String {
    format!(
        r#"{{"id":"00000000-0000-4000-8000-{:012}","originalFileName":"IMG_{:05}.jpg","originalPath":"/mnt/gdrive-fotos/{}/IMG_{:05}.jpg","fileCreatedAt":"2024-04-01T02:30:00.000Z","fileModifiedAt":"2024-04-01T02:30:00.000Z","localDateTime":"{}","type":"IMAGE","isFavorite":false,"isArchived":false,"isTrashed":false,"visibility":"timeline"}}"#,
        i, i, pasta, i, ldt
    )
}

/// Biblioteca de cada teste: para o indice i, a pasta e se esta em album.
type Biblioteca = fn(usize) -> (&'static str, bool);
fn tudo_na_entrada(_: usize) -> (&'static str, bool) {
    ("_entrada", false)
}

/// Servidor falso: 1503 fotos em duas paginas (1000 + 503), nextPage como texto.
fn subir_servidor(total: usize) -> (String, Arc<Mutex<Vec<Pedido>>>) {
    subir_servidor_com(total, tudo_na_entrada)
}

fn subir_servidor_com(total: usize, bib: Biblioteca) -> (String, Arc<Mutex<Vec<Pedido>>>) {
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
                let so_sem_album = v["isNotInAlbum"].as_bool().unwrap_or(false);
                let lista: Vec<usize> = (0..total).filter(|i| !(so_sem_album && bib(*i).1)).collect();
                let ini = ((page - 1) * size).min(lista.len());
                let fim = (ini + size).min(lista.len());
                let itens: Vec<String> = lista[ini..fim]
                    .iter()
                    .map(|i| asset_em(*i, "2024-03-31T23:30:00.000Z", bib(*i).0))
                    .collect();
                let next = if fim < lista.len() { format!("\"{}\"", page + 1) } else { "null".into() };
                let corpo = format!(
                    r#"{{"albums":{{"total":0,"count":0,"items":[],"facets":[],"nextPage":null,"nextCursor":null}},"assets":{{"total":{},"count":{},"items":[{}],"facets":[],"nextPage":{},"nextCursor":null}}}}"#,
                    itens.len(), itens.len(), itens.join(","), next
                );
                responder(&mut s, 200, &corpo);
            } else if p.metodo == "POST" && p.caminho.starts_with("/api/trash/restore") {
                responder(&mut s, 200, "");
            } else if p.metodo == "DELETE" && p.caminho.starts_with("/api/albums/") && p.caminho.ends_with("/assets") {
                responder(&mut s, 200, "[]");
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
    // duas paginas da biblioteca inteira e duas da passada "sem album"
    assert_eq!(buscas.len(), 4);
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

/// 0..20: i%4==0 em _lixo, i%4==1 numa pasta definitiva, o resto em _entrada;
/// i%5==0 esta em album.
fn misto(i: usize) -> (&'static str, bool) {
    let pasta = match i % 4 {
        0 => "_lixo",
        1 => "Viagens/2024",
        _ => "_entrada",
    };
    (pasta, i % 5 == 0)
}

#[test]
fn ja_triado_nao_volta_como_pendente() {
    let (base, _p) = subir_servidor_com(20, misto);
    let r = immich_scan_library(&base, "chave-de-teste").unwrap();
    // fora da lista: _lixo (0, 4, 8, 12, 16)
    assert_eq!(r.total_arquivos, 15);
    let pendentes: Vec<_> = r.itens.iter().filter(|i| !i.triada).map(|i| i.nome.clone()).collect();
    // pendente: em _entrada e fora de album -> 2, 3, 6, 7, 11, 14, 18, 19 (10 e 15 estao em album)
    let esperado: Vec<String> = [2, 3, 6, 7, 11, 14, 18, 19].iter().map(|i| format!("IMG_{:05}.jpg", i)).collect();
    let mut p = pendentes.clone();
    p.sort();
    assert_eq!(p, esperado);
    assert_eq!(r.meses[0].restantes, 8);
    assert_eq!(r.meses[0].total, 15);
}

#[test]
fn biblioteca_sem_entrada_continua_toda_pendente() {
    fn sem_entrada(_: usize) -> (&'static str, bool) {
        ("Camera", false)
    }
    let (base, _p) = subir_servidor_com(6, sem_entrada);
    let r = immich_scan_library(&base, "chave-de-teste").unwrap();
    assert_eq!(r.itens.iter().filter(|i| !i.triada).count(), 6);
}

#[test]
fn classificar_reconhece_segmento_e_nao_substring() {
    let a: ImmichAssetDto = serde_json::from_str(
        r#"{"id":"x","originalPath":"/mnt/gdrive-fotos/_entrada_velha/IMG.jpg"}"#,
    )
    .unwrap();
    let mut sem = HashSet::new();
    sem.insert("x".to_string());
    // "_entrada_velha" nao e "_entrada": com a biblioteca usando _entrada, isso ja foi organizado
    assert_eq!(classificar(&a, true, &sem), EstadoTriagem::Triada);
    let b: ImmichAssetDto =
        serde_json::from_str(r#"{"id":"x","originalPath":"/mnt/gdrive-fotos/_lixo/IMG.jpg"}"#).unwrap();
    assert_eq!(classificar(&b, true, &sem), EstadoTriagem::Descartada);
}

#[test]
fn desfazer_album_tira_do_album_no_servidor() {
    let (base, pedidos) = subir_servidor(0);
    immich_remove_from_album(&base, "chave-de-teste", "00000000-0000-4000-8000-000000000002", "album-1").unwrap();
    let p = pedidos.lock().unwrap();
    let d: Vec<_> = p.iter().filter(|x| x.metodo == "DELETE").collect();
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].caminho, "/api/albums/album-1/assets");
    assert!(d[0].corpo.contains("00000000-0000-4000-8000-000000000002"));
}
