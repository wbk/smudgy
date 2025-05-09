use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::SessionId;
use super::runtime::Runtime;

/// Global registry of all active sessions
static SESSION_REGISTRY: OnceLock<Arc<Mutex<HashMap<SessionId, Arc<Runtime>>>>> = OnceLock::new();

/// Get the global session registry
pub fn get_registry() -> Arc<Mutex<HashMap<SessionId, Arc<Runtime>>>> {
    SESSION_REGISTRY
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

/// Register a new session in the global registry
pub fn register_session(session_id: SessionId, runtime: Arc<Runtime>) {
    let registry = get_registry();
    let mut sessions = registry.lock().unwrap();
    sessions.insert(session_id, runtime);
    log::info!("Registered session {} in global registry", session_id);
}

/// Unregister a session from the global registry
pub fn unregister_session(session_id: SessionId) {
    let registry = get_registry();
    let mut sessions = registry.lock().unwrap();
    if sessions.remove(&session_id).is_some() {
        log::info!("Unregistered session {} from global registry", session_id);
    } else {
        log::warn!("Attempted to unregister non-existent session {}", session_id);
    }
}

/// Get all active session IDs
pub fn get_all_session_ids() -> Vec<SessionId> {
    let registry = get_registry();
    let sessions = registry.lock().unwrap();
    sessions.keys().copied().collect()
}

/// Get a specific runtime by session ID
pub fn get_runtime(session_id: SessionId) -> Option<Arc<Runtime>> {
    let registry = get_registry();
    let sessions = registry.lock().unwrap();
    sessions.get(&session_id).cloned()
}

/// Get the number of active sessions
pub fn session_count() -> usize {
    let registry = get_registry();
    let sessions = registry.lock().unwrap();
    sessions.len()
} 