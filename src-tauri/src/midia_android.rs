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

/// Fora do Android nao ha indexador para avisar, e o aviso vira nada.
#[cfg(not(target_os = "android"))]
pub fn avisar_scanner(_caminhos: &[String]) {}

/// DESLIGADO. Hoje isto nao faz nada, de proposito.
///
/// A primeira tentativa usava `ndk_context::android_context()` para pegar a
/// JavaVM e o Context, e em 10/09/2026, num Pixel 8 com Android 17, ela
/// derrubou o app no primeiro movimento:
///
/// ```text
/// panicked at ndk-context-0.1.1/src/lib.rs:72: android context was not initialized
/// core::panicking::panic_cannot_unwind  ->  SIGABRT
/// ```
///
/// A causa e que **nada inicializa o `ndk_context` neste app**: `cargo tree -i
/// ndk-context` mostra que o tao e o wry nao dependem dele, entao o estatico
/// que ele consulta nunca e preenchido. E como a chamada acontece dentro de um
/// comando do Tauri, que roda atravessando a fronteira JNI, o panic nao pode
/// desenrolar a pilha e vira `abort`: o processo inteiro morre.
///
/// Envolver em `catch_unwind` seria remendo e nem funcionaria no perfil de
/// release, que usa `panic = "abort"`. Entao a funcao fica inerte ate existir
/// o caminho certo, que e um plugin Tauri com uma classe Kotlin chamando
/// `MediaScannerConnection.scanFile`, registrada por
/// `api.register_android_plugin`. O plugin recebe o `Context` de verdade das
/// maos do proprio Tauri, em vez de tentar adivinha-lo a partir do Rust.
///
/// Enquanto isso o app funciona: o MediaProvider do Android observa o FUSE do
/// armazenamento compartilhado e percebe sozinho boa parte das mudancas de
/// arquivo. So que **percebe quando quer**, e e justamente essa incerteza que
/// o aviso explicito existia para eliminar.
#[cfg(target_os = "android")]
pub fn avisar_scanner(_caminhos: &[String]) {}
