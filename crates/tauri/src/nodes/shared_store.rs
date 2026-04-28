//! Type-erased per-node-instance shared state.
//!
//! Each node type defines its own state struct (e.g. `VstNodeShared`) and
//! registers it under the node id. The struct lives behind an `Arc` so the
//! audio-thread node, Tauri commands, helper threads (e.g. the VST editor
//! window thread) can all hold references to the same data.
//!
//! This mechanism is plugin-friendly: a future plugin-loaded node type only
//! needs its state struct to be `'static + Send + Sync`. Two slots cannot
//! share the same node id with different types.

use std::any::Any;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

impl std::fmt::Debug for NodeSharedStore {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let len = self.inner.lock().map(|m| m.len()).unwrap_or(0);
    f.debug_struct("NodeSharedStore")
      .field("entries", &len)
      .finish()
  }
}

pub(crate) struct NodeSharedStore {
  inner: Mutex<BTreeMap<String, Arc<dyn Any + Send + Sync>>>,
}

impl NodeSharedStore {
  pub fn new() -> Self {
    Self {
      inner: Mutex::new(BTreeMap::new()),
    }
  }

  /// Returns the existing `Arc<T>` for `node_id`, or inserts a new one
  /// produced by `init`. Returns `None` if a slot with that id already
  /// exists with a different concrete type.
  pub fn get_or_init<T, F>(&self, node_id: &str, init: F) -> Option<Arc<T>>
  where
    T: 'static + Send + Sync,
    F: FnOnce() -> T,
  {
    let mut map = self.inner.lock().ok()?;
    if let Some(existing) = map.get(node_id) {
      return existing.clone().downcast::<T>().ok();
    }
    let new: Arc<T> = Arc::new(init());
    map.insert(
      node_id.to_string(),
      new.clone() as Arc<dyn Any + Send + Sync>,
    );
    Some(new)
  }

  /// Returns the `Arc<T>` for `node_id` if it exists and the type matches.
  pub fn get<T: 'static + Send + Sync>(&self, node_id: &str) -> Option<Arc<T>> {
    let map = self.inner.lock().ok()?;
    map.get(node_id).cloned()?.downcast::<T>().ok()
  }

  pub fn remove(&self, node_id: &str) {
    if let Ok(mut map) = self.inner.lock() {
      map.remove(node_id);
    }
  }
}

impl Default for NodeSharedStore {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  struct Foo(u32);
  struct Bar(String);

  #[test]
  fn get_or_init_returns_same_arc() {
    let store = NodeSharedStore::new();
    let a = store.get_or_init::<Foo, _>("n1", || Foo(42)).unwrap();
    let b = store.get_or_init::<Foo, _>("n1", || Foo(99)).unwrap();
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(b.0, 42);
  }

  #[test]
  fn get_returns_none_when_missing() {
    let store = NodeSharedStore::new();
    assert!(store.get::<Foo>("missing").is_none());
  }

  #[test]
  fn get_returns_none_for_wrong_type() {
    let store = NodeSharedStore::new();
    let _ = store.get_or_init::<Foo, _>("n1", || Foo(1));
    assert!(store.get::<Bar>("n1").is_none());
  }

  #[test]
  fn remove_drops_entry() {
    let store = NodeSharedStore::new();
    let _ = store.get_or_init::<Foo, _>("n1", || Foo(1));
    store.remove("n1");
    assert!(store.get::<Foo>("n1").is_none());
  }
}
