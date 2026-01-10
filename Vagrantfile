# -*- mode: ruby -*-
# vi: set ft=ruby :

Vagrant.configure("2") do |config|
  # Windows 10 image for driver testing
  config.vm.box = "gusztavvargadr/windows-10"

  config.vm.provider "hyperv" do |h|
    h.memory = "4096"
    h.cpus = 2
    h.linked_clone = true # Linked clone setting to save disk space
  end

  config.vm.synced_folder "./target/debug", "/driver_test", disabled: false,
    smb_password: "vagrant", smb_username: "sier",
    mount_options: ["username=sier", "password=vagrant"]

  config.vm.provision "shell", inline: <<-SHELL
    bcdedit /set testsigning on
    
    reg add "HKLM\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Debug Print Filter" /v DEFAULT /t REG_DWORD /d 0xf /f
    
    Write-Host "Test environment setup complete. A reboot may be required."
  SHELL
end