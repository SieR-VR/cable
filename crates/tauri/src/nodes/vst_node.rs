/// VST3 plugin host node.
///
/// 선택된 VST3 플러그인 DLL을 동적으로 로드하여 오디오를 처리한다.
/// libloading으로 DLL을 열고, COM vtable dispatch로 IAudioProcessor를 호출한다.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
  nodes::{vst3_com, AudioBuffer, NodeTrait},
  runtime::{Runtime, RuntimeState},
};

/// VST3 플러그인 스캔 결과 항목.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VstPluginInfo {
  pub name: String,
  pub path: String,
  pub vendor: String,
  pub num_inputs: u16,
  pub num_outputs: u16,
  pub num_params: u32,
}

/// VST3 파라미터 정보 (프론트엔드에 전달).
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VstParamInfo {
  pub id: u32,
  pub title: String,
  pub value: f64,
}

// ---------------------------------------------------------------------------
// Vst3Plugin 내부 구조체
// ---------------------------------------------------------------------------

/// 로드된 VST3 플러그인 인스턴스.
///
/// DLL이 살아 있는 동안 IComponent / IAudioProcessor 포인터가 유효하다.
/// 드롭 시 COM 해제와 라이브러리 언로드가 자동으로 수행된다.
struct Vst3Plugin {
  lib: libloading::Library,
  component: *mut vst3_com::IComponent,
  processor: *mut vst3_com::IAudioProcessor,
}

// VST3 플러그인은 spec에 따라 스레드 안전이 보장된다.
// 오디오 처리는 항상 동일한 스레드(spin-loop)에서 호출된다.
unsafe impl Send for Vst3Plugin {}

impl std::fmt::Debug for Vst3Plugin {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Vst3Plugin {{ component: {:?}, processor: {:?} }}", self.component, self.processor)
  }
}

impl Drop for Vst3Plugin {
  fn drop(&mut self) {
    unsafe {
      if !self.processor.is_null() {
        (*self.processor).set_processing(false);
        (*self.processor).release();
      }
      if !self.component.is_null() {
        (*self.component).set_active(false);
        (*self.component).terminate();
        (*self.component).release();
      }
      // lib은 마지막으로 drop되어 DLL 언로드
    }
  }
}

// ---------------------------------------------------------------------------
// VstNode
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VstNode {
  /// Node ID (ReactFlow node id와 일치)
  id: String,
  /// 선택된 .vst3 DLL 절대 경로
  plugin_path: String,
  /// 입력 버스 수 (핸들 vst-in-0..N-1)
  num_inputs: u16,
  /// 출력 버스 수 (핸들 vst-out-0..N-1)
  num_outputs: u16,
  /// 처리 채널 수 (입출력 공통, 일반적으로 2 = stereo)
  channels: u16,
  /// 파라미터 정규화 값 [0.0, 1.0], 인덱스 순서
  params: Vec<f64>,

  #[serde(skip)]
  plugin: Option<Vst3Plugin>,
  /// IComponent::getControllerClassId()로 얻은 CID. load_plugin 후 설정.
  #[serde(skip)]
  pub ctrl_cid: Option<[u8; 16]>,
}

impl NodeTrait for VstNode {
  fn id(&self) -> &str {
    &self.id
  }

  /// DLL을 임시로 로드해 IEditController CID만 추출한다.
  /// Runtime 없이도 호출 가능하며, 플러그인 선택 시 즉시 실행된다.
  fn create(&mut self) -> Result<(), String> {
    if self.plugin_path.is_empty() {
      return Ok(());
    }
    unsafe { self.extract_ctrl_cid() }
  }

  fn init(&mut self, runtime: &Runtime) -> Result<(), String> {
    println!("Initializing VST node: {} ({})", self.id, self.plugin_path);

    if self.plugin_path.is_empty() {
      return Ok(());
    }

    unsafe { self.load_plugin(runtime) }
  }

  fn dispose(&mut self, _runtime: &Runtime) -> Result<(), String> {
    println!("Disposing VST node: {}", self.id);
    // Vst3Plugin::drop()이 COM 해제 및 DLL 언로드를 처리한다.
    self.plugin = None;
    Ok(())
  }

  fn process(&mut self, runtime: &Runtime,
             state: &RuntimeState)
             -> Result<BTreeMap<String, AudioBuffer>, String> {
    let plugin = match self.plugin.as_mut() {
      Some(p) => p,
      None => return self.passthrough(runtime, state),
    };

    unsafe { Self::process_with_plugin(plugin, &self.id, self.channels, self.num_inputs,
                                       self.num_outputs, runtime, state) }
  }
}

impl VstNode {
  /// DLL을 임시 로드하여 IEditController CID만 추출한다.
  /// IComponent는 생성 직후 해제하며, lib은 스코프 종료 시 언로드된다.
  unsafe fn extract_ctrl_cid(&mut self) -> Result<(), String> {
    let lib = libloading::Library::new(&self.plugin_path)
      .map_err(|e| format!("VST3 DLL 로드 실패: {e}"))?;

    let get_factory: libloading::Symbol<vst3_com::GetPluginFactoryFn> =
      lib.get(b"GetPluginFactory\0")
         .map_err(|e| format!("GetPluginFactory 심볼 없음: {e}"))?;
    let factory = get_factory();
    if factory.is_null() {
      return Err("factory null".to_string());
    }
    let factory = &mut *factory;

    let num_classes = factory.count_classes();
    let mut audio_cid: Option<[u8; 16]> = None;
    for i in 0..num_classes {
      if let Some(info) = factory.get_class_info(i) {
        let cat = vst3_com::cchar_to_string(&info.category);
        if cat.starts_with("Audio Module Class") {
          audio_cid = Some(info.cid);
          break;
        }
      }
    }
    let audio_cid =
      audio_cid.ok_or_else(|| "Audio Module Class를 찾을 수 없습니다.".to_string())?;

    if let Some(comp_ptr) = factory.create_instance(&audio_cid, &vst3_com::IID_ICOMPONENT) {
      let component = comp_ptr as *mut vst3_com::IComponent;
      if (*component).initialize(std::ptr::null_mut()) == vst3_com::K_RESULT_OK {
        self.ctrl_cid = (*component).get_controller_class_id();
        (*component).terminate();
      }
      (*component).release();
    }
    // lib drops → DLL 언로드
    Ok(())
  }

  /// DLL을 로드하고 IComponent / IAudioProcessor를 초기화한다.
  unsafe fn load_plugin(&mut self, runtime: &Runtime) -> Result<(), String> {
    let lib = libloading::Library::new(&self.plugin_path)
      .map_err(|e| format!("VST3 DLL 로드 실패 '{}': {}", self.plugin_path, e))?;

    // GetPluginFactory 심볼 획득
    let get_factory: libloading::Symbol<vst3_com::GetPluginFactoryFn> =
      lib.get(b"GetPluginFactory\0")
         .map_err(|e| format!("GetPluginFactory 심볼 없음: {}", e))?;
    let factory = get_factory();
    if factory.is_null() {
      return Err("GetPluginFactory가 null을 반환했습니다.".to_string());
    }
    let factory = &mut *factory;

    // Audio Module Class CID 탐색
    let num_classes = factory.count_classes();
    let mut audio_cid: Option<[u8; 16]> = None;
    for i in 0..num_classes {
      if let Some(info) = factory.get_class_info(i) {
        let cat = vst3_com::cchar_to_string(&info.category);
        if cat.starts_with("Audio Module Class") {
          audio_cid = Some(info.cid);
          break;
        }
      }
    }
    let audio_cid = audio_cid.ok_or_else(|| "Audio Module Class를 찾을 수 없습니다.".to_string())?;

    // IComponent 생성
    let comp_ptr = factory.create_instance(&audio_cid, &vst3_com::IID_ICOMPONENT)
                          .ok_or_else(|| "IComponent 생성 실패".to_string())?;
    let component = comp_ptr as *mut vst3_com::IComponent;
    let result = (*component).initialize(std::ptr::null_mut());
    if result != vst3_com::K_RESULT_OK {
      (*component).release();
      return Err(format!("IComponent::initialize 실패: {result:#x}"));
    }

    // IAudioProcessor 쿼리
    let proc_ptr = (*component).query_interface(&vst3_com::IID_IAUDIO_PROCESSOR)
                               .ok_or_else(|| "IAudioProcessor 인터페이스 없음".to_string())?;
    let processor = proc_ptr as *mut vst3_com::IAudioProcessor;

    // 버스 스피커 어레인지먼트 설정
    let arrangement = if self.channels == 1 { vst3_com::K_MONO } else { vst3_com::K_STEREO };
    let mut inputs: Vec<u64> = vec![arrangement; self.num_inputs as usize];
    let mut outputs: Vec<u64> = vec![arrangement; self.num_outputs as usize];
    (*processor).set_bus_arrangements(&mut inputs, &mut outputs);

    // 입출력 버스 활성화
    for i in 0..(self.num_inputs as i32) {
      (*component).activate_bus(vst3_com::K_AUDIO, vst3_com::K_INPUT, i, true);
    }
    for i in 0..(self.num_outputs as i32) {
      (*component).activate_bus(vst3_com::K_AUDIO, vst3_com::K_OUTPUT, i, true);
    }

    // setupProcessing
    let setup = vst3_com::ProcessSetup::new(vst3_com::K_REALTIME,
                                            vst3_com::K_SAMPLE32,
                                            runtime.buffer_size as i32,
                                            runtime.sample_rate as f64);
    let r = (*processor).setup_processing(&setup);
    if r != vst3_com::K_RESULT_OK {
      println!("VST3 setupProcessing 반환값: {r:#x}");
    }

    (*component).set_active(true);
    (*processor).set_processing(true);

    // ctrl_cid: IEditController CID를 미리 저장해 에디터 스레드에서 재사용
    self.ctrl_cid = (*component).get_controller_class_id();

    self.plugin = Some(Vst3Plugin { lib, component, processor });
    println!("VST3 플러그인 초기화 완료: {}", self.plugin_path);
    Ok(())
  }

  /// 실제 IAudioProcessor::process()를 호출하여 오디오를 처리한다.
  unsafe fn process_with_plugin(plugin: &mut Vst3Plugin, node_id: &str, channels: u16,
                                 num_inputs: u16, num_outputs: u16, runtime: &Runtime,
                                 state: &RuntimeState)
                                 -> Result<BTreeMap<String, AudioBuffer>, String> {
    let ch = channels as usize;
    let frames = runtime.buffer_size as usize;

    // 입력 버스별 채널 분리 버퍼 수집
    let mut in_channel_bufs: Vec<Vec<Vec<f32>>> = Vec::new();
    let mut proto: Option<AudioBuffer> = None;

    for bus_idx in 0..num_inputs {
      let handle_id = format!("vst-in-{}", bus_idx);
      let buf = runtime.edges.iter().find(|e| {
        e.to == node_id && e.to_handle.as_deref() == Some(&handle_id)
      }).and_then(|e| state.edge_values.get(&e.id));

      let samples = if let Some(b) = buf {
        if proto.is_none() {
          proto = Some(b.clone());
        }
        b.samples.clone()
      } else {
        vec![0.0f32; frames * ch]
      };

      // 인터리브드 → 채널별 분리
      let mut chans: Vec<Vec<f32>> = vec![vec![0.0f32; frames]; ch];
      for (i, s) in samples.iter().enumerate() {
        chans[i % ch][i / ch] = *s;
      }
      in_channel_bufs.push(chans);
    }

    // 출력 채널 버퍼 (0으로 초기화)
    let mut out_channel_bufs: Vec<Vec<Vec<f32>>> =
      vec![vec![vec![0.0f32; frames]; ch]; num_outputs as usize];

    // AudioBusBuffers 포인터 배열 구성
    let mut in_ptrs: Vec<Vec<*mut f32>> = in_channel_bufs.iter_mut()
                                                          .map(|bus| {
                                                            bus.iter_mut()
                                                               .map(|ch| ch.as_mut_ptr())
                                                               .collect()
                                                          })
                                                          .collect();
    let mut out_ptrs: Vec<Vec<*mut f32>> = out_channel_bufs.iter_mut()
                                                            .map(|bus| {
                                                              bus.iter_mut()
                                                                 .map(|ch| ch.as_mut_ptr())
                                                                 .collect()
                                                            })
                                                            .collect();

    let mut in_buses: Vec<vst3_com::AudioBusBuffers> =
      in_ptrs.iter_mut()
             .map(|ptrs| vst3_com::AudioBusBuffers::new(ch as i32, 0, ptrs.as_mut_ptr()))
             .collect();
    let mut out_buses: Vec<vst3_com::AudioBusBuffers> =
      out_ptrs.iter_mut()
              .map(|ptrs| vst3_com::AudioBusBuffers::new(ch as i32, 0, ptrs.as_mut_ptr()))
              .collect();

    let mut process_data =
      vst3_com::ProcessData::new(frames as i32,
                                 in_buses.as_mut_ptr(),
                                 num_inputs as i32,
                                 out_buses.as_mut_ptr(),
                                 num_outputs as i32);

    (*plugin.processor).process(&mut process_data);

    // 출력 채널 → 인터리브드 AudioBuffer
    let sample_rate = proto.as_ref().map_or(48000, |p| p.sample_rate);
    let bits = proto.as_ref().map_or(32, |p| p.bits_per_sample);

    let mut result = BTreeMap::new();
    for edge in &runtime.edges {
      if edge.from != node_id {
        continue;
      }
      // 출력 버스 인덱스 결정: vst-out-N 또는 첫 번째 버스
      let bus_idx: usize = 0; // 단일 출력 버스 사용
      let chans = &out_channel_bufs[bus_idx.min(out_channel_bufs.len() - 1)];
      // 채널별 → 인터리브드
      let mut interleaved = vec![0.0f32; frames * ch];      for (c, chan) in chans.iter().enumerate() {
        for (f, &s) in chan.iter().enumerate() {
          interleaved[f * ch + c] = s;
        }
      }
      result.insert(edge.id.clone(), AudioBuffer::new(interleaved, channels, sample_rate, bits));
    }

    Ok(result)
  }

  /// 플러그인 없이 입력 → 출력으로 패스스루.
  fn passthrough(&self, runtime: &Runtime,
                 state: &RuntimeState)
                 -> Result<BTreeMap<String, AudioBuffer>, String> {
    let mut incoming_samples: Vec<f32> = Vec::new();
    let mut proto: Option<AudioBuffer> = None;

    for edge in &runtime.edges {
      if edge.to == self.id {
        if let Some(buf) = state.edge_values.get(&edge.id) {
          incoming_samples.extend_from_slice(&buf.samples);
          if proto.is_none() {
            proto = Some(buf.clone());
          }
        }
      }
    }

    let mut output = BTreeMap::new();
    if let Some(p) = proto {
      if !incoming_samples.is_empty() {
        let out_buf =
          AudioBuffer::new(incoming_samples, p.channels, p.sample_rate, p.bits_per_sample);
        for edge in &runtime.edges {
          if edge.from == self.id {
            output.insert(edge.id.clone(), out_buf.clone());
          }
        }
      }
    }

    Ok(output)
  }
}

// ---------------------------------------------------------------------------
// Plugin scanning
// ---------------------------------------------------------------------------

/// 시스템 VST3 플러그인 디렉터리를 스캔한다.
///
/// GetPluginFactory를 호출하여 실제 플러그인 이름/벤더 정보를 읽는다.
/// DLL 로드 실패 시 파일명 기반 정보로 폴백한다.
pub fn scan_vst3_plugins() -> Vec<VstPluginInfo> {
  let mut results = Vec::new();

  let mut scan_dirs = vec![std::path::PathBuf::from(r"C:\Program Files\Common Files\VST3")];
  if let Ok(local) = std::env::var("LOCALAPPDATA") {
    scan_dirs.push(std::path::PathBuf::from(local).join("Programs")
                                                   .join("Common")
                                                   .join("VST3"));
  }

  for dir in scan_dirs {
    if dir.exists() {
      scan_vst3_dir(&dir, &mut results);
    }
  }

  results
}

fn scan_vst3_dir(dir: &std::path::Path, results: &mut Vec<VstPluginInfo>) {
  let entries = match std::fs::read_dir(dir) {
    Ok(e) => e,
    Err(_) => return,
  };

  for entry in entries.flatten() {
    let path = entry.path();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !ext.eq_ignore_ascii_case("vst3") {
      continue;
    }

    let dll_path = if path.is_dir() {
      let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
      let c = path.join("Contents").join("x86_64-win").join(format!("{}.vst3", stem));
      if c.exists() { c } else { continue }
    } else {
      path.clone()
    };

    let fallback_name = dll_path.file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Unknown")
                                .to_string();
    let dll_str = dll_path.to_string_lossy().into_owned();

    match scan_single_dll(&dll_str, &fallback_name) {
      Ok(info) => results.push(info),
      Err(_) => {
        results.push(VstPluginInfo { name: fallback_name,
                                     path: dll_str,
                                     vendor: String::new(),
                                     num_inputs: 1,
                                     num_outputs: 1,
                                     num_params: 0 });
      }
    }
  }
}

/// 단일 DLL을 로드하여 GetPluginFactory로 플러그인 정보를 읽는다.
fn scan_single_dll(dll_path: &str, fallback_name: &str) -> Result<VstPluginInfo, String> {
  unsafe {
    let lib = libloading::Library::new(dll_path)
      .map_err(|e| format!("DLL 로드 실패: {e}"))?;

    let get_factory: libloading::Symbol<vst3_com::GetPluginFactoryFn> =
      lib.get(b"GetPluginFactory\0")
         .map_err(|e| format!("심볼 없음: {e}"))?;
    let factory = get_factory();
    if factory.is_null() {
      return Err("factory null".to_string());
    }
    let factory = &mut *factory;

    let vendor = factory.get_factory_info()
                        .map(|fi| vst3_com::cchar_to_string(&fi.vendor))
                        .unwrap_or_default();

    let num_classes = factory.count_classes();
    let mut plugin_name = fallback_name.to_string();
    let num_inputs: u16 = 1;
    let num_outputs: u16 = 1;
    let num_params: u32 = 0;

    for i in 0..num_classes {
      if let Some(info) = factory.get_class_info(i) {
        let cat = vst3_com::cchar_to_string(&info.category);
        if cat.starts_with("Audio Module Class") {
          let name = vst3_com::cchar_to_string(&info.name);
          if !name.is_empty() {
            plugin_name = name;
          }
          // 더 정확한 입출력 채널은 IComponent를 생성해야 알 수 있다.
          // 스캔 성능을 위해 기본값 유지.
          let _ = (num_inputs, num_outputs, num_params);
          break;
        }
      }
    }

    Ok(VstPluginInfo { name: plugin_name,
                       path: dll_path.to_string(),
                       vendor,
                       num_inputs,
                       num_outputs,
                       num_params })
  }
}
