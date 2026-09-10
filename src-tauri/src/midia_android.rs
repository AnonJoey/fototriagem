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

/// Pede ao Android que reindexe estes caminhos, os que sairam e os que
/// chegaram. Falha aqui nunca derruba a operacao de arquivo que ja aconteceu:
/// no pior caso o indice fica velho ate a proxima varredura do sistema.
#[cfg(target_os = "android")]
pub fn avisar_scanner(caminhos: &[String]) {
    if caminhos.is_empty() {
        return;
    }
    if let Err(e) = tentar_avisar(caminhos) {
        eprintln!("[fototriagem] aviso ao MediaStore falhou: {}", e);
    }
}

#[cfg(target_os = "android")]
fn tentar_avisar(caminhos: &[String]) -> Result<(), String> {
    use jni::objects::{JObject, JObjectArray, JValue};

    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let contexto = unsafe { JObject::from_raw(ctx.context().cast()) };

    let classe_string = env.find_class("java/lang/String").map_err(|e| e.to_string())?;
    let vazio = env.new_string("").map_err(|e| e.to_string())?;
    let array: JObjectArray = env
        .new_object_array(caminhos.len() as i32, &classe_string, &vazio)
        .map_err(|e| e.to_string())?;

    for (i, caminho) in caminhos.iter().enumerate() {
        let s = env.new_string(caminho).map_err(|e| e.to_string())?;
        env.set_object_array_element(&array, i as i32, &s)
            .map_err(|e| e.to_string())?;
    }

    // scanFile(Context, String[] paths, String[] mimeTypes, OnScanCompletedListener)
    // mimeTypes nulo deixa o Android deduzir pela extensao; listener nulo
    // porque nao ha nada a fazer quando termina.
    env.call_static_method(
        "android/media/MediaScannerConnection",
        "scanFile",
        "(Landroid/content/Context;[Ljava/lang/String;[Ljava/lang/String;Landroid/media/MediaScannerConnection$OnScanCompletedListener;)V",
        &[
            JValue::Object(&contexto),
            JValue::Object(&array),
            JValue::Object(&JObject::null()),
            JValue::Object(&JObject::null()),
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}
