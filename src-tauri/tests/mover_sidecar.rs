//! O que este teste cobre e a promessa do `mover`: a midia chega no destino
//! com a data verdadeira gravada, o sidecar do Takeout chega junto com ela,
//! as copias identicas de outros albuns vao para a quarentena, e um Ctrl+Z
//! devolve a decisao inteira ao que era antes.

use filetime::FileTime;
use fototriagem_lib::actions::{GerenciadorAcoes, Movimento};
use fototriagem_lib::library::Copia;
use std::fs;
use std::path::{Path, PathBuf};

/// 15/06/2024 12:30:00 UTC, a data que o sidecar afirma.
const TS_FOTO: i64 = 1718454600;
/// 03/09/2026, a data que o Google carimbou ao gerar o export.
const TS_EXPORT: i64 = 1788000000;

fn mtime(p: &Path) -> i64 {
    let meta = fs::metadata(p).expect("arquivo deveria existir");
    FileTime::from_last_modification_time(&meta).unix_seconds()
}

struct Cenario {
    raiz: PathBuf,
    origem: PathBuf,
    destino: PathBuf,
}

impl Cenario {
    fn novo(nome: &str) -> Self {
        let raiz = std::env::temp_dir().join(format!("fototriagem-teste-{}-{}", nome, std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        let origem = raiz.join("Google Photos/Photos from 2024");
        let destino = raiz.join("Destino");
        fs::create_dir_all(&origem).unwrap();
        fs::create_dir_all(&destino).unwrap();
        Self { raiz, origem, destino }
    }

    /// Uma midia com a data do export e o .json do Takeout ao lado dela.
    fn com_par(&self, nome: &str) -> PathBuf {
        self.par_em(&self.origem, nome)
    }

    fn par_em(&self, pasta: &Path, nome: &str) -> PathBuf {
        fs::create_dir_all(pasta).unwrap();
        let midia = pasta.join(nome);
        fs::write(&midia, b"conteudo da foto").unwrap();
        filetime::set_file_mtime(&midia, FileTime::from_unix_time(TS_EXPORT, 0)).unwrap();
        fs::write(
            pasta.join(format!("{}.json", nome)),
            format!(r#"{{"title":"{}","photoTakenTime":{{"timestamp":"{}"}}}}"#, nome, TS_FOTO),
        )
        .unwrap();
        midia
    }

    fn album(&self, nome: &str) -> PathBuf {
        self.raiz.join("Google Photos").join(nome)
    }
}

impl Drop for Cenario {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.raiz);
    }
}

fn mov<'a>(caminho: &'a str, ts: Option<i64>, sidecar: Option<&'a str>, copias: &'a [Copia]) -> Movimento<'a> {
    Movimento { caminho, timestamp: ts, sidecar, copias }
}

#[test]
fn mover_carimba_a_data_e_leva_o_sidecar_junto() {
    let c = Cenario::novo("par");
    let midia = c.com_par("IMG_0001.jpg");
    let alvo = c.destino.join("Viagem");

    let mut acoes = GerenciadorAcoes::new(c.destino.clone());
    let novo = acoes
        .mover(mov(midia.to_str().unwrap(), Some(TS_FOTO), Some("IMG_0001.jpg.json"), &[]), &alvo)
        .expect("mover deveria funcionar");

    let novo = PathBuf::from(novo);
    assert!(novo.is_file(), "a midia deveria estar no destino");
    assert!(!midia.exists(), "a midia nao deveria continuar na origem");
    assert_eq!(mtime(&novo), TS_FOTO, "a data do sidecar deveria estar gravada no arquivo");

    let sidecar_destino = alvo.join("IMG_0001.jpg.json");
    assert!(sidecar_destino.is_file(), "o sidecar deveria ter viajado junto");
    assert!(
        !c.origem.join("IMG_0001.jpg.json").exists(),
        "o sidecar nao deveria ter ficado para tras"
    );
}

#[test]
fn desfazer_devolve_lugar_data_e_sidecar() {
    let c = Cenario::novo("desfazer");
    let midia = c.com_par("IMG_0002.jpg");
    let sidecar = c.origem.join("IMG_0002.jpg.json");
    let alvo = c.destino.join("Viagem");

    let mut acoes = GerenciadorAcoes::new(c.destino.clone());
    acoes
        .mover(mov(midia.to_str().unwrap(), Some(TS_FOTO), Some("IMG_0002.jpg.json"), &[]), &alvo)
        .unwrap();

    // Sem confirmar que o par saiu daqui, o teste passaria tambem se o
    // sidecar simplesmente nunca tivesse se movido.
    assert!(alvo.join("IMG_0002.jpg.json").is_file(), "o sidecar deveria ter ido para o destino");
    assert!(!sidecar.exists(), "e ter saido da origem");

    let item = acoes.desfazer().unwrap().expect("havia o que desfazer");

    assert_eq!(item.caminho_restaurado, midia.to_string_lossy());
    assert!(midia.is_file(), "a midia deveria ter voltado para a origem");
    assert!(sidecar.is_file(), "o sidecar deveria ter voltado com o nome original");
    assert!(!alvo.join("IMG_0002.jpg").exists(), "nada deveria sobrar no destino");
    assert!(!alvo.join("IMG_0002.jpg.json").exists(), "nem o sidecar");
    assert_eq!(
        mtime(&midia),
        TS_EXPORT,
        "desfazer deveria devolver tambem a data que o arquivo tinha antes do carimbo"
    );
}

#[test]
fn colisao_no_destino_mantem_a_midia_e_o_sidecar_pareados() {
    let c = Cenario::novo("colisao");
    let midia = c.com_par("IMG_0003.jpg");
    let alvo = c.destino.join("Viagem");
    fs::create_dir_all(&alvo).unwrap();
    fs::write(alvo.join("IMG_0003.jpg"), b"outra foto, mesmo nome").unwrap();

    let mut acoes = GerenciadorAcoes::new(c.destino.clone());
    let novo = PathBuf::from(
        acoes
            .mover(mov(midia.to_str().unwrap(), Some(TS_FOTO), Some("IMG_0003.jpg.json"), &[]), &alvo)
            .unwrap(),
    );

    assert_eq!(novo.file_name().unwrap(), "IMG_0003_2.jpg", "a colisao deveria virar _2");
    assert!(
        alvo.join("IMG_0003_2.jpg.json").is_file(),
        "o sidecar deveria seguir o nome final da midia, senao o par se perde"
    );
}

#[test]
fn sem_sidecar_e_sem_data_o_arquivo_chega_intacto() {
    let c = Cenario::novo("nada");
    let midia = c.origem.join("IMG_0004.jpg");
    fs::write(&midia, b"foto sem metadado nenhum").unwrap();
    filetime::set_file_mtime(&midia, FileTime::from_unix_time(TS_EXPORT, 0)).unwrap();
    let alvo = c.destino.join("Viagem");

    let mut acoes = GerenciadorAcoes::new(c.destino.clone());
    let novo = PathBuf::from(
        acoes.mover(mov(midia.to_str().unwrap(), None, None, &[]), &alvo).unwrap(),
    );

    assert!(novo.is_file());
    assert_eq!(mtime(&novo), TS_EXPORT, "sem data de verdade, nada deveria ser carimbado");
}

#[test]
fn sidecar_declarado_mas_ausente_nao_derruba_o_movimento() {
    let c = Cenario::novo("fantasma");
    let midia = c.origem.join("IMG_0005.jpg");
    fs::write(&midia, b"foto cujo json sumiu").unwrap();
    let alvo = c.destino.join("Viagem");

    let mut acoes = GerenciadorAcoes::new(c.destino.clone());
    let novo = PathBuf::from(
        acoes
            .mover(mov(midia.to_str().unwrap(), Some(TS_FOTO), Some("IMG_0005.jpg.json"), &[]), &alvo)
            .expect("a midia deveria mover mesmo sem o sidecar prometido"),
    );

    assert!(novo.is_file());
    assert_eq!(mtime(&novo), TS_FOTO);
}

// --- copias de album ---------------------------------------------------

#[test]
fn as_copias_vao_para_a_quarentena_espelhando_a_pasta_escolhida() {
    let c = Cenario::novo("copias");
    let midia = c.com_par("IMG_0006.jpg");
    let copia_a = c.par_em(&c.album("Viagem Praia 2024"), "IMG_0006.jpg");
    let copia_b = c.par_em(&c.album("Melhores de 2024"), "IMG_0006.jpg");
    let alvo = c.destino.join("Junho");

    let copias = vec![
        Copia { caminho: copia_a.to_string_lossy().to_string(), sidecar: Some("IMG_0006.jpg.json".into()) },
        Copia { caminho: copia_b.to_string_lossy().to_string(), sidecar: Some("IMG_0006.jpg.json".into()) },
    ];

    let mut acoes = GerenciadorAcoes::new(c.destino.clone());
    acoes
        .mover(mov(midia.to_str().unwrap(), Some(TS_FOTO), Some("IMG_0006.jpg.json"), &copias), &alvo)
        .unwrap();

    let quarentena = c.destino.join("_copias").join("Junho");
    assert!(quarentena.join("IMG_0006.jpg").is_file(), "a primeira copia deveria estar parqueada");
    assert!(
        quarentena.join("IMG_0006_2.jpg").is_file(),
        "a segunda copia, de mesmo nome, deveria virar _2 em vez de sobrescrever a primeira"
    );
    assert!(!copia_a.exists() && !copia_b.exists(), "os albuns de origem deveriam ter esvaziado");
    assert_eq!(
        mtime(&quarentena.join("IMG_0006.jpg")),
        TS_FOTO,
        "a copia parqueada tambem merece a data verdadeira"
    );
    assert!(
        quarentena.join("IMG_0006.jpg.json").is_file() && quarentena.join("IMG_0006_2.jpg.json").is_file(),
        "cada copia deveria chegar com o sidecar do proprio album"
    );
    assert!(
        alvo.join("IMG_0006.jpg").is_file() && !alvo.join("IMG_0006_2.jpg").exists(),
        "a pasta escolhida deveria ficar com uma foto so"
    );
}

#[test]
fn um_ctrl_z_desfaz_a_decisao_inteira_e_nao_um_arquivo_por_vez() {
    let c = Cenario::novo("desfaz-copias");
    let midia = c.com_par("IMG_0007.jpg");
    let copia = c.par_em(&c.album("Viagem Praia 2024"), "IMG_0007.jpg");
    let alvo = c.destino.join("Junho");

    let copias = vec![Copia {
        caminho: copia.to_string_lossy().to_string(),
        sidecar: Some("IMG_0007.jpg.json".into()),
    }];

    let mut acoes = GerenciadorAcoes::new(c.destino.clone());
    acoes
        .mover(mov(midia.to_str().unwrap(), Some(TS_FOTO), Some("IMG_0007.jpg.json"), &copias), &alvo)
        .unwrap();
    assert!(!copia.exists(), "a copia deveria ter saido do album");

    acoes.desfazer().unwrap().expect("havia o que desfazer");

    assert!(midia.is_file(), "a foto deveria ter voltado");
    assert!(copia.is_file(), "e a copia tambem, no album de onde saiu");
    assert!(
        c.album("Viagem Praia 2024").join("IMG_0007.jpg.json").is_file(),
        "com o sidecar do album junto"
    );
    assert_eq!(mtime(&copia), TS_EXPORT, "a copia deveria recuperar a data que tinha");
    assert!(
        !c.destino.join("_copias").exists(),
        "a quarentena vazia nao deveria sobrar atrapalhando a conferencia"
    );
}

#[test]
fn copia_que_sumiu_do_disco_nao_impede_a_decisao() {
    let c = Cenario::novo("copia-fantasma");
    let midia = c.com_par("IMG_0008.jpg");
    let alvo = c.destino.join("Junho");

    let copias = vec![Copia {
        caminho: c.album("Album Que Nao Existe").join("IMG_0008.jpg").to_string_lossy().to_string(),
        sidecar: None,
    }];

    let mut acoes = GerenciadorAcoes::new(c.destino.clone());
    let novo = PathBuf::from(
        acoes
            .mover(mov(midia.to_str().unwrap(), Some(TS_FOTO), Some("IMG_0008.jpg.json"), &copias), &alvo)
            .expect("a foto deveria mover mesmo com uma copia inexistente na lista"),
    );

    assert!(novo.is_file());
    assert_eq!(mtime(&novo), TS_FOTO);
}
