//! Varre uma pasta e imprime o que a triagem enxergaria, sem abrir a janela
//! nem mover nada. Serve para conferir um Takeout de verdade antes de confiar
//! a galeria a ele.
//!
//!     cargo run --example varrer -- "/caminho/Google Photos" [destino]

use fototriagem_lib::library::carregar_biblioteca;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let origem = match args.next() {
        Some(a) => PathBuf::from(a),
        None => {
            eprintln!("uso: varrer <pasta de origem> [pasta de destino]");
            std::process::exit(2);
        }
    };
    if !origem.is_dir() {
        eprintln!("pasta inexistente: {}", origem.display());
        std::process::exit(1);
    }
    // Sem destino explicito nada e escrito fora de um diretorio temporario:
    // o cache e o relatorio de copias vao para la e a origem fica intacta.
    let destino = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("fototriagem-varrer"));
    let _ = std::fs::create_dir_all(&destino);

    let scan = carregar_biblioteca(&origem, &destino, false);

    let mut por_fonte: BTreeMap<String, usize> = BTreeMap::new();
    let mut com_sidecar = 0;
    for item in &scan.itens {
        *por_fonte.entry(item.origem_data.clone()).or_default() += 1;
        if item.sidecar.is_some() {
            com_sidecar += 1;
        }
    }

    println!("origem   : {}", origem.display());
    println!("arquivos : {} varridos, {} apos deduplicar", scan.total_arquivos, scan.itens.len());
    println!("copias   : {} identicas preservadas na origem", scan.duplicatas_removidas);
    println!("sidecar  : {} de {} tem .json do Takeout", com_sidecar, scan.itens.len());
    println!("data vem de:");
    for (fonte, n) in &por_fonte {
        println!("    {:<10} {}", fonte, n);
    }

    println!("\nmeses:");
    for m in &scan.meses {
        println!("    {}/{}  {} arquivos, {} sem data real", m.nome_mes, m.ano, m.total, m.sem_data_real);
    }

    if !scan.avisos.is_empty() {
        println!("\navisos:");
        for a in &scan.avisos {
            println!("    {}", a);
        }
    }

    // Os que cairiam no mtime sao os que a triagem nao consegue datar; sao
    // eles que merecem o olho do usuario antes de qualquer movimento.
    let duvidosos: Vec<_> = scan.itens.iter().filter(|i| i.origem_data == "mtime").take(20).collect();
    if !duvidosos.is_empty() {
        println!("\nsem data confiavel (ate 20):");
        for i in &duvidosos {
            println!("    {}  {}", i.data_formatada, i.caminho);
        }
    }
}
