// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
  // ---------------------------------------------------------------------------
  // Elevated helper mode: Cable.exe --rename-endpoint <endpoint_id> <name>
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

    // -----------------------------------------------------------------------
    // Elevated helper mode: Cable.exe --set-endpoint-format <endpoint_id>
    //                                   <sample_rate> <channels> <bits>
    //
    // Writes PKEY_AudioEngine_DeviceFormat via IPropertyStore so Windows Audio
    // Engine uses the requested format when opening the virtual device.
    // -----------------------------------------------------------------------
    if args.len() >= 6 && args[1] == "--set-endpoint-format" {
      let endpoint_id = &args[2];
      let sample_rate = args[3].parse::<u32>().unwrap_or(48000);
      let channels = args[4].parse::<u16>().unwrap_or(2);
      let bits_per_sample = args[5].parse::<u16>().unwrap_or(32);
      match ui::set_endpoint_format_elevated(endpoint_id, sample_rate, channels, bits_per_sample) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
          eprintln!("set-endpoint-format failed: {}", e);
          std::process::exit(1);
        }
      }
    }
  }

  // -------------------------------------------------------------------------
  // Headless RPC mode: Cable.exe --headless [port]
  //
  // Starts a local HTTP server on 127.0.0.1:<port> (default 17285) that
  // exposes app control commands as JSON-over-HTTP without opening the Tauri
  // GUI window.  Used for driver integration testing from PowerShell / C#.
  // -------------------------------------------------------------------------
  {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "--headless" {
      let port = args
        .get(2)
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(17285);
      tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build Tokio runtime")
        .block_on(ui::run_headless(port));
      return;
    }
  }

  ui::run();
}
