// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
  // ---------------------------------------------------------------------------
  // Elevated helper mode: cable-ui.exe --rename-endpoint <endpoint_id> <name>
  //
  // When the main app needs to write PKEY_Device_DeviceDesc it re-launches
  // itself with these arguments via ShellExecute "runas".  This sub-process
  // runs elevated, performs the COM property write, and exits immediately —
  // the Tauri window is never opened.
  // ---------------------------------------------------------------------------
  #[cfg(windows)]
  {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 4 && args[1] == "--rename-endpoint" {
      let endpoint_id = &args[2];
      // The name may contain spaces; remaining args are joined with a space.
      let new_name = args[3..].join(" ");
      match ui::rename_endpoint_elevated(endpoint_id, &new_name) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
          eprintln!("rename-endpoint failed: {}", e);
          std::process::exit(1);
        }
      }
    }
  }

  ui::run();
}
