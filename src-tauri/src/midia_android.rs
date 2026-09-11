//! Aviso ao indice de midia do Android.
//!
//! O app move arquivo com `fs::rename`, e o Android nao percebe: o indexador
//! de midia so sabe do que passa por ele. Sem este aviso acontecem duas coisas,
//! e as duas sao ruins.
//!
//! A foto continua indexada no **caminho antigo**, entao a galeria mostra um
//! fantasma. E o **caminho novo nao existe** para o sistema, entao a pasta que
//! o app organizou pode nem aparecer na lista de "pastas do dispositivo" do
//! Google Fotos, e o que cair la nunca sobe.
//!
//! E o elo de que o fluxo inteiro depende: sem ele, "triar no app e deixar o
//! Google Fotos subir os escolhidos" nao funciona de forma confiavel.
//!
//! # Como funciona hoje
//!
//! Um plugin Tauri de Android: a classe Kotlin `MidiaPlugin` chama
//! `MediaScannerConnection.scanFile`, e o Rust fala com ela por
//! `register_android_plugin` + `run_mobile_plugin_async`. A fonte de verdade
//! do Kotlin esta em `src-tauri/android/MidiaPlugin.kt`, no git, e
//! `scripts/android_plugin_midia.py` copia para dentro de `gen/android/`
//! depois de cada `tauri android init`.
//!
//! # A tentativa anterior, e por que ela morreu
//!
//! A primeira versao usava `ndk_context::android_context()` para pegar a
//! JavaVM e o Context, e em 10/09/2026, num Pixel 8 com Android 17, ela
//! derrubou o app no primeiro movimento:
//!
//! ```text
//! panicked at ndk-context-0.1.1/src/lib.rs:72: android context was not initialized
//! core::panicking::panic_cannot_unwind  ->  SIGABRT
//! ```
//!
//! A causa era que **nada inicializa o `ndk_context` neste app**: `cargo tree
//! -i ndk-context` mostra que o tao e o wry nao dependem dele, entao o
//! estatico que ele consulta nunca era preenchido. E como a chamada acontecia
//! dentro de um comando do Tauri, que roda atravessando a fronteira JNI, o
//! panic nao podia desenrolar a pilha e virava `abort`.
//!
//! O plugin conserta a raiz do problema: o `Context` chega pronto, das maos do
//! Tauri, em vez de ser adivinhado a partir do Rust.
//!
//! # As tres regras que mantem isto longe do SIGABRT
//!
//! 1. **Falha de registro nao derruba o app.** Se a classe Kotlin nao estiver
//!    no APK (alguem rodou `tauri android init` e esqueceu o script), o
//!    `setup` registra o erro e devolve `Ok`. O app sobe com o aviso inerte,
//!    que e exatamente o comportamento de antes desta mudanca.
//! 2. **Nunca bloquear a thread principal.** `run_mobile_plugin` (a versao
//!    sincrona) espera a resposta com `Receiver::recv`, e a resposta so chega
//!    quando o looper principal do Android processa a mensagem. Os comandos
//!    sincronos do Tauri rodam na thread principal, entao chamar a versao
//!    sincrona de dentro de `move_item` travaria o app para sempre. Por isso
//!    o trabalho vai para a runtime assincrona do Tauri e usa
//!    `run_mobile_plugin_async`.
//! 3. **Erro e coisa de log, nao de `unwrap`.** Nenhum caminho aqui entra em
//!    panico: o pior caso e uma linha no logcat.

/// Fora do Android nao ha indexador para avisar, e o aviso vira nada.
#[cfg(not(target_os = "android"))]
pub fn avisar_scanner(_caminhos: &[String]) {}

/// No desktop o plugin existe so para o `lib.rs` poder registra-lo sem `cfg`.
/// Ele nao tem comando nenhum e nao faz nada.
#[cfg(not(target_os = "android"))]
pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::<tauri::Wry, ()>::new(NOME).build()
}

/// Nome do plugin do lado do Tauri. Precisa bater com a chave que o
/// `PluginManager` do Android guarda, que e este mesmo nome.
pub const NOME: &str = "midia";

/// Pacote Kotlin onde a classe vive. Precisa bater, letra por letra, com a
/// linha `package` de `src-tauri/android/MidiaPlugin.kt`: o Tauri acha a
/// classe por `Class.forName`, e um erro de digitacao aqui so aparece no
/// aparelho. `tests/test_android_plugin_midia.py` compara os dois.
pub const PACOTE_KOTLIN: &str = "com.anonjoey.fototriagem.midia";

/// Nome da classe Kotlin, pelo mesmo motivo.
pub const CLASSE_KOTLIN: &str = "MidiaPlugin";

#[cfg(target_os = "android")]
mod ponte {
    use super::{CLASSE_KOTLIN, NOME, PACOTE_KOTLIN};
    use serde::{Deserialize, Serialize};
    use std::sync::OnceLock;
    use tauri::{
        plugin::{Builder, PluginHandle, TauriPlugin},
        Wry,
    };

    /// Quantos caminhos vao por chamada. Esvaziar o descarte pode gerar
    /// milhares, e tudo isso viraria uma unica string JSON atravessando o JNI.
    /// Em lotes, cada travessia tem tamanho previsivel.
    const TAMANHO_DO_LOTE: usize = 512;

    /// Preenchido uma vez, no `setup`. Fica vazio se o registro falhar, e
    /// `avisar_scanner` vira silencio em vez de erro.
    static PONTE: OnceLock<PluginHandle<Wry>> = OnceLock::new();

    #[derive(Serialize)]
    struct PedidoScan {
        caminhos: Vec<String>,
    }

    #[derive(Deserialize)]
    struct RespostaScan {
        enviados: usize,
    }

    pub fn init() -> TauriPlugin<Wry> {
        Builder::<Wry, ()>::new(NOME)
            .setup(|_app, api| {
                match api.register_android_plugin(PACOTE_KOTLIN, CLASSE_KOTLIN) {
                    Ok(handle) => {
                        let _ = PONTE.set(handle);
                        println!("[midia] plugin registrado: {PACOTE_KOTLIN}.{CLASSE_KOTLIN}");
                    }
                    Err(e) => {
                        // Devolver Err aqui abortaria a subida do app. O aviso
                        // ao MediaStore e importante, mas nao vale o app.
                        eprintln!(
                            "[midia] registro falhou ({e}). O aviso ao MediaStore fica desligado. \
                             Rode: python3 scripts/android_permissoes.py"
                        );
                    }
                }
                Ok(())
            })
            .build()
    }

    /// Manda os caminhos para o `MediaScannerConnection`.
    ///
    /// Nao espera resposta e nao bloqueia quem chamou: o trabalho vai inteiro
    /// para a runtime assincrona. Ver a regra 2 no topo do modulo para o
    /// porque disso nao ser negociavel.
    pub fn avisar_scanner(caminhos: &[String]) {
        let Some(ponte) = PONTE.get() else {
            return;
        };

        let mut limpos: Vec<String> = caminhos
            .iter()
            .filter(|c| !c.trim().is_empty())
            .cloned()
            .collect();
        limpos.sort();
        limpos.dedup();
        if limpos.is_empty() {
            return;
        }

        let ponte = ponte.clone();
        tauri::async_runtime::spawn(async move {
            for lote in limpos.chunks(TAMANHO_DO_LOTE) {
                let pedido = PedidoScan {
                    caminhos: lote.to_vec(),
                };
                match ponte
                    .run_mobile_plugin_async::<RespostaScan>("scanFile", pedido)
                    .await
                {
                    Ok(r) => println!("[midia] lote aceito pelo scanner: {} caminho(s)", r.enviados),
                    Err(e) => eprintln!("[midia] scanFile falhou: {e}"),
                }
            }
        });
    }
}

#[cfg(target_os = "android")]
pub use ponte::{avisar_scanner, init};

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::*;

    #[test]
    fn fallback_desktop_executa_sem_panico() {
        let caminhos = vec![
            "/caminho/teste/foto1.jpg".to_string(),
            "/caminho/teste/foto2.jpg".to_string(),
        ];
        avisar_scanner(&caminhos);
        avisar_scanner(&[]);
    }

    #[test]
    fn init_desktop_cria_plugin_sem_panico() {
        let _ = init();
    }
}
