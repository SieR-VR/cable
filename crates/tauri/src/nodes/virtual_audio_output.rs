/// Virtual Audio Output node.
///
/// Uses a pre-created virtual **render** (speaker) device.
/// In the Flow UI this is a **source** node: Windows applications write audio
/// to this virtual speaker, and the node reads samples from the driver's shared
/// ring buffer, forwarding them to downstream nodes.
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
pub(crate) struct VirtualAudioOutputNode {
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

  /// Mapped ring buffer for reading audio data
  #[serde(skip)]
  #[cfg(windows)]
  ring_buffer: Option<RingBufferMapping>,

  #[serde(skip)]
  debug_tick: u64,
}

impl std::fmt::Debug for VirtualAudioOutputNode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("VirtualAudioOutputNode")
      .field("id", &self.id)
      .field("device_id", &self.device_id)
      .field("name", &self.name)
      .finish()
  }
}

impl NodeTrait for VirtualAudioOutputNode {
  fn id(&self) -> &str {
    &self.id
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    println!(
      "Initializing virtual audio output (render): {} device={}",
      self.name, self.device_id
    );

    if self.device_id.is_empty() {
      return Err("No virtual render device selected".to_string());
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
        "Mapped ring buffer for virtual render {}: addr={:?}, total={}, data={}",
        self.device_id, mapping.user_address, mapping.total_size, mapping.data_buffer_size
      );

      self.parsed_device_id = Some(device_id);
      self.driver_handle = Some(driver.clone());
      self.ring_buffer = Some(mapping);

      // Stream format metadata is owned by the driver and updated when the
      // render stream is created. Do not override it from user mode.
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
      "Disposing virtual audio output (render): {} device={}",
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
    _state: &RuntimeState,
  ) -> Result<BTreeMap<String, Vec<f32>>, String> {
    #[cfg(windows)]
    {
      let ring_buffer = match self.ring_buffer.as_mut() {
        Some(rb) => rb,
        None => return Ok(BTreeMap::new()),
      };

      // Read available samples from the driver ring buffer.
      // Use the actual channel count from the stream format metadata so that
      // mono, stereo, and surround formats all produce the correct frame count.
      // Fall back to 2 (stereo) if metadata is not yet available.
      let channels = ring_buffer
        .read_stream_format_metadata()
        .map(|(_sr, ch, _bits, _dt)| ch as usize)
        .unwrap_or(2)
        .max(1);
      // Read all available data (up to 4x buffer_size) to keep up with the
      // driver's write rate.  Limiting to a reasonable maximum prevents
      // unbounded allocations if the ring buffer is very full.
      let max_samples = runtime.buffer_size as usize * channels * 4;
      let mut buffer = vec![0.0f32; max_samples];
      let samples_read = ring_buffer.read_f32_samples(&mut buffer);

      self.debug_tick = self.debug_tick.wrapping_add(1);
      if self.debug_tick % 200 == 0 {
        let (w, r, sz, st) = ring_buffer.debug_ring_stats();
        let fmt = ring_buffer.read_stream_format_metadata();
        println!(
          "VirtualAudioOutput[{}] stats: read={} write_idx={} read_idx={} size={} status={} fmt={:?}",
          self.id, samples_read, w, r, sz, st, fmt
        );
      }

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
