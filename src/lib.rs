// Module declarations
mod agent;
mod app;
mod events;
mod map;
mod map_type;
mod openrouter;
mod rendering;

// Re-export the main app (used by WASM entry point)
#[cfg(target_arch = "wasm32")]
use app::MyApp;

// OpenRouter API key
const OPENROUTER_API_KEY: &str =
    "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";

// WASM entry point
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    let web_options = eframe::WebOptions::default();
    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "the_canvas_id",
                web_options,
                Box::new(|cc| Ok(Box::new(MyApp::new(cc, OPENROUTER_API_KEY.to_string())))),
            )
            .await
            .expect("failed to start eframe");
    });
}
