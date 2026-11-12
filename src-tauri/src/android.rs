//! Android: the JNI bridge to the Media3 player in `HumPlayer.kt`. The app calls `nativeInit`
//! once from Java (so the class is found with the app's class loader); from then on the engine's
//! commands reach `HumPlayer.command(json)` from any thread, and the player's events come back
//! through `nativeEvent(json)`.

use std::sync::OnceLock;

use jni::objects::{GlobalRef, JClass, JString};
use jni::{JNIEnv, JavaVM};

static VM: OnceLock<JavaVM> = OnceLock::new();
static CLASS: OnceLock<GlobalRef> = OnceLock::new();

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_melihgurlek_hum_HumPlayer_nativeInit(env: JNIEnv, class: JClass) {
    let (Ok(vm), Ok(class)) = (env.get_java_vm(), env.new_global_ref(class)) else {
        log::error!("android: cannot keep a handle to HumPlayer");
        return;
    };
    let _ = VM.set(vm);
    let _ = CLASS.set(class);
    hum_core::player::set_backend(Box::new(send));
    log::info!("android: player bridge ready");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_melihgurlek_hum_HumPlayer_nativeEvent(mut env: JNIEnv, _class: JClass, json: JString) {
    if let Ok(s) = env.get_string(&json) {
        hum_core::player::event(&String::from(s));
    }
}

fn send(json: String) {
    let (Some(vm), Some(class)) = (VM.get(), CLASS.get()) else { return };
    // Engine threads are long-lived tokio workers: attach once and stay attached.
    let Ok(mut env) = vm.attach_current_thread_permanently() else {
        log::error!("android: cannot attach a thread to the JVM");
        return;
    };
    let result = env.new_string(&json).and_then(|s| {
        env.call_static_method(<&JClass>::from(class.as_obj()), "command", "(Ljava/lang/String;)V", &[(&s).into()])
    });
    if let Err(e) = result {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        log::error!("android: HumPlayer.command failed: {e}");
    }
}
