/// Virtual Audio Input node.
///
/// Uses a pre-created virtual **capture** (microphone) device.
/// In the Flow UI this is a **sink** node: it receives audio data from upstream
/// nodes and writes samples into the driver's shared ring buffer.
use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
  nodes::NodeTrait,
  runtime::{Runtime, RuntimeState},
};

#[cfg(windows)]
use crate::driver_client::{DriverHandle, RingBufferMapping};
#[cfg(windows)]
use common::DeviceId;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VirtualAudioInputNode {
  /// Node ID (matches ReactFlow node id)
  id: String,
  /// Hex-encoded device ID of the pre-created virtual device
  device_id: String,
  /// Display name (informational)
  name: String,

  /// Parsed 16-byte device ID
  #[serde(skip)]
  #[cfg(windows)]
  parsed_device_id: Option<DeviceId>,

  /// Handle to the driver
  #[serde(skip)]
  #[cfg(windows)]
  driver_handle: Option<Arc<DriverHandle>>,

  /// Mapped ring buffer for writing audio data
  #[serde(skip)]
  #[cfg(windows)]
  ring_buffer: Option<RingBufferMapping>,
}

impl std::fmt::Debug for VirtualAudioInputNode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("VirtualAudioInputNode")
      .field("id", &self.id)
      .field("device_id", &self.device_id)
      .field("name", &self.name)
      .finish()
  }
}

impl NodeTrait for VirtualAudioInputNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    println!(
      "Initializing virtual audio input (capture): {} device={}",
      self.name, self.device_id
    );

    if self.device_id.is_empty() {
      return Err("No virtual capture device selected".to_string());
    }

    #[cfg(windows)]
    {
      let driver = runtime
        .driver_handle
        .as_ref()
        .ok_or_else(|| "CableAudio driver not connected".to_string())?;

      let device_id = crate::hex_to_device_id(&self.device_id)?;

      // Map the ring buffer for the pre-created device
      let mapping = driver.map_ring_buffer(&device_id)?;
      println!(
        "Mapped ring buffer for virtual capture {}: addr={:?}, total={}, data={}",
        self.device_id, mapping.user_address, mapping.total_size, mapping.data_buffer_size
      );

      self.parsed_device_id = Some(device_id);
      self.driver_handle = Some(driver.clone());
      self.ring_buffer = Some(mapping);
    }

    #[cfg(not(windows))]
    {
      let _ = runtime;
      return Err("Virtual audio devices require Windows".to_string());
    }

    #[cfg(windows)]
    Ok(())
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    println!(
      "Disposing virtual audio input (capture): {} device={}",
      self.name, self.device_id
    );

    #[cfg(windows)]
    {
      // Unmap ring buffer (device is NOT removed here - it's managed by the menu panel)
      if let (Some(driver), Some(device_id), Some(mapping)) = (
        self.driver_handle.as_ref(),
        self.parsed_device_id.as_ref(),
        self.ring_buffer.take(),
      ) {
        if let Err(e) = driver.unmap_ring_buffer(device_id, mapping.user_address) {
          eprintln!("Warning: failed to unmap ring buffer: {}", e);
        }
      }

      self.parsed_device_id = None;
      self.driver_handle = None;
    }

    Ok(())
  }

  fn process(
    &mut self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, Vec<f32>>, String> {
    #[cfg(windows)]
    {
      let ring_buffer = match self.ring_buffer.as_mut() {
        Some(rb) => rb,
        None => return Ok(BTreeMap::new()),
      };

      // Collect all incoming edge data and write to ring buffer
      for edge in &runtime.edges {
        if edge.to == self.id {
          if let Some(data) = state.edge_values.get(&edge.id) {
            ring_buffer.write_f32_samples(data);
          }
        }
      }
    }

    #[cfg(not(windows))]
    {
      let _ = (runtime, state);
    }

    // Sink node - no downstream output
    Ok(BTreeMap::new())
  }
}
