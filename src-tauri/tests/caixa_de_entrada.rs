//! O que este teste cobre: a triagem direto no Drive montado, com a origem em
//! `Fotos/_entrada` e o destino em `Fotos` (desenho de 30/10, Epico 5). A regra de
//! sobreposicao recusava esse par; agora ele e a unica excecao, e so no formato
//! exato. Mover dali para uma pasta irma ou para `_lixo` tira a foto da fila.

use fototriagem_lib::actions::{GerenciadorAcoes, Movimento};
use fototriagem_lib::library::{carregar_biblioteca, validar_caminhos_origem_destino, ENTRADA};
use std::fs;
use std::path::PathBuf;

struct Drive {
    raiz: PathBuf,
    fotos: PathBuf,
    entrada: PathBuf,
}

impl Drive {
    fn novo(nome: &str) -> Self {
        let raiz = std::env::temp_dir().join(format!("fototriagem-entrada-{}-{}", nome, std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        let fotos = raiz.join("Fotos");
        let entrada = fotos.join(ENTRADA);
        fs::create_dir_all(&entrada).unwrap();
        Self { raiz, fotos, entrada }
    }
}

impl Drop for Drive {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.raiz);
    }
}

fn mov(caminho: &str) -> Movimento<'_> {
    Movimento { caminho, timestamp: None, sidecar: None, copias: &[] }
}

#[test]
fn entrada_dentro_do_destino_e_aceita() {
    let d = Drive::novo("aceita");
    assert!(validar_caminhos_origem_destino(&d.entrada, &d.fotos).is_ok());
}

#[test]
fn so_o_formato_exato_e_aceito() {
    let d = Drive::novo("exato");
    let funda = d.entrada.join("sub");
    let parecida = d.fotos.join("_entrada_velha");
    let outra = d.fotos.join("Camera");
    for p in [&funda, &parecida, &outra] {
        fs::create_dir_all(p).unwrap();
        assert!(validar_caminhos_origem_destino(p, &d.fotos).is_err(), "{} nao e a caixa de entrada", p.display());
    }
    // e o contrario continua recusado: destino dentro da origem
    assert!(validar_caminhos_origem_destino(&d.fotos, &d.entrada).is_err());
}

#[test]
fn foto_triada_sai_da_fila_e_nao_volta() {
    let d = Drive::novo("fila");
    fs::write(d.entrada.join("A.jpg"), b"foto a").unwrap();
    fs::write(d.entrada.join("B.jpg"), b"foto b").unwrap();
    let antes = carregar_biblioteca(&d.entrada, &d.fotos, false);
    assert_eq!(antes.itens.len(), 2);

    let mut acoes = GerenciadorAcoes::new(d.fotos.clone());
    let a = d.entrada.join("A.jpg");
    let b = d.entrada.join("B.jpg");
    acoes.mover(mov(a.to_str().unwrap()), &d.fotos.join("Viagens")).unwrap();
    acoes.para_o_lixo(mov(b.to_str().unwrap())).unwrap();

    assert!(d.fotos.join("Viagens/A.jpg").is_file(), "A foi para a pasta irma");
    assert!(d.fotos.join("_lixo/B.jpg").is_file(), "B foi para Fotos/_lixo");
    let depois = carregar_biblioteca(&d.entrada, &d.fotos, false);
    assert_eq!(depois.itens.len(), 0, "nada triado volta para a fila");
}
