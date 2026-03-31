/// Virtual Audio Output node.
///
/// Creates a virtual **render** (speaker/output) device in the CableAudio driver.
/// In the Flow UI this is a **source** node: Windows applications write audio
/// to this virtual speaker, and the node reads samples from the driver's shared
/// ring buffer, forwarding them to downstream nodes (e.g., real audio outputs).
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
pub(crate) struct VirtualAudioOutputNode {
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

  /// Mapped ring buffer for reading audio data
  #[serde(skip)]
  #[cfg(windows)]
  ring_buffer: Option<RingBufferMapping>,
}

impl std::fmt::Debug for VirtualAudioOutputNode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("VirtualAudioOutputNode")
      .field("id", &self.id)
      .field("name", &self.name)
      .finish()
  }
}

impl NodeTrait for VirtualAudioOutputNode {
  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    println!(
      "Initializing virtual audio output (render): {} ({})",
      self.name, self.id
    );

    #[cfg(windows)]
    {
      let driver = runtime
        .driver_handle
        .as_ref()
        .ok_or_else(|| "CableAudio driver not connected".to_string())?;

      // Create a virtual render device in the driver
      let device_id = driver.create_virtual_device(&self.name, DeviceType::Render)?;
      println!(
        "Created virtual render device: {:?} for node {}",
        device_id, self.id
      );

      // Map the ring buffer into our process
      let mapping = driver.map_ring_buffer(&device_id)?;
      println!(
        "Mapped ring buffer for virtual render: addr={:?}, total={}, data={}",
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
      "Disposing virtual audio output (render): {} ({})",
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
    _state: &RuntimeState,
  ) -> Result<BTreeMap<String, Vec<f32>>, String> {
    #[cfg(windows)]
    {
      let ring_buffer = match self.ring_buffer.as_mut() {
        Some(rb) => rb,
        None => return Ok(BTreeMap::new()),
      };

      // Read available samples from the driver ring buffer.
      // The driver writes to this buffer when Windows apps play audio to
      // this virtual render device.
      let max_samples = runtime.buffer_size as usize * 2; // stereo
      let mut buffer = vec![0.0f32; max_samples];
      let samples_read = ring_buffer.read_f32_samples(&mut buffer);

      if samples_read == 0 {
        return Ok(BTreeMap::new());
      }

      buffer.truncate(samples_read);

      // Output data on all outgoing edges
      let mut output = BTreeMap::new();
      for edge in &runtime.edges {
        if edge.from == self.id {
          output.insert(edge.id.clone(), buffer.clone());
        }
      }

      return Ok(output);
    }

    #[cfg(not(windows))]
    {
      let _ = (runtime, _state);
      Ok(BTreeMap::new())
    }
  }
}
