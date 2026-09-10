//! Prova o caminho inteiro num Takeout de mentira mas com os vicios reais de
//! nomenclatura do Google: a varredura tem que descobrir qual .json pertence
//! a qual midia, e o movimento tem que levar esse .json e gravar a data dele.
//! Aqui nada e alimentado a mao: o nome do sidecar sai do proprio scanner.

use filetime::FileTime;
use fototriagem_lib::actions::{GerenciadorAcoes, Movimento};
use fototriagem_lib::library::{carregar_biblioteca, MidiaItem};
use std::fs;
use std::path::{Path, PathBuf};

const TS_EXPORT: i64 = 1788000000;

struct Takeout {
    raiz: PathBuf,
    ano: PathBuf,
    destino: PathBuf,
}

impl Takeout {
    /// `nome` separa um teste do outro: eles rodam no mesmo processo e, sem
    /// isso, um apaga a arvore do outro no meio da execucao.
    fn novo(nome: &str) -> Self {
        let raiz = std::env::temp_dir().join(format!("fototriagem-e2e-{}-{}", nome, std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        let ano = raiz.join("Google Photos/Photos from 2024");
        let destino = raiz.join("Destino");
        fs::create_dir_all(&ano).unwrap();
        fs::create_dir_all(&destino).unwrap();
        Self { raiz, ano, destino }
    }

    /// `midia` e o arquivo; `json` e o nome que o Google deu ao sidecar, que
    /// nem sempre e `midia + .json`; `titulo` e o campo title de dentro dele.
    fn par(&self, midia: &str, json: &str, titulo: &str, ts: i64) {
        fs::write(self.ano.join(midia), format!("bytes de {}", midia)).unwrap();
        filetime::set_file_mtime(&self.ano.join(midia), FileTime::from_unix_time(TS_EXPORT, 0)).unwrap();
        fs::write(
            self.ano.join(json),
            format!(r#"{{"title":"{}","photoTakenTime":{{"timestamp":"{}"}}}}"#, titulo, ts),
        )
        .unwrap();
    }

    fn origem(&self) -> PathBuf {
        self.raiz.join("Google Photos")
    }
}

impl Drop for Takeout {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.raiz);
    }
}

fn achar<'a>(itens: &'a [MidiaItem], nome: &str) -> &'a MidiaItem {
    itens
        .iter()
        .find(|i| i.nome == nome)
        .unwrap_or_else(|| panic!("{} deveria ter sido varrido; varridos: {:?}", nome, itens.iter().map(|i| &i.nome).collect::<Vec<_>>()))
}

#[test]
fn a_varredura_descobre_o_sidecar_de_cada_vicio_de_nome() {
    let t = Takeout::novo("vicios");
    // 1. par exato
    t.par("IMG_0001.jpg", "IMG_0001.jpg.json", "IMG_0001.jpg", 1718454600);
    // 2. parentese que o Google move para depois da extensao
    t.par("IMG_0002(1).jpg", "IMG_0002.jpg(1).json", "IMG_0002.jpg", 1718541000);
    // 3. copia editada, cujo json e o do original
    t.par("IMG_0003-edited.jpg", "IMG_0003.jpg.json", "IMG_0003.jpg", 1718627400);
    // 4. o sufixo supplemental-metadata dos exports recentes
    t.par("IMG_0004.jpg", "IMG_0004.jpg.supplemental-metadata.json", "algum-titulo-que-nao-casa", 1718713800);

    let scan = carregar_biblioteca(&t.origem(), &t.destino, false);

    for (nome, json_esperado, ts_esperado) in [
        ("IMG_0001.jpg", "IMG_0001.jpg.json", 1718454600),
        ("IMG_0002(1).jpg", "IMG_0002.jpg(1).json", 1718541000),
        ("IMG_0003-edited.jpg", "IMG_0003.jpg.json", 1718627400),
        ("IMG_0004.jpg", "IMG_0004.jpg.supplemental-metadata.json", 1718713800),
    ] {
        let item = achar(&scan.itens, nome);
        assert_eq!(
            item.sidecar.as_deref(),
            Some(json_esperado),
            "{}: a varredura apontou o sidecar errado",
            nome
        );
        assert_eq!(item.timestamp, ts_esperado, "{}: data errada", nome);
        assert!(item.origem_data.starts_with("takeout"), "{}: fonte {}", nome, item.origem_data);
    }
}

#[test]
fn o_que_a_varredura_achou_e_o_que_chega_no_destino() {
    let t = Takeout::novo("ate-o-destino");
    t.par("IMG_0002(1).jpg", "IMG_0002.jpg(1).json", "IMG_0002.jpg", 1718541000);

    let scan = carregar_biblioteca(&t.origem(), &t.destino, false);
    let item = achar(&scan.itens, "IMG_0002(1).jpg").clone();

    let alvo = t.destino.join("Junho");
    let mut acoes = GerenciadorAcoes::new(t.destino.clone());
    let novo = PathBuf::from(
        acoes
            .mover(
                Movimento {
                    caminho: &item.caminho,
                    timestamp: Some(item.timestamp),
                    sidecar: item.sidecar.as_deref(),
                    copias: &item.copias,
                },
                &alvo,
            )
            .unwrap(),
    );

    let meta = fs::metadata(&novo).unwrap();
    assert_eq!(
        FileTime::from_last_modification_time(&meta).unix_seconds(),
        1718541000,
        "o arquivo no destino deveria carregar a data da foto, nao a do export"
    );
    assert!(
        alvo.join("IMG_0002(1).jpg.json").is_file(),
        "o sidecar deveria chegar ao lado da midia e com o nome dela"
    );
    assert!(
        !t.ano.join("IMG_0002.jpg(1).json").exists(),
        "e nao ficar orfao na pasta do Takeout"
    );

    // O ganho concreto: relido do destino, o par agora casa pela via exata.
    let scan_destino = carregar_biblioteca(&alvo, &alvo, false);
    let relido = achar(&scan_destino.itens, "IMG_0002(1).jpg");
    assert_eq!(relido.timestamp, 1718541000, "o destino deveria continuar datavel sozinho");
}

#[test]
fn foto_sem_sidecar_nenhum_nao_ganha_um_inventado() {
    let t = Takeout::novo("orfa");
    let orfa = t.ano.join("IMG_9999.jpg");
    fs::write(&orfa, b"foto que o Takeout nao documentou").unwrap();
    filetime::set_file_mtime(&orfa, FileTime::from_unix_time(TS_EXPORT, 0)).unwrap();

    let scan = carregar_biblioteca(&t.origem(), &t.destino, false);
    let item = achar(&scan.itens, "IMG_9999.jpg");

    assert_eq!(item.sidecar, None, "nao havia .json para achar");
    assert_eq!(item.origem_data, "mtime", "a cadeia deveria admitir que caiu no mtime");
    assert!(Path::new(&item.caminho).is_file());
}
