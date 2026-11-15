//! The single save slot and, on the web, the URL query string. Native: a file in the working directory.
//! Web: `localStorage` through the miniquad plugin `web/qe_plugin.js`.
use sim::save::{load_from_string, save_to_string};
use sim::Game;

#[cfg(not(target_arch = "wasm32"))]
const SAVE_FILE: &str = "quietly_everything.save.json";

#[cfg(target_arch = "wasm32")]
mod web {
    #[link(wasm_import_module = "env")]
    extern "C" {
        pub fn qe_save(ptr: *const u8, len: u32);
        pub fn qe_load_len() -> i32;
        pub fn qe_load_into(ptr: *mut u8);
        pub fn qe_query_len() -> i32;
        pub fn qe_query_into(ptr: *mut u8);
    }

    /// The plugin handshake: `mq_js_bundle.js` compares this with the `version` the plugin registered.
    #[no_mangle]
    pub extern "C" fn qe_crate_version() -> u32 {
        1
    }

    /// Rust allocates `len` bytes and JS copies the string into them.
    pub fn fetch(len: i32, into: unsafe extern "C" fn(*mut u8)) -> Option<String> {
        if len < 0 {
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        if len > 0 {
            unsafe { into(buf.as_mut_ptr()) };
        }
        String::from_utf8(buf).ok()
    }
}

fn read_text() -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read_to_string(SAVE_FILE).ok()
    }
    #[cfg(target_arch = "wasm32")]
    {
        web::fetch(unsafe { web::qe_load_len() }, web::qe_load_into)
    }
}

/// Writes the slot; false when the platform refused.
pub fn save(game: &Game) -> bool {
    let text = save_to_string(game);
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::write(SAVE_FILE, text).is_ok()
    }
    #[cfg(target_arch = "wasm32")]
    {
        unsafe { web::qe_save(text.as_ptr(), text.len() as u32) };
        exists()
    }
}

pub fn load() -> Option<Game> {
    load_from_string(&read_text()?).ok()
}

pub fn exists() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::path::Path::new(SAVE_FILE).exists()
    }
    #[cfg(target_arch = "wasm32")]
    {
        unsafe { web::qe_load_len() >= 0 }
    }
}

/// The flags: argv natively, the URL query keys on the web (`?autoplay&seed=3` becomes `autoplay`, `seed=3`).
pub fn args() -> Vec<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::args().skip(1).collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        let query = web::fetch(unsafe { web::qe_query_len() }, web::qe_query_into).unwrap_or_default();
        query.split('&').filter(|p| !p.is_empty()).map(str::to_owned).collect()
    }
}
