//! O que este teste cobre: as duas pastas de descarte se escondem do indice de
//! midia do Android, e o app sabe dizer quanto elas seguram e esvazia-las sem
//! deixar um desfazer quebrado para tras.

use fototriagem_lib::actions::{GerenciadorAcoes, Movimento, NOMEDIA};
use fototriagem_lib::library::Copia;
use fototriagem_lib::{esvaziar_pasta_de_descarte, medir_descarte};
use std::fs;
use std::path::PathBuf;

struct Cenario {
    raiz: PathBuf,
    origem: PathBuf,
    destino: PathBuf,
}

impl Cenario {
    fn novo(nome: &str) -> Self {
        let raiz = std::env::temp_dir().join(format!("fototriagem-descarte-{}-{}", nome, std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        let origem = raiz.join("origem");
        let destino = raiz.join("destino");
        fs::create_dir_all(&origem).unwrap();
        fs::create_dir_all(&destino).unwrap();
        Self { raiz, origem, destino }
    }

    fn foto(&self, nome: &str, bytes: usize) -> PathBuf {
        let p = self.origem.join(nome);
        fs::write(&p, vec![b'x'; bytes]).unwrap();
        p
    }
}

impl Drop for Cenario {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.raiz);
    }
}

fn mov(caminho: &str) -> Movimento<'_> {
    Movimento { caminho, timestamp: None, sidecar: None, copias: &[] }
}

#[test]
fn o_lixo_nasce_escondido_do_indexador() {
    let c = Cenario::novo("nomedia-lixo");
    let foto = c.foto("ruim.jpg", 100);

    let mut acoes = GerenciadorAcoes::new(c.destino.clone());
    acoes.para_o_lixo(mov(foto.to_str().unwrap())).unwrap();

    let marca = c.destino.join("_lixo").join(NOMEDIA);
    assert!(
        marca.is_file(),
        "sem .nomedia o _lixo aparece na galeria e na lista de pastas do Google Fotos, \
         como uma chave pronta para subir o lixo para a nuvem"
    );
}

#[test]
fn a_quarentena_de_copias_tambem_nasce_escondida() {
    let c = Cenario::novo("nomedia-copias");
    let foto = c.foto("boa.jpg", 100);
    let album = c.raiz.join("album");
    fs::create_dir_all(&album).unwrap();
    let copia = album.join("boa.jpg");
    fs::write(&copia, vec![b'x'; 100]).unwrap();

    let copias = vec![Copia { caminho: copia.to_string_lossy().to_string(), sidecar: None }];
    let mut acoes = GerenciadorAcoes::new(c.destino.clone());
    acoes
        .mover(
            Movimento { caminho: foto.to_str().unwrap(), timestamp: None, sidecar: None, copias: &copias },
            &c.destino.join("Viagem"),
        )
        .unwrap();

    assert!(c.destino.join("_copias").join(NOMEDIA).is_file(), "a marca vai na raiz de _copias");
    // A marca esconde a arvore inteira a partir da raiz, entao a subpasta nao
    // precisa da propria, mas a copia tem que estar mesmo la dentro.
    assert!(c.destino.join("_copias").join("Viagem").join("boa.jpg").is_file());
}

#[test]
fn a_medida_do_descarte_ignora_a_propria_marca() {
    let c = Cenario::novo("medida");
    let vazio = medir_descarte("lixo", &c.destino);
    assert!(!vazio.existe && vazio.arquivos == 0 && vazio.bytes == 0);

    let mut acoes = GerenciadorAcoes::new(c.destino.clone());
    for (nome, tamanho) in [("a.jpg", 1000), ("b.jpg", 2000), ("c.jpg", 3000)] {
        let f = c.foto(nome, tamanho);
        acoes.para_o_lixo(mov(f.to_str().unwrap())).unwrap();
    }

    let info = medir_descarte("lixo", &c.destino);
    assert!(info.existe);
    assert_eq!(info.arquivos, 3, "a marca .nomedia nao e coisa do usuario e nao entra na conta");
    assert_eq!(info.bytes, 6000);
    assert!(info.formatado.contains("KB"), "formatado: {}", info.formatado);
}

#[test]
fn esvaziar_libera_o_espaco_e_leva_a_marca_junto() {
    let c = Cenario::novo("esvaziar");
    let foto = c.foto("ruim.jpg", 4096);
    let mut acoes = GerenciadorAcoes::new(c.destino.clone());
    acoes.para_o_lixo(mov(foto.to_str().unwrap())).unwrap();

    let lixo = c.destino.join("_lixo");
    assert_eq!(medir_descarte("lixo", &c.destino).bytes, 4096);

    esvaziar_pasta_de_descarte(&lixo).unwrap();

    assert!(!lixo.exists(), "a pasta some inteira, marca incluida");
    assert_eq!(medir_descarte("lixo", &c.destino).bytes, 0);
    // Esvaziar uma pasta que nao existe mais nao e erro.
    esvaziar_pasta_de_descarte(&lixo).unwrap();
}

#[test]
fn esvaziar_o_lixo_tira_da_pilha_o_desfazer_que_ficou_impossivel() {
    let c = Cenario::novo("pilha");
    let para_lixo = c.foto("ruim.jpg", 100);
    let para_pasta = c.foto("boa.jpg", 100);

    let mut acoes = GerenciadorAcoes::new(c.destino.clone());
    acoes.para_o_lixo(mov(para_lixo.to_str().unwrap())).unwrap();
    acoes.mover(mov(para_pasta.to_str().unwrap()), &c.destino.join("Viagem")).unwrap();
    assert_eq!(acoes.pilha.len(), 2);

    let lixo = c.destino.join("_lixo");
    esvaziar_pasta_de_descarte(&lixo).unwrap();
    let perdidos = acoes.descartar_pilha_sob(&lixo);

    assert_eq!(perdidos, 1, "so a decisao que apontava para dentro do lixo sai da pilha");
    assert_eq!(acoes.pilha.len(), 1, "a que foi para a pasta de destino continua desfazivel");

    // E o desfazer que sobrou funciona de verdade, em vez de dar erro.
    let restaurado = acoes.desfazer().unwrap().expect("ainda ha o que desfazer");
    assert_eq!(restaurado.caminho_restaurado, para_pasta.to_string_lossy());
    assert!(para_pasta.is_file());
}

#[test]
fn esvaziar_a_quarentena_tira_da_pilha_a_decisao_que_tinha_copia_la() {
    let c = Cenario::novo("pilha-copias");
    let foto = c.foto("boa.jpg", 100);
    let album = c.raiz.join("album");
    fs::create_dir_all(&album).unwrap();
    let copia = album.join("boa.jpg");
    fs::write(&copia, vec![b'x'; 100]).unwrap();

    let copias = vec![Copia { caminho: copia.to_string_lossy().to_string(), sidecar: None }];
    let mut acoes = GerenciadorAcoes::new(c.destino.clone());
    acoes
        .mover(
            Movimento { caminho: foto.to_str().unwrap(), timestamp: None, sidecar: None, copias: &copias },
            &c.destino.join("Viagem"),
        )
        .unwrap();

    // A midia esta na pasta de destino, mas a copia esta na quarentena: apagar
    // a quarentena torna esse desfazer impossivel de completar.
    let quarentena = c.destino.join("_copias");
    esvaziar_pasta_de_descarte(&quarentena).unwrap();
    let perdidos = acoes.descartar_pilha_sob(&quarentena);

    assert_eq!(perdidos, 1, "a decisao sai da pilha por causa da copia, mesmo com a midia intacta");
    assert!(acoes.pilha.is_empty());
    assert!(matches!(acoes.desfazer(), Ok(None)), "e o desfazer admite que nao ha o que fazer");
}
