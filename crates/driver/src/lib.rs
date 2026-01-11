#![no_std]

extern crate alloc;

#[cfg(not(test))]
extern crate wdk_panic;

use alloc::vec::Vec;
use common::{
  AudioFormat, DeviceControlPayload, IoctlRequest, IOCTL_CREATE_VIRTUAL_DEVICE,
  IOCTL_REMOVE_VIRTUAL_DEVICE, IOCTL_SET_STREAM_FORMAT, IOCTL_UPDATE_DEVICE_NAME,
};
use wdk_alloc::WdkAllocator;
use wdk_sys::{
  ntddk::{
    IoCreateDevice, IoCreateSymbolicLink, IoDeleteDevice, IoDeleteSymbolicLink, IofCompleteRequest,
  },
  DEVICE_OBJECT, DO_BUFFERED_IO, DRIVER_OBJECT, FILE_DEVICE_SECURE_OPEN, FILE_DEVICE_UNKNOWN,
  IO_STACK_LOCATION, IRP, IRP_MJ_CLOSE, IRP_MJ_CREATE, IRP_MJ_DEVICE_CONTROL, IRP_MJ_PNP,
  IRP_MN_REMOVE_DEVICE, IRP_MN_START_DEVICE, NTSTATUS, STATUS_INVALID_DEVICE_REQUEST,
  STATUS_SUCCESS, UNICODE_STRING,
};

/// Get the current IRP stack location
unsafe fn io_get_current_irp_stack_location(irp: *mut IRP) -> *mut IO_STACK_LOCATION {
  let irp_ref = &*irp;
  irp_ref
    .Tail
    .Overlay
    .__bindgen_anon_2
    .__bindgen_anon_1
    .CurrentStackLocation
}

#[cfg(not(test))]
#[global_allocator]
static GLOBAL_ALLOCATOR: WdkAllocator = WdkAllocator;

/// 장치 이름과 심볼릭 링크 이름 정의
const DEVICE_NAME: &str = "\\Device\\CableAudioBus";
const SYMLINK_NAME: &str = "\\DosDevices\\CableAudioBus";

/// 유니코드 문자열 생성을 위한 헬퍼
fn create_unicode_string(s: &[u16]) -> UNICODE_STRING {
  let len = (s.len() * 2) as u16;
  UNICODE_STRING {
    Length: len,
    MaximumLength: len,
    Buffer: s.as_ptr() as *mut _,
  }
}

/// 유니코드 문자열 버퍼 (생명주기 관리용)
struct UnicodeStringWrapper {
  _buffer: Vec<u16>,
  unicode_string: UNICODE_STRING,
}

impl UnicodeStringWrapper {
  fn new(s: &str) -> Self {
    let mut buffer: Vec<u16> = s.encode_utf16().collect();
    buffer.push(0); // Null entry
                    // 유니코드 스트링 버퍼는 널 문자를 포함하지 않는 길이를 사용합니다.
    let unicode_string = create_unicode_string(&buffer[..buffer.len() - 1]);
    Self {
      _buffer: buffer,
      unicode_string,
    }
  }
}

/// Driver Entry Point
#[export_name = "DriverEntry"]
pub unsafe extern "system" fn driver_entry(
  driver_object: &mut DRIVER_OBJECT,
  _registry_path: &UNICODE_STRING,
) -> NTSTATUS {
  wdk::println!("CableAudioBus: DriverEntry Called");

  // 기본 Unload 설정
  driver_object.DriverUnload = Some(driver_unload);

  // Major Function 설정
  driver_object.MajorFunction[IRP_MJ_CREATE as usize] = Some(dispatch_create_close);
  driver_object.MajorFunction[IRP_MJ_CLOSE as usize] = Some(dispatch_create_close);
  driver_object.MajorFunction[IRP_MJ_DEVICE_CONTROL as usize] = Some(dispatch_device_control);

  // PnP 핸들러 설정
  driver_object.DriverExtension.as_mut().unwrap().AddDevice = Some(add_device);
  driver_object.MajorFunction[IRP_MJ_PNP as usize] = Some(dispatch_pnp);

  STATUS_SUCCESS
}

/// Unload Callback
unsafe extern "C" fn driver_unload(_driver_object: *mut DRIVER_OBJECT) {
  wdk::println!("CableAudioBus: Driver Unload");
}

/// AddDevice Callback (PnP)
unsafe extern "C" fn add_device(
  driver_object: *mut DRIVER_OBJECT,
  _physical_device_object: *mut DEVICE_OBJECT,
) -> NTSTATUS {
  wdk::println!("CableAudioBus: AddDevice Called");

  let name_wrapper = UnicodeStringWrapper::new(DEVICE_NAME);
  let mut name_us = name_wrapper.unicode_string;

  let mut device_object: *mut DEVICE_OBJECT = core::ptr::null_mut();

  let status = IoCreateDevice(
    driver_object,
    0, // Device ext size
    &mut name_us,
    FILE_DEVICE_UNKNOWN,
    FILE_DEVICE_SECURE_OPEN,
    0, // Not exclusive
    &mut device_object,
  );

  if status != STATUS_SUCCESS {
    wdk::println!("CableAudioBus: IoCreateDevice Failed: {:#x}", status);
    return status;
  }

  // 버퍼드 I/O 사용
  (*device_object).Flags |= DO_BUFFERED_IO;

  // 심볼릭 링크 생성
  let link_wrapper = UnicodeStringWrapper::new(SYMLINK_NAME);
  let mut link_us = link_wrapper.unicode_string;
  let link_status = IoCreateSymbolicLink(&mut link_us, &mut name_us);

  if link_status != STATUS_SUCCESS {
    wdk::println!(
      "CableAudioBus: IoCreateSymbolicLink Failed: {:#x}",
      link_status
    );
    IoDeleteDevice(device_object);
    return link_status;
  }

  // 장치 초기화 완료
  (*device_object).Flags &= !wdk_sys::DO_DEVICE_INITIALIZING;

  wdk::println!("CableAudioBus: Device Created Successfully");
  STATUS_SUCCESS
}

/// Create/Close Dispatch
unsafe extern "C" fn dispatch_create_close(
  _device_object: *mut DEVICE_OBJECT,
  irp: *mut IRP,
) -> NTSTATUS {
  (*irp).IoStatus.Information = 0;
  (*irp).IoStatus.__bindgen_anon_1.Status = STATUS_SUCCESS;
  IofCompleteRequest(irp, 0);
  STATUS_SUCCESS
}

/// PnP Dispatch
unsafe extern "C" fn dispatch_pnp(device_object: *mut DEVICE_OBJECT, irp: *mut IRP) -> NTSTATUS {
  let stack = io_get_current_irp_stack_location(irp);
  let minor_function = (*stack).MinorFunction;

  match minor_function as u32 {
    IRP_MN_START_DEVICE => {
      wdk::println!("CableAudioBus: PnP Start Device");
      (*irp).IoStatus.__bindgen_anon_1.Status = STATUS_SUCCESS;
    }
    IRP_MN_REMOVE_DEVICE => {
      wdk::println!("CableAudioBus: PnP Remove Device");
      // 심볼릭 링크 해제
      let link_wrapper = UnicodeStringWrapper::new(SYMLINK_NAME);
      let mut link_us = link_wrapper.unicode_string;
      IoDeleteSymbolicLink(&mut link_us);
      IoDeleteDevice(device_object);
      (*irp).IoStatus.__bindgen_anon_1.Status = STATUS_SUCCESS;
    }
    _ => {
      // 다른 PnP 요청은 기본 성공 처리
      (*irp).IoStatus.__bindgen_anon_1.Status = STATUS_SUCCESS;
    }
  }

  let status = (*irp).IoStatus.__bindgen_anon_1.Status;
  IofCompleteRequest(irp, 0);
  status
}

/// Device Control Dispatch (IOCTL Handler)
unsafe extern "C" fn dispatch_device_control(
  _device_object: *mut DEVICE_OBJECT,
  irp: *mut IRP,
) -> NTSTATUS {
  let stack = io_get_current_irp_stack_location(irp);
  let io_control_code = (*stack).Parameters.DeviceIoControl.IoControlCode;
  let input_len = (*stack).Parameters.DeviceIoControl.InputBufferLength;

  let buffer = (*irp).AssociatedIrp.SystemBuffer as *mut IoctlRequest;

  wdk::println!("CableAudioBus: DeviceControl Code: {:#x}", io_control_code);

  let status = match io_control_code {
    IOCTL_CREATE_VIRTUAL_DEVICE => {
      if input_len as usize >= core::mem::size_of::<DeviceControlPayload>() {
        if !buffer.is_null() {
          let payload = (*buffer).device_control;
          wdk::println!("CableAudioBus: Create Virtual Device");
          // 실제 구현시엔 여기서 Child PDO 생성 등을 수행.
          STATUS_SUCCESS
        } else {
          STATUS_INVALID_DEVICE_REQUEST
        }
      } else {
        STATUS_INVALID_DEVICE_REQUEST
      }
    }
    IOCTL_REMOVE_VIRTUAL_DEVICE => {
      if !buffer.is_null() {
        let payload = (*buffer).device_control;
        wdk::println!("CableAudioBus: Remove Virtual Device");
        STATUS_SUCCESS
      } else {
        STATUS_INVALID_DEVICE_REQUEST
      }
    }
    IOCTL_UPDATE_DEVICE_NAME => {
      wdk::println!("CableAudioBus: Update Device Name");
      STATUS_SUCCESS
    }
    IOCTL_SET_STREAM_FORMAT => {
      if input_len as usize >= core::mem::size_of::<AudioFormat>() {
        let format = (*buffer).format_update;
        wdk::println!("CableAudioBus: Set Format: Rate={}", format.sample_rate);
        STATUS_SUCCESS
      } else {
        STATUS_INVALID_DEVICE_REQUEST
      }
    }
    _ => {
      wdk::println!("CableAudioBus: Unknown IOCTL");
      STATUS_INVALID_DEVICE_REQUEST
    }
  };

  (*irp).IoStatus.Information = 0;
  (*irp).IoStatus.__bindgen_anon_1.Status = status;
  IofCompleteRequest(irp, 0);
  status
}
