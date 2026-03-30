use std::collections::BTreeMap;

use crate::runtime::{Runtime, RuntimeState};

pub mod audio_input_device;
pub mod audio_output_device;

pub(crate) trait NodeTrait {
  fn init(&mut self, runtime: &Runtime) -> Result<(), String>;

  fn dispose(&mut self, runtime: &Runtime) -> Result<(), String>;

  fn process(
    &self,
    runtime: &Runtime,
    state: &RuntimeState,
  ) -> Result<BTreeMap<String, Vec<f32>>, String>;
}
