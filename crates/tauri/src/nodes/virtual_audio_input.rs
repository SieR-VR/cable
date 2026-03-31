/// Virtual Audio Input node.
///
/// Creates a virtual **capture** (microphone) device in the CableAudio driver.
/// In the Flow UI this is a **sink** node: it receives audio data from upstream
/// nodes and writes samples into the driver's shared ring buffer. Windows
/// applications (Discord, OBS, etc.) see this as a microphone input device.
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
use common::{DeviceId, DeviceType};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VirtualAudioInputNode {
  /// Node ID (matches ReactFlow node id)
  id: String,
  /// User-chosen display name for the virtual device
  name: String,

  /// Driver-assigned device ID (set during init)
  #[serde(skip)]
  #[cfg(windows)]
  device_id: Option<DeviceId>,

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
      .field("name", &self.name)
      .finish()
  }
}

impl NodeTrait for VirtualAudioInputNode {
  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    println!(
      "Initializing virtual audio input (capture): {} ({})",
      self.name, self.id
    );

    #[cfg(windows)]
    {
      let driver = runtime
        .driver_handle
        .as_ref()
        .ok_or_else(|| "CableAudio driver not connected".to_string())?;

      // Create a virtual capture device in the driver
      let device_id = driver.create_virtual_device(&self.name, DeviceType::Capture)?;
      println!(
        "Created virtual capture device: {:?} for node {}",
        device_id, self.id
      );

      // Map the ring buffer into our process
      let mapping = driver.map_ring_buffer(&device_id)?;
      println!(
        "Mapped ring buffer for virtual capture: addr={:?}, total={}, data={}",
        mapping.user_address, mapping.total_size, mapping.data_buffer_size
      );

      self.device_id = Some(device_id);
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
      "Disposing virtual audio input (capture): {} ({})",
      self.name, self.id
    );

    #[cfg(windows)]
    {
      // Unmap ring buffer first
      if let (Some(driver), Some(device_id), Some(mapping)) = (
        self.driver_handle.as_ref(),
        self.device_id.as_ref(),
        self.ring_buffer.take(),
      ) {
        if let Err(e) = driver.unmap_ring_buffer(device_id, mapping.user_address) {
          eprintln!("Warning: failed to unmap ring buffer: {}", e);
        }
      }

      // Remove the virtual device
      if let (Some(driver), Some(device_id)) =
        (self.driver_handle.as_ref(), self.device_id.as_ref())
      {
        if let Err(e) = driver.remove_virtual_device(device_id) {
          eprintln!("Warning: failed to remove virtual device: {}", e);
        }
      }

      self.device_id = None;
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
