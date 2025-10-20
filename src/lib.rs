// Module declarations
mod agent;
mod animation;
mod app;
mod board;
mod editor;
mod events;
mod map;
mod map_type;
mod openrouter;
mod rendering;
mod tool_execution;
mod ui;

// Re-export the main app (used by WASM entry point)
#[cfg(target_arch = "wasm32")]
use app::MyApp;



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
                Box::new(|cc| Ok(Box::new(MyApp::new(cc, String::new())))),
            )
            .await
            .expect("failed to start eframe");
    });
}
