//! O que este teste cobre: o cliente do Drive (`gdrive.rs`) contra um Drive falso
//! em memoria, com os formatos conferidos no trafego real do rclone em 06/11/2026.
//! O dublê e estrito onde o Drive real e: sem `Authorization: Bearer` responde 401,
//! `PATCH` sem `addParents` e `removeParents` responde 400, `removeParents` que
//! nao e pai do arquivo responde 400, e a listagem pode voltar com menos itens que
//! `pageSize` e um `nextPageToken` (o Drive faz isso).

use fototriagem_lib::gdrive::{consulta_pastas, escapar_q, ClienteDrive, MIME_PASTA};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone)]
struct Pedido {
    metodo: String,
    caminho: String,
    query: HashMap<String, String>,
    auth: String,
    corpo: String,
}

fn decodificar(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let h = u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap();
                out.push(h);
                i += 2;
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8(out).unwrap()
}

fn ler_pedido(stream: &mut TcpStream) -> Option<Pedido> {
    let mut leitor = BufReader::new(stream.try_clone().ok()?);
    let mut linha = String::new();
    leitor.read_line(&mut linha).ok()?;
    let mut partes = linha.split_whitespace();
    let metodo = partes.next()?.to_string();
    let alvo = partes.next()?.to_string();
    let (caminho, qs) = alvo.split_once('?').unwrap_or((alvo.as_str(), ""));
    let query = qs
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decodificar(k), decodificar(v))
        })
        .collect();
    let mut tamanho = 0usize;
    let mut auth = String::new();
    loop {
        let mut h = String::new();
        leitor.read_line(&mut h).ok()?;
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        let low = h.to_ascii_lowercase();
        if let Some(v) = low.strip_prefix("content-length:") {
            tamanho = v.trim().parse().unwrap_or(0);
        }
        if low.starts_with("authorization:") {
            auth = h["authorization:".len()..].trim().to_string();
        }
    }
    let mut corpo = vec![0u8; tamanho];
    leitor.read_exact(&mut corpo).ok()?;
    Some(Pedido { metodo, caminho: caminho.to_string(), query, auth, corpo: String::from_utf8_lossy(&corpo).to_string() })
}

fn responder(stream: &mut TcpStream, status: u16, corpo: &str) {
    let resp = format!(
        "HTTP/1.1 {} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status, corpo.len(), corpo
    );
    let _ = stream.write_all(resp.as_bytes());
}

#[derive(Debug, Clone)]
struct Arquivo {
    nome: String,
    mime: String,
    pais: Vec<String>,
}

struct DriveFalso {
    arquivos: HashMap<String, Arquivo>,
    proximo: usize,
}

/// Extrai de uma consulta `q` o pai e o nome, desfazendo o escape do Drive.
fn ler_q(q: &str) -> (Option<String>, Option<String>, bool) {
    fn valor_entre_aspas(s: &str) -> String {
        let mut out = String::new();
        let mut it = s.chars();
        while let Some(c) = it.next() {
            match c {
                '\\' => out.push(it.next().unwrap()),
                '\'' => break,
                c => out.push(c),
            }
        }
        out
    }
    let pai = q.find("('").map(|i| valor_entre_aspas(&q[i + 2..]));
    let nome = q.find("name='").map(|i| valor_entre_aspas(&q[i + 6..]));
    (pai, nome, q.contains(MIME_PASTA))
}

fn json_item(id: &str, a: &Arquivo) -> String {
    serde_json::json!({"id": id, "name": a.nome, "mimeType": a.mime, "parents": a.pais}).to_string()
}

fn subir_drive(pastas_iniciais: &[(&str, &str, &str)], arquivos: &[(&str, &str, &str)]) -> (String, Arc<Mutex<Vec<Pedido>>>, Arc<Mutex<DriveFalso>>) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", ouvinte.local_addr().unwrap());
    let pedidos = Arc::new(Mutex::new(Vec::new()));
    let mut mapa = HashMap::new();
    for (id, nome, pai) in pastas_iniciais {
        mapa.insert(id.to_string(), Arquivo { nome: nome.to_string(), mime: MIME_PASTA.into(), pais: vec![pai.to_string()] });
    }
    for (id, nome, pai) in arquivos {
        mapa.insert(id.to_string(), Arquivo { nome: nome.to_string(), mime: "image/jpeg".into(), pais: vec![pai.to_string()] });
    }
    let drive = Arc::new(Mutex::new(DriveFalso { arquivos: mapa, proximo: 100 }));
    let (reg, d) = (pedidos.clone(), drive.clone());
    thread::spawn(move || {
        for conexao in ouvinte.incoming() {
            let Ok(mut s) = conexao else { continue };
            let Some(p) = ler_pedido(&mut s) else { continue };
            reg.lock().unwrap().push(p.clone());
            if p.auth != "Bearer token-de-teste" {
                responder(&mut s, 401, r#"{"error":{"code":401}}"#);
                continue;
            }
            let mut dr = d.lock().unwrap();
            if p.metodo == "GET" && p.caminho == "/drive/v3/files/root" {
                responder(&mut s, 200, r#"{"id":"raiz"}"#);
            } else if p.metodo == "GET" && p.caminho == "/drive/v3/files" {
                let (pai, nome, so_pastas) = ler_q(p.query.get("q").map(|s| s.as_str()).unwrap_or(""));
                let mut achados: Vec<(String, Arquivo)> = dr
                    .arquivos
                    .iter()
                    .filter(|(_, a)| pai.as_ref().map_or(true, |x| a.pais.contains(x)))
                    .filter(|(_, a)| nome.as_ref().map_or(true, |n| &a.nome == n))
                    .filter(|(_, a)| !so_pastas || a.mime == MIME_PASTA)
                    .map(|(k, a)| (k.clone(), a.clone()))
                    .collect();
                achados.sort_by(|a, b| a.0.cmp(&b.0));
                // O Drive pode devolver menos que pageSize: aqui, no maximo 2 por pagina.
                let ini: usize = p.query.get("pageToken").and_then(|t| t.parse().ok()).unwrap_or(0);
                let fim = (ini + 2).min(achados.len());
                let itens: Vec<String> = achados[ini..fim].iter().map(|(k, a)| json_item(k, a)).collect();
                let token = if fim < achados.len() { format!(r#","nextPageToken":"{}""#, fim) } else { String::new() };
                responder(&mut s, 200, &format!(r#"{{"files":[{}]{}}}"#, itens.join(","), token));
            } else if p.metodo == "POST" && p.caminho == "/drive/v3/files" {
                let v: serde_json::Value = serde_json::from_str(&p.corpo).unwrap_or_default();
                let (Some(nome), Some(pai)) = (v["name"].as_str(), v["parents"][0].as_str()) else {
                    responder(&mut s, 400, r#"{"error":{"code":400}}"#);
                    continue;
                };
                if v["mimeType"] != MIME_PASTA {
                    responder(&mut s, 400, r#"{"error":{"code":400,"message":"so pasta neste teste"}}"#);
                    continue;
                }
                dr.proximo += 1;
                let id = format!("p{}", dr.proximo);
                let a = Arquivo { nome: nome.into(), mime: MIME_PASTA.into(), pais: vec![pai.into()] };
                let corpo = json_item(&id, &a);
                dr.arquivos.insert(id, a);
                responder(&mut s, 200, &corpo);
            } else if p.metodo == "PATCH" && p.caminho.starts_with("/drive/v3/files/") {
                let id = p.caminho.trim_start_matches("/drive/v3/files/").to_string();
                let (Some(add), Some(rem)) = (p.query.get("addParents"), p.query.get("removeParents")) else {
                    responder(&mut s, 400, r#"{"error":{"code":400,"message":"faltou addParents/removeParents"}}"#);
                    continue;
                };
                let Some(a) = dr.arquivos.get_mut(&id) else {
                    responder(&mut s, 404, r#"{"error":{"code":404}}"#);
                    continue;
                };
                if !a.pais.contains(rem) {
                    responder(&mut s, 400, r#"{"error":{"code":400,"message":"removeParents nao e pai"}}"#);
                    continue;
                }
                a.pais.retain(|x| x != rem);
                a.pais.push(add.clone());
                let corpo = json_item(&id, a);
                responder(&mut s, 200, &corpo);
            } else {
                responder(&mut s, 404, r#"{"error":{"code":404}}"#);
            }
        }
    });
    (base, pedidos, drive)
}

#[test]
fn escape_da_consulta() {
    assert_eq!(escapar_q("Viagem d'Oeste"), "Viagem d\\'Oeste");
    assert_eq!(escapar_q("a\\b"), "a\\\\b");
    let q = consulta_pastas("raiz", Some("O'Brien"));
    assert!(q.starts_with("trashed=false and ('raiz' in parents)"));
    assert!(q.contains("(name='O\\'Brien')"));
    assert!(q.ends_with("(mimeType='application/vnd.google-apps.folder')"));
}

#[test]
fn garantir_caminho_reusa_o_que_existe_e_cria_o_que_falta() {
    let (base, pedidos, drive) = subir_drive(&[("fotos", "Fotos", "raiz")], &[]);
    let c = ClienteDrive::com_base(&base, "token-de-teste").unwrap();
    let id = c.garantir_caminho("Fotos/_entrada").unwrap();
    let d = drive.lock().unwrap();
    let criada = &d.arquivos[&id];
    assert_eq!(criada.nome, "_entrada");
    assert_eq!(criada.pais, vec!["fotos".to_string()], "_entrada dentro de Fotos, nao numa Fotos nova");
    let posts = pedidos.lock().unwrap().iter().filter(|p| p.metodo == "POST").count();
    assert_eq!(posts, 1, "Fotos ja existia; so _entrada e criada");
    drop(d);
    // segunda chamada nao cria nada
    let id2 = c.garantir_caminho("Fotos/_entrada").unwrap();
    assert_eq!(id, id2);
    assert_eq!(pedidos.lock().unwrap().iter().filter(|p| p.metodo == "POST").count(), 1);
}

#[test]
fn nome_com_aspas_e_achado() {
    let (base, _p, _d) = subir_drive(&[("v", "Viagem d'Oeste", "raiz")], &[]);
    let c = ClienteDrive::com_base(&base, "token-de-teste").unwrap();
    let achada = c.achar_pasta("raiz", "Viagem d'Oeste").unwrap();
    assert_eq!(achada.map(|p| p.id), Some("v".to_string()));
}

#[test]
fn listagem_segue_next_page_token() {
    let pastas: Vec<(String, String)> = (0..5).map(|i| (format!("f{}", i), format!("Pasta {}", i))).collect();
    let refs: Vec<(&str, &str, &str)> = pastas.iter().map(|(a, b)| (a.as_str(), b.as_str(), "fotos")).collect();
    let (base, pedidos, _d) = subir_drive(&refs, &[("x1", "IMG.jpg", "fotos")]);
    let c = ClienteDrive::com_base(&base, "token-de-teste").unwrap();
    let todas = c.listar_pastas("fotos", None).unwrap();
    assert_eq!(todas.len(), 5, "as 5 pastas, em 3 paginas, sem o arquivo de imagem");
    let gets = pedidos.lock().unwrap().iter().filter(|p| p.metodo == "GET").count();
    assert_eq!(gets, 3);
}

#[test]
fn mover_troca_o_pai_sem_baixar() {
    let (base, pedidos, drive) = subir_drive(&[("entrada", "_entrada", "fotos"), ("viagens", "Viagens", "fotos")], &[("img1", "IMG_1.jpg", "entrada")]);
    let c = ClienteDrive::com_base(&base, "token-de-teste").unwrap();
    let r = c.mover("img1", "entrada", "viagens").unwrap().unwrap();
    assert_eq!(r.parents, vec!["viagens".to_string()]);
    assert_eq!(drive.lock().unwrap().arquivos["img1"].pais, vec!["viagens".to_string()]);
    let p = pedidos.lock().unwrap();
    let patch = p.iter().find(|x| x.metodo == "PATCH").unwrap();
    assert_eq!(patch.caminho, "/drive/v3/files/img1");
    assert_eq!(patch.query["addParents"], "viagens");
    assert_eq!(patch.query["removeParents"], "entrada");
    assert!(p.iter().all(|x| !x.caminho.contains("alt=media")), "nada de download");
}

#[test]
fn mover_para_o_mesmo_pai_nao_chama_a_api() {
    let (base, pedidos, _d) = subir_drive(&[], &[("img1", "IMG_1.jpg", "entrada")]);
    let c = ClienteDrive::com_base(&base, "token-de-teste").unwrap();
    assert_eq!(c.mover("img1", "entrada", "entrada").unwrap(), None);
    assert!(pedidos.lock().unwrap().is_empty());
}

#[test]
fn erro_do_drive_vira_mensagem_e_nao_sucesso() {
    let (base, _p, _d) = subir_drive(&[], &[("img1", "IMG_1.jpg", "entrada")]);
    let c = ClienteDrive::com_base(&base, "token-de-teste").unwrap();
    let e = c.mover("img1", "outra", "viagens").unwrap_err();
    assert!(e.contains("HTTP 400"), "{}", e);
    let ruim = ClienteDrive::com_base(&base, "token-errado").unwrap();
    assert!(ruim.id_da_raiz().unwrap_err().contains("401"));
    assert!(ClienteDrive::com_base(&base, "  ").is_err());
}
