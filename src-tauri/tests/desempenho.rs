//! Medicao de tempo e de memoria da varredura.
//!
//! Nao sao testes de regressao: sao o instrumento com que os numeros do
//! relatorio foram tirados. Ficam marcados `#[ignore]` para nao entrarem na
//! suite normal, e rodam com:
//!
//! ```sh
//! cargo test --release --test desempenho -- --ignored --nocapture
//! ```
//!
//! O tamanho da arvore sintetica vem de `FT_BENCH_ARQUIVOS` (padrao 8000).
//! O alocador contado abaixo vale para este binario de teste inteiro, entao o
//! pico relatado e o pico do processo durante a varredura, nao uma estimativa.

use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

struct Contador;

static EM_USO: AtomicUsize = AtomicUsize::new(0);
static PICO: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Contador {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc(layout);
        if !p.is_null() {
            let agora = EM_USO.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PICO.fetch_max(agora, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        EM_USO.fetch_sub(layout.size(), Ordering::Relaxed);
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, novo: usize) -> *mut u8 {
        let p = System.realloc(ptr, layout, novo);
        if !p.is_null() {
            if novo >= layout.size() {
                let d = novo - layout.size();
                let agora = EM_USO.fetch_add(d, Ordering::Relaxed) + d;
                PICO.fetch_max(agora, Ordering::Relaxed);
            } else {
                EM_USO.fetch_sub(layout.size() - novo, Ordering::Relaxed);
            }
        }
        p
    }
}

#[global_allocator]
static ALOC: Contador = Contador;

fn zerar_pico() {
    PICO.store(EM_USO.load(Ordering::Relaxed), Ordering::Relaxed);
}

fn pico_mb() -> f64 {
    PICO.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
}

fn quantos() -> usize {
    std::env::var("FT_BENCH_ARQUIVOS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8000)
}

/// Uma arvore parecida com a de um Takeout: uma pasta de ano grande, alguns
/// albuns, sidecar para a maioria dos arquivos e um naco sem sidecar nenhum,
/// que e o caso que faz a busca cair no ramo de ultimo recurso.
fn montar_takeout(raiz: &Path, total: usize, fracao_sem_sidecar: usize) {
    let ano = raiz.join("Takeout/Google Fotos/Photos from 2023");
    fs::create_dir_all(&ano).unwrap();
    let albuns: Vec<PathBuf> = (0..5)
        .map(|i| raiz.join(format!("Takeout/Google Fotos/Viagem {}", i)))
        .collect();
    for a in &albuns {
        fs::create_dir_all(a).unwrap();
    }

    for i in 0..total {
        let pasta = if i % 7 == 0 { &albuns[i % 5] } else { &ano };
        let nome = format!("IMG_{:06}.jpg", i);
        // Tamanho unico por arquivo: fotos reais nao colidem em tamanho, e
        // colidir jogaria a medicao inteira para dentro do SHA-256.
        let corpo = vec![b'x'; 512 + i];
        fs::write(pasta.join(&nome), &corpo).unwrap();

        if i % 100 >= fracao_sem_sidecar {
            let json = format!(
                r#"{{"title":"{}","photoTakenTime":{{"timestamp":"{}"}}}}"#,
                nome,
                1_600_000_000i64 + (i as i64) * 60
            );
            fs::write(pasta.join(format!("{}.supplemental-metadata.json", nome)), json).unwrap();
        }
    }
}

fn medir(rotulo: &str, total: usize, fracao_sem_sidecar: usize) {
    let raiz = std::env::temp_dir().join(format!("ft-bench-{}-{}", rotulo, std::process::id()));
    let _ = fs::remove_dir_all(&raiz);
    let origem = raiz.join("origem");
    let destino = raiz.join("destino");
    fs::create_dir_all(&origem).unwrap();
    fs::create_dir_all(&destino).unwrap();

    montar_takeout(&origem, total, fracao_sem_sidecar);

    zerar_pico();
    let t0 = Instant::now();
    let scan = fototriagem_lib::library::carregar_biblioteca(&origem, &destino, false);
    let dt = t0.elapsed();
    let pico = pico_mb();

    println!(
        "[{}] arquivos={} sem_sidecar={}% itens={} tempo={:.3}s pico_heap={:.1}MB",
        rotulo,
        total,
        fracao_sem_sidecar,
        scan.itens.len(),
        dt.as_secs_f64(),
        pico
    );
    // O resultado tem de ser usado, senao o compilador pode encurtar o
    // trabalho que se quer medir.
    assert_eq!(scan.itens.len(), total);

    let _ = fs::remove_dir_all(&raiz);
}

#[test]
#[ignore]
fn bench_varredura_com_sidecar() {
    medir("todos-com-sidecar", quantos(), 0);
}

#[test]
#[ignore]
fn bench_varredura_10pct_sem_sidecar() {
    medir("10pct-sem-sidecar", quantos(), 10);
}

#[test]
#[ignore]
fn bench_varredura_metade_sem_sidecar() {
    medir("metade-sem-sidecar", quantos(), 50);
}
