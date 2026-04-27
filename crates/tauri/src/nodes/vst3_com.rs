//! VST3 COM 인터페이스 vtable 정의.
//!
//! Windows x64에서 VST3 DLL은 `GetPluginFactory()` 심볼을 내보내며,
//! 반환된 IPluginFactory 포인터를 통해 IComponent / IAudioProcessor /
//! IEditController / IPlugView 인터페이스를 생성한다.
//!
//! 모든 구조체는 `#[repr(C)]`로 선언되어 C ABI와 정확히 일치한다.
//! vtable 함수 포인터는 사용하지 않는 메서드도 전부 포함해야 순서가 맞는다.
#![allow(dead_code)]

use std::ffi::c_void;

// ---------------------------------------------------------------------------
// 결과 코드
// ---------------------------------------------------------------------------

pub const K_RESULT_OK: i32 = 0;
pub const K_NOT_IMPLEMENTED: i32 = 0x80004001u32 as i32;
pub const K_NO_INTERFACE: i32 = 0x80004002u32 as i32;
pub const K_RESULT_FALSE: i32 = 1;

// ---------------------------------------------------------------------------
// 상수
// ---------------------------------------------------------------------------

/// kAudio bus type
pub const K_AUDIO: i32 = 0;
/// kEvent bus type
pub const K_EVENT: i32 = 1;
/// kInput direction
pub const K_INPUT: i32 = 0;
/// kOutput direction
pub const K_OUTPUT: i32 = 1;
/// kRealtime process mode
pub const K_REALTIME: i32 = 0;
/// kSample32 symbolic sample size
pub const K_SAMPLE32: i32 = 0;
/// Stereo speaker arrangement (kSpeakerL | kSpeakerR)
pub const K_STEREO: u64 = 0x03;
/// Mono speaker arrangement (kSpeakerM, bit 19)
pub const K_MONO: u64 = 1 << 19;
/// Audio Module Class category string
pub const K_VA_TYPE_AUDIO: &[u8] = b"Audio Module Class\0";

// ---------------------------------------------------------------------------
// Interface IDs — Windows COM 호환 바이트 순서
//
// VST3 SDK는 Windows에서 COM 호환 GUID 포맷을 사용한다.
// DECLARE_CLASS_IID(Foo, l1, l2, l3, l4) 로부터 변환:
//   bytes  0-3  : l1 little-endian
//   bytes  4-5  : (l2 >> 16) little-endian
//   bytes  6-7  : (l2 & 0xFFFF) little-endian
//   bytes  8-15 : l3, l4 big-endian
// ---------------------------------------------------------------------------

/// DECLARE_CLASS_IID의 4-uint32 값을 Windows COM GUID 바이트 순서로 변환한다.
const fn vst3_iid(l1: u32, l2: u32, l3: u32, l4: u32) -> [u8; 16] {
  [
    (l1 & 0xFF) as u8,
    ((l1 >> 8) & 0xFF) as u8,
    ((l1 >> 16) & 0xFF) as u8,
    ((l1 >> 24) & 0xFF) as u8,
    ((l2 >> 16) & 0xFF) as u8,
    ((l2 >> 24) & 0xFF) as u8,
    (l2 & 0xFF) as u8,
    ((l2 >> 8) & 0xFF) as u8,
    ((l3 >> 24) & 0xFF) as u8,
    ((l3 >> 16) & 0xFF) as u8,
    ((l3 >> 8) & 0xFF) as u8,
    (l3 & 0xFF) as u8,
    ((l4 >> 24) & 0xFF) as u8,
    ((l4 >> 16) & 0xFF) as u8,
    ((l4 >> 8) & 0xFF) as u8,
    (l4 & 0xFF) as u8,
  ]
}

// IPluginFactory : DECLARE_CLASS_IID(IPluginFactory, 0x7A4D811C, 0x52114A1F, 0xAED9D2EE, 0x0B615AA8)
pub const IID_IPLUGIN_FACTORY: [u8; 16] = vst3_iid(0x7A4D811C, 0x52114A1F, 0xAED9D2EE, 0x0B615AA8);
// IComponent : DECLARE_CLASS_IID(IComponent, 0xE831FF31, 0xF2D54301, 0x928EBBEE, 0x25697802)
pub const IID_ICOMPONENT: [u8; 16] = vst3_iid(0xE831FF31, 0xF2D54301, 0x928EBBEE, 0x25697802);
// IAudioProcessor : DECLARE_CLASS_IID(IAudioProcessor, 0x42043F99, 0xB7DA453C, 0xA569E79D, 0x9AAEC33D)
pub const IID_IAUDIO_PROCESSOR: [u8; 16] = vst3_iid(0x42043F99, 0xB7DA453C, 0xA569E79D, 0x9AAEC33D);
// IEditController : DECLARE_CLASS_IID(IEditController, 0xDCD7BBE3, 0x7742448D, 0xA874AACC, 0x979C759E)
pub const IID_IEDIT_CONTROLLER: [u8; 16] = vst3_iid(0xDCD7BBE3, 0x7742448D, 0xA874AACC, 0x979C759E);
// IPlugView : DECLARE_CLASS_IID(IPlugView, 0x5BC32507, 0xD060049E, 0xA6151B52, 0x2B755B29)
pub const IID_IPLUG_VIEW: [u8; 16] = vst3_iid(0x5BC32507, 0xD060049E, 0xA6151B52, 0x2B755B29);

// ---------------------------------------------------------------------------
// 공통 데이터 구조체
// ---------------------------------------------------------------------------

/// IPluginFactory::getFactoryInfo() 결과.
#[repr(C)]
pub struct PFactoryInfo {
  pub vendor: [u8; 64],
  pub url: [u8; 256],
  pub email: [u8; 128],
  pub flags: i32,
}

impl Default for PFactoryInfo {
  fn default() -> Self {
    unsafe { std::mem::zeroed() }
  }
}

/// IPluginFactory::getClassInfo() 결과.
#[repr(C)]
pub struct PClassInfo {
  pub cid: [u8; 16],
  pub cardinality: i32,
  pub category: [u8; 32],
  pub name: [u8; 64],
}

impl Default for PClassInfo {
  fn default() -> Self {
    unsafe { std::mem::zeroed() }
  }
}

/// IAudioProcessor::setupProcessing() 파라미터.
/// C 레이아웃:
///   offset  0: process_mode (i32, 4)
///   offset  4: symbolic_sample_size (i32, 4)
///   offset  8: max_samples_per_block (i32, 4)
///   offset 12: padding (4)
///   offset 16: sample_rate (f64, 8)
///   size: 24
#[repr(C)]
pub struct ProcessSetup {
  pub process_mode: i32,
  pub symbolic_sample_size: i32,
  pub max_samples_per_block: i32,
  _pad: i32,
  pub sample_rate: f64,
}

impl ProcessSetup {
  pub fn new(process_mode: i32, symbolic_sample_size: i32, max_samples: i32,
             sample_rate: f64)
             -> Self {
    Self { process_mode, symbolic_sample_size, max_samples_per_block: max_samples, _pad: 0,
           sample_rate }
  }
}

/// AudioBusBuffers — ProcessData 에 들어가는 오디오 버스 버퍼 서술자.
/// C 레이아웃 (64-bit):
///   offset  0: num_channels (i32, 4)
///   offset  4: padding (4)
///   offset  8: silence_flags (u64, 8)
///   offset 16: channel_buffers32 (*mut *mut f32, 8)
///   size: 24
#[repr(C)]
pub struct AudioBusBuffers {
  pub num_channels: i32,
  _pad: i32,
  pub silence_flags: u64,
  pub channel_buffers32: *mut *mut f32,
}

impl AudioBusBuffers {
  pub fn new(num_channels: i32, silence_flags: u64, channel_buffers32: *mut *mut f32) -> Self {
    Self { num_channels, _pad: 0, silence_flags, channel_buffers32 }
  }
}

/// ProcessData — IAudioProcessor::process() 에 전달되는 구조체.
/// C 레이아웃 (64-bit):
///   offset  0: process_mode (i32, 4)
///   offset  4: symbolic_sample_size (i32, 4)
///   offset  8: num_samples (i32, 4)
///   offset 12: num_inputs (i32, 4)
///   offset 16: num_outputs (i32, 4)
///   offset 20: padding (4)
///   offset 24: inputs (*mut AudioBusBuffers, 8)
///   offset 32: outputs (*mut AudioBusBuffers, 8)
///   ... 5개 추가 포인터 (각 8바이트)
///   size: 80
#[repr(C)]
pub struct ProcessData {
  pub process_mode: i32,
  pub symbolic_sample_size: i32,
  pub num_samples: i32,
  pub num_inputs: i32,
  pub num_outputs: i32,
  _pad: i32,
  pub inputs: *mut AudioBusBuffers,
  pub outputs: *mut AudioBusBuffers,
  pub input_param_changes: *mut c_void,
  pub output_param_changes: *mut c_void,
  pub input_events: *mut c_void,
  pub output_events: *mut c_void,
  pub process_context: *mut c_void,
}

impl ProcessData {
  pub fn new(num_samples: i32, inputs: *mut AudioBusBuffers, num_inputs: i32,
             outputs: *mut AudioBusBuffers, num_outputs: i32)
             -> Self {
    Self { process_mode: K_REALTIME,
           symbolic_sample_size: K_SAMPLE32,
           num_samples,
           num_inputs,
           num_outputs,
           _pad: 0,
           inputs,
           outputs,
           input_param_changes: std::ptr::null_mut(),
           output_param_changes: std::ptr::null_mut(),
           input_events: std::ptr::null_mut(),
           output_events: std::ptr::null_mut(),
           process_context: std::ptr::null_mut() }
  }
}

/// IPlugView::getSize() / onSize() 에 사용되는 뷰 크기.
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct ViewRect {
  pub left: i32,
  pub top: i32,
  pub right: i32,
  pub bottom: i32,
}

impl ViewRect {
  pub fn width(&self) -> i32 {
    self.right - self.left
  }

  pub fn height(&self) -> i32 {
    self.bottom - self.top
  }
}

/// IEditController::getParameterInfo() 결과.
/// 크기: 4 + 256 + 256 + 256 + 4 + 8 + 4 + 4 = 792 bytes
#[repr(C)]
pub struct ParameterInfo {
  pub id: u32,
  pub title: [i16; 128],
  pub short_title: [i16; 128],
  pub units: [i16; 128],
  pub step_count: i32,
  pub default_normalized_value: f64,
  pub unit_id: i32,
  pub flags: i32,
}

impl Default for ParameterInfo {
  fn default() -> Self {
    unsafe { std::mem::zeroed() }
  }
}

// ---------------------------------------------------------------------------
// UTF-16 문자열 헬퍼
// ---------------------------------------------------------------------------

/// VST3 String128 (i16[128]) → Rust String 변환.
pub fn wchar_to_string(chars: &[i16]) -> String {
  let u16s: Vec<u16> = chars.iter().take_while(|&&c| c != 0).map(|&c| c as u16).collect();
  String::from_utf16_lossy(&u16s)
}

/// C 스타일 ASCII 바이트 배열 → Rust String 변환.
pub fn cchar_to_string(bytes: &[u8]) -> String {
  let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
  String::from_utf8_lossy(&bytes[..end]).into_owned()
}

// ---------------------------------------------------------------------------
// FUnknown vtable — 모든 VST3 인터페이스의 기반
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct FUnknownVtbl {
  pub query_interface:
    unsafe extern "system" fn(*mut FUnknown, iid: *const u8, obj: *mut *mut c_void) -> i32,
  pub add_ref: unsafe extern "system" fn(*mut FUnknown) -> u32,
  pub release: unsafe extern "system" fn(*mut FUnknown) -> u32,
}

#[repr(C)]
pub struct FUnknown {
  pub vtable: *const FUnknownVtbl,
}

impl FUnknown {
  pub unsafe fn query_interface(&mut self, iid: &[u8; 16]) -> Option<*mut c_void> {
    let mut obj: *mut c_void = std::ptr::null_mut();
    if ((*self.vtable).query_interface)(self, iid.as_ptr(), &mut obj) == K_RESULT_OK
       && !obj.is_null()
    {
      Some(obj)
    } else {
      None
    }
  }

  pub unsafe fn release(&mut self) -> u32 {
    ((*self.vtable).release)(self)
  }
}

// ---------------------------------------------------------------------------
// IPluginFactory vtable
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct IPluginFactoryVtbl {
  pub query_interface:
    unsafe extern "system" fn(*mut IPluginFactory, *const u8, *mut *mut c_void) -> i32,
  pub add_ref: unsafe extern "system" fn(*mut IPluginFactory) -> u32,
  pub release: unsafe extern "system" fn(*mut IPluginFactory) -> u32,
  pub get_factory_info:
    unsafe extern "system" fn(*mut IPluginFactory, *mut PFactoryInfo) -> i32,
  pub count_classes: unsafe extern "system" fn(*mut IPluginFactory) -> i32,
  pub get_class_info:
    unsafe extern "system" fn(*mut IPluginFactory, i32, *mut PClassInfo) -> i32,
  pub create_instance:
    unsafe extern "system" fn(*mut IPluginFactory, *const u8, *const u8,
                              *mut *mut c_void) -> i32,
}

#[repr(C)]
pub struct IPluginFactory {
  pub vtable: *const IPluginFactoryVtbl,
}

impl IPluginFactory {
  pub unsafe fn get_factory_info(&mut self) -> Option<PFactoryInfo> {
    let mut info = PFactoryInfo::default();
    if ((*self.vtable).get_factory_info)(self, &mut info) == K_RESULT_OK {
      Some(info)
    } else {
      None
    }
  }

  pub unsafe fn count_classes(&mut self) -> i32 {
    ((*self.vtable).count_classes)(self)
  }

  pub unsafe fn get_class_info(&mut self, index: i32) -> Option<PClassInfo> {
    let mut info = PClassInfo::default();
    if ((*self.vtable).get_class_info)(self, index, &mut info) == K_RESULT_OK {
      Some(info)
    } else {
      None
    }
  }

  /// cid와 iid로 인터페이스 인스턴스를 생성한다.
  pub unsafe fn create_instance(&mut self, cid: &[u8; 16],
                                iid: &[u8; 16])
                                -> Option<*mut c_void> {
    let mut obj: *mut c_void = std::ptr::null_mut();
    let r = ((*self.vtable).create_instance)(self, cid.as_ptr(), iid.as_ptr(), &mut obj);
    if r == K_RESULT_OK && !obj.is_null() {
      Some(obj)
    } else {
      println!("create_instance 실패: result={r:#010x}, obj={obj:?}");
      None
    }
  }

  pub unsafe fn release(&mut self) -> u32 {
    ((*self.vtable).release)(self)
  }
}

// ---------------------------------------------------------------------------
// IComponent vtable (extends IPluginBase, extends FUnknown)
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct IComponentVtbl {
  // FUnknown
  pub query_interface:
    unsafe extern "system" fn(*mut IComponent, *const u8, *mut *mut c_void) -> i32,
  pub add_ref: unsafe extern "system" fn(*mut IComponent) -> u32,
  pub release: unsafe extern "system" fn(*mut IComponent) -> u32,
  // IPluginBase
  pub initialize: unsafe extern "system" fn(*mut IComponent, *mut c_void) -> i32,
  pub terminate: unsafe extern "system" fn(*mut IComponent) -> i32,
  // IComponent
  pub get_controller_class_id:
    unsafe extern "system" fn(*mut IComponent, *mut [u8; 16]) -> i32,
  pub set_io_mode: unsafe extern "system" fn(*mut IComponent, i32) -> i32,
  pub get_bus_count: unsafe extern "system" fn(*mut IComponent, i32, i32) -> i32,
  pub get_bus_info: unsafe extern "system" fn(*mut IComponent, i32, i32, i32,
                                              *mut c_void) -> i32,
  pub get_routing_info:
    unsafe extern "system" fn(*mut IComponent, *mut c_void, *mut c_void) -> i32,
  pub activate_bus: unsafe extern "system" fn(*mut IComponent, i32, i32, i32, u8) -> i32,
  pub set_active: unsafe extern "system" fn(*mut IComponent, u8) -> i32,
  pub set_state: unsafe extern "system" fn(*mut IComponent, *mut c_void) -> i32,
  pub get_state: unsafe extern "system" fn(*mut IComponent, *mut c_void) -> i32,
}

#[repr(C)]
pub struct IComponent {
  pub vtable: *const IComponentVtbl,
}

impl IComponent {
  pub unsafe fn query_interface(&mut self, iid: &[u8; 16]) -> Option<*mut c_void> {
    let mut obj: *mut c_void = std::ptr::null_mut();
    if ((*self.vtable).query_interface)(self, iid.as_ptr(), &mut obj) == K_RESULT_OK
       && !obj.is_null()
    {
      Some(obj)
    } else {
      None
    }
  }

  pub unsafe fn initialize(&mut self, host_context: *mut c_void) -> i32 {
    ((*self.vtable).initialize)(self, host_context)
  }

  pub unsafe fn terminate(&mut self) -> i32 {
    ((*self.vtable).terminate)(self)
  }

  pub unsafe fn get_controller_class_id(&mut self) -> Option<[u8; 16]> {
    let mut cid = [0u8; 16];
    if ((*self.vtable).get_controller_class_id)(self, &mut cid) == K_RESULT_OK {
      Some(cid)
    } else {
      None
    }
  }

  pub unsafe fn activate_bus(&mut self, media_type: i32, dir: i32, index: i32,
                             state: bool)
                             -> i32 {
    ((*self.vtable).activate_bus)(self, media_type, dir, index, state as u8)
  }

  pub unsafe fn set_active(&mut self, state: bool) -> i32 {
    ((*self.vtable).set_active)(self, state as u8)
  }

  pub unsafe fn release(&mut self) -> u32 {
    ((*self.vtable).release)(self)
  }
}

// ---------------------------------------------------------------------------
// IAudioProcessor vtable (extends FUnknown directly)
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct IAudioProcessorVtbl {
  // FUnknown
  pub query_interface:
    unsafe extern "system" fn(*mut IAudioProcessor, *const u8, *mut *mut c_void) -> i32,
  pub add_ref: unsafe extern "system" fn(*mut IAudioProcessor) -> u32,
  pub release: unsafe extern "system" fn(*mut IAudioProcessor) -> u32,
  // IAudioProcessor
  pub set_bus_arrangements:
    unsafe extern "system" fn(*mut IAudioProcessor, *mut u64, i32, *mut u64, i32) -> i32,
  pub get_bus_arrangement:
    unsafe extern "system" fn(*mut IAudioProcessor, i32, i32, *mut u64) -> i32,
  pub can_process_sample_size:
    unsafe extern "system" fn(*mut IAudioProcessor, i32) -> i32,
  pub get_latency_samples: unsafe extern "system" fn(*mut IAudioProcessor) -> u32,
  pub setup_processing:
    unsafe extern "system" fn(*mut IAudioProcessor, *const ProcessSetup) -> i32,
  pub set_processing: unsafe extern "system" fn(*mut IAudioProcessor, u8) -> i32,
  pub process: unsafe extern "system" fn(*mut IAudioProcessor, *mut ProcessData) -> i32,
  pub get_tail_samples: unsafe extern "system" fn(*mut IAudioProcessor) -> u32,
}

#[repr(C)]
pub struct IAudioProcessor {
  pub vtable: *const IAudioProcessorVtbl,
}

impl IAudioProcessor {
  pub unsafe fn set_bus_arrangements(&mut self, inputs: &mut [u64],
                                     outputs: &mut [u64])
                                     -> i32 {
    ((*self.vtable).set_bus_arrangements)(self,
                                          inputs.as_mut_ptr(),
                                          inputs.len() as i32,
                                          outputs.as_mut_ptr(),
                                          outputs.len() as i32)
  }

  pub unsafe fn setup_processing(&mut self, setup: &ProcessSetup) -> i32 {
    ((*self.vtable).setup_processing)(self, setup)
  }

  pub unsafe fn set_processing(&mut self, state: bool) -> i32 {
    ((*self.vtable).set_processing)(self, state as u8)
  }

  pub unsafe fn process(&mut self, data: &mut ProcessData) -> i32 {
    ((*self.vtable).process)(self, data)
  }

  pub unsafe fn release(&mut self) -> u32 {
    ((*self.vtable).release)(self)
  }
}

// ---------------------------------------------------------------------------
// IEditController vtable (extends IPluginBase)
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct IEditControllerVtbl {
  // FUnknown
  pub query_interface:
    unsafe extern "system" fn(*mut IEditController, *const u8, *mut *mut c_void) -> i32,
  pub add_ref: unsafe extern "system" fn(*mut IEditController) -> u32,
  pub release: unsafe extern "system" fn(*mut IEditController) -> u32,
  // IPluginBase
  pub initialize: unsafe extern "system" fn(*mut IEditController, *mut c_void) -> i32,
  pub terminate: unsafe extern "system" fn(*mut IEditController) -> i32,
  // IEditController
  pub set_component_state:
    unsafe extern "system" fn(*mut IEditController, *mut c_void) -> i32,
  pub set_state: unsafe extern "system" fn(*mut IEditController, *mut c_void) -> i32,
  pub get_state: unsafe extern "system" fn(*mut IEditController, *mut c_void) -> i32,
  pub get_parameter_count: unsafe extern "system" fn(*mut IEditController) -> i32,
  pub get_parameter_info:
    unsafe extern "system" fn(*mut IEditController, i32, *mut ParameterInfo) -> i32,
  pub get_param_string_by_value:
    unsafe extern "system" fn(*mut IEditController, u32, f64, *mut i16) -> i32,
  pub get_param_value_by_string:
    unsafe extern "system" fn(*mut IEditController, u32, *const i16, *mut f64) -> i32,
  pub normalized_param_to_plain:
    unsafe extern "system" fn(*mut IEditController, u32, f64) -> f64,
  pub plain_param_to_normalized:
    unsafe extern "system" fn(*mut IEditController, u32, f64) -> f64,
  pub get_param_normalized: unsafe extern "system" fn(*mut IEditController, u32) -> f64,
  pub set_param_normalized:
    unsafe extern "system" fn(*mut IEditController, u32, f64) -> i32,
  pub set_component_handler:
    unsafe extern "system" fn(*mut IEditController, *mut c_void) -> i32,
  pub create_view:
    unsafe extern "system" fn(*mut IEditController, *const i8) -> *mut IPlugView,
}

#[repr(C)]
pub struct IEditController {
  pub vtable: *const IEditControllerVtbl,
}

impl IEditController {
  pub unsafe fn initialize(&mut self, host_context: *mut c_void) -> i32 {
    ((*self.vtable).initialize)(self, host_context)
  }

  pub unsafe fn terminate(&mut self) -> i32 {
    ((*self.vtable).terminate)(self)
  }

  pub unsafe fn get_parameter_count(&mut self) -> i32 {
    ((*self.vtable).get_parameter_count)(self)
  }

  pub unsafe fn get_parameter_info(&mut self, index: i32) -> Option<ParameterInfo> {
    let mut info = ParameterInfo::default();
    if ((*self.vtable).get_parameter_info)(self, index, &mut info) == K_RESULT_OK {
      Some(info)
    } else {
      None
    }
  }

  pub unsafe fn get_param_normalized(&mut self, id: u32) -> f64 {
    ((*self.vtable).get_param_normalized)(self, id)
  }

  pub unsafe fn set_param_normalized(&mut self, id: u32, value: f64) -> i32 {
    ((*self.vtable).set_param_normalized)(self, id, value)
  }

  /// "editor" 뷰 생성.
  pub unsafe fn create_view(&mut self) -> Option<*mut IPlugView> {
    let name = b"editor\0";
    let ptr = ((*self.vtable).create_view)(self, name.as_ptr() as *const i8);
    if ptr.is_null() { None } else { Some(ptr) }
  }

  pub unsafe fn release(&mut self) -> u32 {
    ((*self.vtable).release)(self)
  }
}

// ---------------------------------------------------------------------------
// IPlugView vtable (extends FUnknown)
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct IPlugViewVtbl {
  // FUnknown
  pub query_interface:
    unsafe extern "system" fn(*mut IPlugView, *const u8, *mut *mut c_void) -> i32,
  pub add_ref: unsafe extern "system" fn(*mut IPlugView) -> u32,
  pub release: unsafe extern "system" fn(*mut IPlugView) -> u32,
  // IPlugView
  pub is_platform_type_supported:
    unsafe extern "system" fn(*mut IPlugView, *const i8) -> i32,
  pub attached: unsafe extern "system" fn(*mut IPlugView, *mut c_void, *const i8) -> i32,
  pub removed: unsafe extern "system" fn(*mut IPlugView) -> i32,
  pub on_wheel: unsafe extern "system" fn(*mut IPlugView, f32) -> i32,
  pub on_key_down: unsafe extern "system" fn(*mut IPlugView, i16, i16, i16) -> i32,
  pub on_key_up: unsafe extern "system" fn(*mut IPlugView, i16, i16, i16) -> i32,
  pub get_size: unsafe extern "system" fn(*mut IPlugView, *mut ViewRect) -> i32,
  pub on_size: unsafe extern "system" fn(*mut IPlugView, *const ViewRect) -> i32,
  pub on_focus: unsafe extern "system" fn(*mut IPlugView, u8) -> i32,
  pub set_frame: unsafe extern "system" fn(*mut IPlugView, *mut c_void) -> i32,
  pub can_resize: unsafe extern "system" fn(*mut IPlugView) -> i32,
  pub check_size_constraint: unsafe extern "system" fn(*mut IPlugView, *mut ViewRect) -> i32,
}

#[repr(C)]
pub struct IPlugView {
  pub vtable: *const IPlugViewVtbl,
}

impl IPlugView {
  pub unsafe fn is_platform_type_supported(&mut self, type_name: &[u8]) -> i32 {
    ((*self.vtable).is_platform_type_supported)(self, type_name.as_ptr() as *const i8)
  }

  pub unsafe fn attached(&mut self, parent: *mut c_void, type_name: &[u8]) -> i32 {
    ((*self.vtable).attached)(self, parent, type_name.as_ptr() as *const i8)
  }

  pub unsafe fn removed(&mut self) -> i32 {
    ((*self.vtable).removed)(self)
  }

  pub unsafe fn get_size(&mut self) -> Option<ViewRect> {
    let mut rect = ViewRect::default();
    if ((*self.vtable).get_size)(self, &mut rect) == K_RESULT_OK {
      Some(rect)
    } else {
      None
    }
  }

  pub unsafe fn release(&mut self) -> u32 {
    ((*self.vtable).release)(self)
  }
}

// ---------------------------------------------------------------------------
// GetPluginFactory 함수 타입
// ---------------------------------------------------------------------------

/// VST3 DLL에서 내보내는 `GetPluginFactory` 함수 타입.
pub type GetPluginFactoryFn = unsafe extern "C" fn() -> *mut IPluginFactory;
