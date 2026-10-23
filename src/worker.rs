//! The isolated browser solver entrypoint. No window, storage, or UI is used.

use wasm_bindgen::prelude::*;
use web_time::{Duration, Instant};

use crate::platform::SearchRequest;
use crate::seating::{Progress, Search};

#[wasm_bindgen]
pub struct WorkerSearch {
    search: Option<Search>,
    progress: Progress,
}

#[wasm_bindgen]
impl WorkerSearch {
    #[wasm_bindgen(constructor)]
    pub fn new(request: &str) -> Result<WorkerSearch, JsValue> {
        let request: SearchRequest =
            serde_json::from_str(request).map_err(|e| JsValue::from_str(&format!("{e}")))?;
        let search = request
            .build()
            .map_err(|e| JsValue::from_str(&format!("{e:#}")))?;
        Ok(Self {
            progress: search.progress(),
            search: Some(search),
        })
    }

    /// Advance a bounded chunk, then let the worker's task queue handle Stop.
    /// A completed outcome crosses the worker boundary as JSON without passing
    /// i64 scores through JavaScript's floating point number representation.
    pub fn advance(&mut self, milliseconds: u32) -> Option<String> {
        let search = self.search.as_mut()?;
        search.advance(Instant::now() + Duration::from_millis(milliseconds as u64));
        if search.done() {
            let outcome = self.search.take()?.finish();
            Some(serde_json::to_string(&outcome).expect("serializable outcome"))
        } else {
            None
        }
    }

    pub fn progress(&self) -> String {
        serde_json::to_string(&self.progress.best()).expect("serializable progress")
    }

    pub fn stop(&self) {
        self.progress.stop();
    }
}
