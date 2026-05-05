//! Headless HTTP-RPC server for driver and runtime integration testing.
//!
//! Start the app with `cable.exe --headless [port]` (default: 17285) to spin
//! up a local HTTP server that exposes app-level control commands without
//! opening the Tauri GUI window.  Only binds to 127.0.0.1 (loopback).
//!
//! All responses are JSON: `{"ok": <data>}` on success, `{"err": "..."}` on failure.
//!
//! Exposed endpoints:
//!
//! | Method | Path                          | Description                     |
//! |--------|-------------------------------|---------------------------------|
//! | GET    | /health                       | Liveness check                  |
//! | POST   | /driver/connect               | Open CableAudio driver handle   |
//! | GET    | /driver/connected             | Check driver connection status  |
//! | GET    | /virtual-devices              | List virtual devices            |
//! | POST   | /virtual-devices              | Create a virtual device         |
//! | DELETE | /virtual-devices/:id          | Remove a virtual device         |
//! | POST   | /virtual-devices/:id/format   | Set virtual device format       |
//! | POST   | /graph                        | Replace entire audio graph      |
//! | POST   | /runtime/enable               | Start audio processing thread   |
//! | POST   | /runtime/disable              | Stop audio processing thread    |
//! | GET    | /audio-devices                | List physical cpal audio devices|

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex as StdMutex};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{delete, get, post},
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::async_runtime::Mutex;

use crate::{AppData, VirtualDevice};
use crate::runtime::{AudioEdge, AudioNode};

/// Shared application state for all HTTP route handlers.
type Shared = Arc<Mutex<AppData>>;

// ---------------------------------------------------------------------------
// Response helpers
// ---------------------------------------------------------------------------

fn ok(data: impl serde::Serialize) -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(json!({ "ok": data })))
}

fn fail(msg: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "err": msg.to_string() })),
    )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn health() -> impl IntoResponse {
    ok("ok")
}

async fn connect_driver(State(state): State<Shared>) -> (StatusCode, Json<Value>) {
    #[cfg(windows)]
    {
        use crate::driver::client::DriverHandle;
        let mut app = state.lock().await;
        match DriverHandle::open() {
            Ok(handle) => {
                let arc = Arc::new(handle);
                app.driver_handle = Some(arc.clone());
                if let Ok(mut rt) = app.runtime.lock() {
                    rt.driver_handle = Some(arc);
                }
                ok(true)
            }
            Err(e) => fail(e),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = state;
        fail("Virtual devices require Windows")
    }
}

async fn is_driver_connected(State(state): State<Shared>) -> (StatusCode, Json<Value>) {
    let app = state.lock().await;
    #[cfg(windows)]
    return ok(app.driver_handle.is_some());
    #[cfg(not(windows))]
    {
        let _ = app;
        ok(false)
    }
}

async fn list_virtual_devices(State(state): State<Shared>) -> (StatusCode, Json<Value>) {
    let app = state.lock().await;
    let devices: Vec<&VirtualDevice> = app.virtual_devices.values().collect();
    ok(devices)
}

#[derive(Deserialize)]
struct CreateDeviceBody {
    name: String,
    #[serde(rename = "deviceType")]
    device_type: String,
}

async fn create_virtual_device(
    State(state): State<Shared>,
    Json(body): Json<CreateDeviceBody>,
) -> (StatusCode, Json<Value>) {
    #[cfg(windows)]
    {
        use crate::driver::endpoint::{
            elevated_set_endpoint_device_desc, find_new_endpoint_id, snapshot_endpoint_ids,
        };
        use crate::driver::types::DeviceType;

        let pre_snapshot = tokio::task::spawn_blocking(snapshot_endpoint_ids)
            .await
            .unwrap_or_else(|_| std::collections::HashSet::new());

        let dt = match body.device_type.as_str() {
            "render" => DeviceType::Render,
            "capture" => DeviceType::Capture,
            other => return fail(format!("Invalid device type: {}", other)),
        };

        let hex_id = {
            let app = state.lock().await;
            let driver = match app.driver_handle.as_ref() {
                Some(d) => d.clone(),
                None => return fail("Driver not connected"),
            };
            drop(app);
            match driver.create_virtual_device(&body.name, dt) {
                Ok(created) => {
                    println!(
                        "Created virtual {} device '{}' -> {}",
                        body.device_type,
                        body.name,
                        hex::encode(created.id)
                    );
                    hex::encode(created.id)
                }
                Err(e) => return fail(e),
            }
        };

        let name_for_ep = body.name.clone();
        let endpoint_id = tokio::task::spawn_blocking(move || {
            let ep_id = find_new_endpoint_id(&pre_snapshot, 40, 500)?;
            if !ep_id.is_empty() {
                if let Err(e) = elevated_set_endpoint_device_desc(&ep_id, &name_for_ep) {
                    eprintln!(
                        "elevated_set_endpoint_device_desc at creation failed: {}",
                        e
                    );
                }
            }
            Ok::<String, String>(ep_id)
        })
        .await
        .unwrap_or_else(|_| Ok(String::new()))
        .unwrap_or_default();

        let vd = VirtualDevice {
            id: hex_id.clone(),
            name: body.name,
            device_type: body.device_type,
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            endpoint_id,
        };

        let mut app = state.lock().await;
        app.virtual_devices.insert(hex_id, vd.clone());
        ok(vd)
    }
    #[cfg(not(windows))]
    {
        let _ = (state, body);
        fail("Virtual devices require Windows")
    }
}

async fn remove_virtual_device(
    State(state): State<Shared>,
    Path(device_id): Path<String>,
) -> (StatusCode, Json<Value>) {
    #[cfg(windows)]
    {
        use crate::driver::commands::hex_to_device_id;

        // Acquire state briefly to get driver + runtime references.
        let (driver, runtime_arc, id_bytes) = {
            let app = state.lock().await;
            let driver = match app.driver_handle.as_ref() {
                Some(d) => d.clone(),
                None => return fail("Driver not connected"),
            };
            let id_bytes = match hex_to_device_id(&device_id) {
                Ok(b) => b,
                Err(e) => return fail(e),
            };
            let runtime_arc = app.runtime.clone();
            (driver, runtime_arc, id_bytes)
        };

        // Dispose graph nodes referencing this device so the ring buffer is
        // unmapped before IOCTL_REMOVE (which returns STATUS_BUSY otherwise).
        match runtime_arc.lock() {
            Ok(mut rt) => {
                let node_ids: Vec<String> = rt
                    .nodes
                    .iter()
                    .filter_map(|n| match n {
                        AudioNode::VirtualAudioOutput(vn) if vn.device_id() == device_id => {
                            Some(n.id().to_string())
                        }
                        AudioNode::VirtualAudioInput(vn) if vn.device_id() == device_id => {
                            Some(n.id().to_string())
                        }
                        _ => None,
                    })
                    .collect();
                for id in &node_ids {
                    if let Err(e) = rt.remove_node(id) {
                        eprintln!(
                            "remove_virtual_device: remove_node({}) failed (non-fatal): {}",
                            id, e
                        );
                    }
                }
            }
            Err(e) => return fail(format!("runtime lock poisoned: {}", e)),
        }

        if let Err(e) = driver.remove_virtual_device(&id_bytes) {
            return fail(e);
        }

        let mut app = state.lock().await;
        app.virtual_devices.remove(&device_id);
        println!("Removed virtual device {}", device_id);
        ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (state, device_id);
        fail("Virtual devices require Windows")
    }
}

#[derive(Deserialize)]
struct SetFormatBody {
    channels: u32,
    #[serde(rename = "sampleRate")]
    sample_rate: u32,
    #[serde(rename = "bitsPerSample")]
    bits_per_sample: u32,
}

async fn set_virtual_device_format(
    State(state): State<Shared>,
    Path(device_id): Path<String>,
    Json(body): Json<SetFormatBody>,
) -> (StatusCode, Json<Value>) {
    if body.channels == 0 || body.channels > 8 {
        return fail(format!("Unsupported channel count: {}", body.channels));
    }
    if body.bits_per_sample != 16 && body.bits_per_sample != 24 && body.bits_per_sample != 32 {
        return fail(format!(
            "Unsupported bits_per_sample: {}. Must be 16, 24, or 32.",
            body.bits_per_sample
        ));
    }

    let endpoint_id = {
        let mut app = state.lock().await;
        let device = match app.virtual_devices.get_mut(&device_id) {
            Some(d) => d,
            None => return fail(format!("Device {} not found", device_id)),
        };
        device.channels = body.channels;
        device.sample_rate = body.sample_rate;
        device.bits_per_sample = body.bits_per_sample;
        device.endpoint_id.clone()
    };

    #[cfg(windows)]
    if !endpoint_id.is_empty() {
        use crate::driver::endpoint::elevated_set_endpoint_device_format;
        let ep = endpoint_id.clone();
        let (sr, ch, bps) = (body.sample_rate, body.channels, body.bits_per_sample);
        let result =
            tokio::task::spawn_blocking(move || {
                elevated_set_endpoint_device_format(&ep, sr, ch as u16, bps as u16)
            })
            .await;
        match result {
            Ok(Ok(())) => {
                println!(
                    "set_virtual_device_format: applied {} Hz / {} ch / {}-bit to '{}'",
                    body.sample_rate, body.channels, body.bits_per_sample, endpoint_id
                );
            }
            Ok(Err(e)) => {
                eprintln!(
                    "set_virtual_device_format: elevated format change failed (non-fatal): {}",
                    e
                );
                return fail(format!(
                    "Format applied locally but endpoint update failed: {}",
                    e
                ));
            }
            Err(e) => {
                eprintln!("set_virtual_device_format: spawn_blocking error: {}", e);
            }
        }
    }

    ok(())
}

#[derive(Deserialize)]
struct ReplaceGraphBody {
    nodes: Vec<AudioNode>,
    edges: Vec<AudioEdge>,
}

async fn replace_graph(
    State(state): State<Shared>,
    Json(body): Json<ReplaceGraphBody>,
) -> (StatusCode, Json<Value>) {
    let app = state.lock().await;
    let runtime_arc = app.runtime.clone();
    drop(app);

    let result = match runtime_arc.lock() {
        Ok(mut rt) => match rt.replace_graph(body.nodes, body.edges) {
            Ok(()) => ok(()),
            Err(e) => fail(e),
        },
        Err(e) => fail(format!("runtime lock poisoned: {}", e)),
    };
    result
}

async fn enable_runtime(State(state): State<Shared>) -> (StatusCode, Json<Value>) {
    let mut app = state.lock().await;
    crate::runtime::start_runtime_thread(&mut app);
    ok(())
}

async fn disable_runtime(State(state): State<Shared>) -> (StatusCode, Json<Value>) {
    let mut app = state.lock().await;
    match crate::runtime::stop_runtime_thread(&mut app) {
        Ok(()) => ok(()),
        Err(e) => fail(e),
    }
}

async fn get_audio_devices() -> (StatusCode, Json<Value>) {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();

    let inputs: Vec<Value> = host
        .input_devices()
        .map(|iter| {
            iter.filter_map(|d| {
                let id = d.id().ok()?.to_string();
                let name = d.description().ok()?.name().to_string();
                Some(json!({ "id": id, "name": name }))
            })
            .collect()
        })
        .unwrap_or_default();

    let outputs: Vec<Value> = host
        .output_devices()
        .map(|iter| {
            iter.filter_map(|d| {
                let id = d.id().ok()?.to_string();
                let name = d.description().ok()?.name().to_string();
                Some(json!({ "id": id, "name": name }))
            })
            .collect()
        })
        .unwrap_or_default();

    ok(json!({ "inputs": inputs, "outputs": outputs }))
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run the headless HTTP server on `127.0.0.1:port`.
///
/// Blocks until Ctrl+C is received.  Initialises a fresh `AppData` with no
/// graph and no driver connection — call `/driver/connect` after startup.
pub async fn run_headless(port: u16) {
    let state: Shared = Arc::new(Mutex::new(AppData {
        runtime: Arc::new(StdMutex::new(crate::runtime::Runtime::new_default())),
        runtime_thread: None,
        runtime_running: None,
        #[cfg(windows)]
        driver_handle: None,
        virtual_devices: BTreeMap::new(),
    }));

    let router = Router::new()
        .route("/health", get(health))
        .route("/driver/connect", post(connect_driver))
        .route("/driver/connected", get(is_driver_connected))
        .route("/virtual-devices", get(list_virtual_devices))
        .route("/virtual-devices", post(create_virtual_device))
        .route("/virtual-devices/{id}", delete(remove_virtual_device))
        .route("/virtual-devices/{id}/format", post(set_virtual_device_format))
        .route("/graph", post(replace_graph))
        .route("/runtime/enable", post(enable_runtime))
        .route("/runtime/disable", post(disable_runtime))
        .route("/audio-devices", get(get_audio_devices))
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    println!("Cable headless RPC server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind {}:{} — {}", addr, port, e));

    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
            println!("Shutting down headless server…");
        })
        .await
        .unwrap();
}
