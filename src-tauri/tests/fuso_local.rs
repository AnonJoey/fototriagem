//! O que este teste cobre: o mes e a hora que a triagem mostra sao os do relogio
//! local, venha a data de onde vier. Achado com dado real em 06/11/2026: na copia
//! do Pixel 8, `Screenshot_20240407-230933.png` (23:09 de 07/04, hora local)
//! aparecia como 08/04 02:09, porque o instante era mostrado em UTC. Uma foto das
//! 23:30 de 31/03 caia em abril.
//!
//! Os testes fixam o fuso em America/Sao_Paulo (-3 h, sem horario de verao desde
//! 2019) pela variavel TZ, para nao depender do fuso de quem roda.

use chrono::{Local, NaiveDate, TimeZone};
use filetime::FileTime;
use fototriagem_lib::library::carregar_biblioteca;
use std::fs;
use std::path::PathBuf;

fn fuso() {
    std::env::set_var("TZ", "America/Sao_Paulo");
}

fn pasta(nome: &str) -> (PathBuf, PathBuf) {
    let raiz = std::env::temp_dir().join(format!("fototriagem-fuso-{}-{}", nome, std::process::id()));
    let _ = fs::remove_dir_all(&raiz);
    let origem = raiz.join("origem");
    let destino = raiz.join("destino");
    fs::create_dir_all(&origem).unwrap();
    fs::create_dir_all(&destino).unwrap();
    (origem, destino)
}

/// 31/03/2024 23:30 no relogio local, como instante.
fn instante_local() -> i64 {
    let naive = NaiveDate::from_ymd_opt(2024, 3, 31).unwrap().and_hms_opt(23, 30, 0).unwrap();
    Local.from_local_datetime(&naive).earliest().unwrap().timestamp()
}

/// JPEG minimo com APP1 Exif e DateTimeOriginal (0x9003) = "2024:03:31 23:30:00".
fn jpeg_com_exif() -> Vec<u8> {
    let mut tiff: Vec<u8> = Vec::new();
    tiff.extend_from_slice(b"II\x2a\x00\x08\x00\x00\x00");
    // IFD0: uma entrada, ponteiro para o IFD Exif em 26
    tiff.extend_from_slice(&1u16.to_le_bytes());
    tiff.extend_from_slice(&0x8769u16.to_le_bytes());
    tiff.extend_from_slice(&4u16.to_le_bytes());
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&26u32.to_le_bytes());
    tiff.extend_from_slice(&0u32.to_le_bytes());
    // IFD Exif em 26: DateTimeOriginal, ASCII, 20 bytes em 44
    tiff.extend_from_slice(&1u16.to_le_bytes());
    tiff.extend_from_slice(&0x9003u16.to_le_bytes());
    tiff.extend_from_slice(&2u16.to_le_bytes());
    tiff.extend_from_slice(&20u32.to_le_bytes());
    tiff.extend_from_slice(&44u32.to_le_bytes());
    tiff.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(tiff.len(), 44);
    tiff.extend_from_slice(b"2024:03:31 23:30:00\0");
    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend_from_slice(&tiff);
    let mut j = vec![0xFF, 0xD8, 0xFF, 0xE1];
    j.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
    j.extend_from_slice(&app1);
    j.extend_from_slice(&[0xFF, 0xD9]);
    j
}

#[test]
fn data_do_arquivo_fica_no_mes_local() {
    fuso();
    let (origem, destino) = pasta("mtime");
    let f = origem.join("captura.png");
    fs::write(&f, b"png falso").unwrap();
    filetime::set_file_mtime(&f, FileTime::from_unix_time(instante_local(), 0)).unwrap();
    let r = carregar_biblioteca(&origem, &destino, false);
    let it = &r.itens[0];
    assert_eq!(it.origem_data, "mtime");
    assert_eq!((it.ano, it.mes), (2024, 3), "23:30 de 31/03 e marco");
    assert_eq!(it.data_formatada, "31/03/2024 23:30");
}

#[test]
fn data_do_takeout_fica_no_mes_local() {
    fuso();
    let (origem, destino) = pasta("sidecar");
    fs::write(origem.join("IMG_1.jpg"), b"sem exif").unwrap();
    fs::write(
        origem.join("IMG_1.jpg.json"),
        format!(r#"{{"title":"IMG_1.jpg","photoTakenTime":{{"timestamp":"{}"}}}}"#, instante_local()),
    )
    .unwrap();
    let r = carregar_biblioteca(&origem, &destino, false);
    let it = &r.itens[0];
    assert_eq!((it.ano, it.mes), (2024, 3));
    assert_eq!(it.data_formatada, "31/03/2024 23:30");
    assert_eq!(it.timestamp, instante_local(), "o carimbo do Takeout nao muda");
}

#[test]
fn exif_e_hora_de_parede_e_vira_instante_local() {
    fuso();
    let (origem, destino) = pasta("exif");
    fs::write(origem.join("FOTO.jpg"), jpeg_com_exif()).unwrap();
    let r = carregar_biblioteca(&origem, &destino, false);
    let it = &r.itens[0];
    assert_eq!(it.origem_data, "exif", "o leitor precisa achar o DateTimeOriginal");
    assert_eq!((it.ano, it.mes), (2024, 3));
    assert_eq!(it.data_formatada, "31/03/2024 23:30");
    // 23:30 em Sao Paulo e 02:30 UTC do dia 01/04: e esse o instante que vai para o mtime ao mover.
    assert_eq!(it.timestamp, instante_local());
}

#[test]
fn cache_antigo_com_exif_em_utc_e_relido() {
    fuso();
    let (origem, destino) = pasta("cache");
    let foto = origem.join("FOTO.jpg");
    fs::write(&foto, jpeg_com_exif()).unwrap();
    let meta = fs::metadata(&foto).unwrap();
    let mtime = meta.modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    // cache no formato anterior: assinatura sem versao e o EXIF lido como UTC (3 h fora)
    let antigo = instante_local() - 3 * 3600;
    let cache = format!(
        r#"{{"{}":{{"sig":"{}:{}","ts":{},"src":"exif","sc":null}}}}"#,
        foto.to_str().unwrap(), meta.len(), mtime, antigo
    );
    fs::write(destino.join(".triagem_cache.json"), cache).unwrap();
    let r = carregar_biblioteca(&origem, &destino, false);
    assert_eq!(r.itens[0].timestamp, instante_local(), "a entrada antiga nao pode ser reaproveitada");
}
