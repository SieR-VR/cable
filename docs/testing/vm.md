# VM Testing Environment

QEMU 기반 Windows 11 VM에서 CableAudio 드라이버와 Tauri 앱을 테스트하는 환경.

## 요구사항

- **QEMU** 10.2.0+ (`qemu-system-x86_64` in PATH)
- **WDK** 10.0.26100.0 (devcon.exe 필요)
- **호스트 OS**: Windows (WHPX 가속기 사용)
- **베이스 이미지**: `.vm/tiny11-cleaninstall.qcow2` (Windows 11 tiny11, BIOS boot)

## 디렉토리 구조

```
.vm/
  tiny11-cleaninstall.qcow2   # 베이스 이미지 (수정 안됨)
  tiny11-test.qcow2           # 오버레이 (VM 상태 보존, -Reuse시 재사용)
  staging/                    # 임시 스테이징 파일 (자동 정리됨)
  staging.iso                 # VM에 마운트되는 ISO (자동 생성/정리)
  launch.ps1                  # VM 실행 스크립트
  vm-exec.ps1                 # WinRM 원격 명령 실행
  vm-install.ps1              # 자동 드라이버 설치
  setup-host-winrm.ps1        # 호스트 WinRM 클라이언트 설정 (1회)
  ioctl-test.ps1              # IOCTL 테스트 (생성/삭제)
  ioctl-test2.ps1             # IOCTL 테스트 (확장판)
```

## 초기 설정

### 1. 호스트 WinRM 클라이언트 설정 (1회)

관리자 권한 PowerShell에서 실행:

```powershell
.vm\setup-host-winrm.ps1
```

수행하는 작업:
- WinRM 서비스 시작 (Manual 시작 유형)
- `WSMan:\localhost\Client\AllowUnencrypted` = `$true`
- `WSMan:\localhost\Client\TrustedHosts` = `127.0.0.1`

### 2. 첫 VM 실행 및 내부 설정

```powershell
.vm\launch.ps1 -Fresh
```

VM이 부팅되면 VM 내부에서 CD 드라이브 (`D:\` 또는 `E:\`)의 `setup-all.bat`을 관리자 권한으로 실행. 이 스크립트가 수행하는 작업:

- 테스트 서명 모드 활성화 (`bcdedit /set testsigning on`)
- `cable` 계정 비밀번호 설정 (`cable123`)
- WinRM 설정 (Basic 인증, 비암호화 허용, 방화벽 규칙)
- 인증서 임포트 (Root + TrustedPublisher 스토어)
- 자동 셧다운

셧다운 후 `-Reuse`로 다시 실행하면 설정이 보존됨:

```powershell
.vm\launch.ps1 -Reuse
```

## launch.ps1

VM을 실행하는 메인 스크립트.

### 파라미터

| 파라미터 | 타입 | 기본값 | 설명 |
|---------|------|--------|------|
| `-Fresh` | switch | off | 새 오버레이 생성 (기존 테스트 이미지 무시) |
| `-Reuse` | switch | off | 기존 오버레이 재사용 (WHPX 리부트 워크어라운드) |
| `-Memory` | int | 4096 | RAM (MB) |
| `-Cores` | int | 2 | CPU 코어 수 |

### 동작 순서

1. **드라이버 패키지 검증** -- `driver/x64/Debug/package/CableAudio.sys` 존재 확인
2. **스테이징 디렉토리 준비** -- `.vm/staging/<random>/`에 복사
   - 드라이버 파일: `CableAudio.sys`, `CableAudio.inf`, `cableaudio.cat`
   - 서명 인증서: `WDKTestCert.cer`
   - `devcon.exe` (WDK에서 복사)
   - 배치 스크립트 자동 생성: `install.bat`, `uninstall.bat`, `status.bat`, `fix-cert.bat`, `setup-winrm.bat`, `setup-all.bat`, `import-cert.ps1`, `setup-winrm.ps1`
3. **디스크 설정** -- `-Fresh`이면 새 qcow2 오버레이, `-Reuse`이면 기존 오버레이 재사용
4. **ISO 생성** -- IMAPI2 COM API로 Joliet ISO 생성 (볼륨 라벨: `CABLE`)
5. **QEMU 실행**
6. **정리** -- VM 종료 후 ISO 삭제, 스테이징 디렉토리 정리

### QEMU 구성

```
가속기:        whpx (Hyper-V 기반)
CPU:          Broadwell
디스크:        qcow2 (IDE), 베이스 이미지 위 오버레이
VGA:          64MB
네트워크:      e1000 NIC
               hostfwd: tcp::15985 -> :5985 (WinRM)
USB:          usb-tablet (마우스 동기화)
오디오:        dsound 백엔드 + intel-hda + hda-duplex
QMP:          tcp:127.0.0.1:14444
부팅:          하드디스크(c)
CD-ROM:       스테이징 ISO (읽기 전용)
```

### 알려진 제한사항

- **WHPX 리부트 버그**: VM 내부에서 리부트 시 `Unexpected VP exit code 4` 에러로 행. 워크어라운드: VM 셧다운 후 `-Reuse`로 재실행.
- **네트워크 프로파일**: 재부팅 후 Public으로 리셋될 수 있음. WinRM이 작동하려면 Private이어야 함.
- **NIC**: `virtio-net-pci`는 드라이버 없어서 작동 안 함. 반드시 `e1000` 사용.
- **오디오 백엔드**: QEMU 10.2.0에서 `wasapi`는 지원 안 됨. `dsound` 사용.

## 원격 명령 실행

### vm-exec.ps1

VM에서 단일 명령을 실행하는 래퍼 스크립트.

```powershell
# 기본 사용법
.vm\vm-exec.ps1 "hostname"

# 오디오 디바이스 확인
.vm\vm-exec.ps1 "Get-PnpDevice -Class AudioEndpoint | ft FriendlyName, Status"
```

| 파라미터 | 타입 | 기본값 | 설명 |
|---------|------|--------|------|
| `-Command` | string | (필수) | 실행할 명령 |
| `-Username` | string | `cable` | VM 사용자 |
| `-Password` | string | `cable123` | VM 비밀번호 |
| `-Port` | int | `15985` | 호스트측 WinRM 포트 |

인증 방식: Basic auth, `127.0.0.1` 대상, 인증서 검증 스킵.

### 직접 WinRM 세션 사용

스크립트 대신 직접 세션을 만들어 여러 명령을 실행할 수 있음:

```powershell
$cred = New-Object PSCredential("cable", (ConvertTo-SecureString "cable123" -AsPlainText -Force))
$so = New-PSSessionOption -OperationTimeout 30000 -OpenTimeout 30000 -NoMachineProfile
$session = New-PSSession -ComputerName 127.0.0.1 -Port 15985 -Credential $cred `
    -Authentication Basic -SessionOption $so

Invoke-Command -Session $session -ScriptBlock { hostname }
Invoke-Command -Session $session -ScriptBlock { Get-PnpDevice -Class MEDIA }

Remove-PSSession $session
```

## 드라이버 설치

### vm-install.ps1

WinRM을 통해 자동으로 드라이버를 설치하는 스크립트.

```powershell
# 전체 설치
.vm\vm-install.ps1

# 진단만 실행 (설치 없이)
.vm\vm-install.ps1 -DiagOnly
```

| 파라미터 | 타입 | 기본값 | 설명 |
|---------|------|--------|------|
| `-Port` | int | `15985` | WinRM 포트 |
| `-Username` | string | `cable` | VM 사용자 |
| `-Password` | string | `cable123` | VM 비밀번호 |
| `-DiagOnly` | switch | off | 진단만 실행 |

### 설치 순서

1. 테스트 서명 확인 (미활성시 활성화 후 리부트 필요)
2. 스테이징 CD 드라이브 찾기 (볼륨 라벨 `CABLE`)
3. `C:\CableAudio`로 파일 복사
4. .NET `X509Store` API로 인증서 임포트
5. `devcon.exe install CableAudio.inf ROOT\CableAudio` 실행
6. 결과 확인 (PnP 디바이스, setupapi.dev.log)

### 수동 드라이버 업데이트

이미 설치된 드라이버를 업데이트할 때는 `devcon update`를 사용:

```powershell
# 새 드라이버 파일을 VM에 복사 후:
.vm\vm-exec.ps1 "C:\CableAudio\devcon.exe update C:\CableAudio\CableAudio.inf ROOT\CableAudio"
```

`devcon install`은 새 디바이스 노드를 생성하므로, 이미 로드된 드라이버에는 `devcon update`를 사용해야 함.

## 앱 배포

### MSI를 통한 Tauri 앱 배포

```powershell
# 1. 빌드 (beforeBuildCommand로 프론트엔드도 함께 빌드됨)
pnpm tauri build

# 2. WinRM 세션으로 배포
$cred = New-Object PSCredential("cable", (ConvertTo-SecureString "cable123" -AsPlainText -Force))
$so = New-PSSessionOption -OperationTimeout 120000 -OpenTimeout 30000 -NoMachineProfile
$session = New-PSSession -ComputerName 127.0.0.1 -Port 15985 -Credential $cred `
    -Authentication Basic -SessionOption $so

# 기존 앱 종료
Invoke-Command -Session $session -ScriptBlock {
    Get-CimInstance Win32_Process -Filter "Name='cable-tauri.exe'" |
        Invoke-CimMethod -MethodName Terminate
}

# MSI 복사
Copy-Item "target\release\bundle\msi\cable-ui_0.1.0_x64_en-US.msi" `
    -Destination "C:\CableAudio\cable-ui.msi" -ToSession $session -Force

# 설치 (사일런트)
Invoke-Command -Session $session -ScriptBlock {
    Start-Process msiexec.exe -ArgumentList '/i "C:\CableAudio\cable-ui.msi" /qn /norestart' `
        -Wait -PassThru -NoNewWindow
}

# 앱 실행 (Session 1 - interactive desktop)
Invoke-Command -Session $session -ScriptBlock {
    schtasks /Create /TN "LaunchCable" `
        /TR '"C:\Users\cable\AppData\Local\cable-ui\cable-tauri.exe"' `
        /SC ONCE /ST 00:00 /F /IT /RL HIGHEST /RU cable /RP cable123
    schtasks /Run /TN "LaunchCable"
}

Remove-PSSession $session
```

WinRM은 Session 0 (비대화형)에서 실행되므로, GUI 앱은 `schtasks /IT`로 Session 1 (대화형 데스크톱)에서 실행해야 함.

### exe 직접 복사

MSI 없이 exe만 교체하는 방법:

```powershell
# 앱 종료 후
Copy-Item "target\release\cable-tauri.exe" `
    -Destination "C:\Users\cable\AppData\Local\cable-ui\cable-tauri.exe" `
    -ToSession $session -Force
```

## IOCTL 테스트

### ioctl-test.ps1

드라이버 IOCTL 통신을 직접 테스트하는 스크립트. VM 내부에서 실행.

```powershell
# 가상 캡처 디바이스 생성
.vm\vm-exec.ps1 "powershell -File D:\ioctl-test.ps1 -DeviceType 1 -Name 'Test Mic'"
```

| 파라미터 | 타입 | 기본값 | 설명 |
|---------|------|--------|------|
| `-DeviceType` | int | `1` | `0` = Render, `1` = Capture |
| `-Name` | string | `Test Device` | Friendly name |

### ioctl-test2.ps1

확장된 IOCTL 테스트. 생성, 삭제, 생성+삭제를 선택 가능.

```powershell
# 생성 후 2초 대기 후 자동 삭제
.vm\vm-exec.ps1 "powershell -File D:\ioctl-test2.ps1 -Action create-remove -DeviceType 0 -Name 'Test Speaker'"

# 생성만
.vm\vm-exec.ps1 "powershell -File D:\ioctl-test2.ps1 -Action create -DeviceType 1 -Name 'My Mic'"

# 삭제 (hex ID 지정)
.vm\vm-exec.ps1 "powershell -File D:\ioctl-test2.ps1 -Action remove -Name '43-42-4C-00-...'"
```

| 파라미터 | 타입 | 기본값 | 설명 |
|---------|------|--------|------|
| `-Action` | string | `create-remove` | `create`, `remove`, `create-remove` |
| `-DeviceType` | int | `1` | `0` = Render, `1` = Capture |
| `-Name` | string | `Test Device` | Friendly name (또는 remove시 hex ID) |

## 네트워크 구성

```
호스트                          VM
127.0.0.1:15985  ------>  10.0.2.15:5985  (WinRM HTTP)
127.0.0.1:14444          (QMP 관리 소켓)
```

- WinRM 인증: Basic, 평문 (로컬 테스트 전용)
- VM 계정: `cable` / `cable123`
- 네트워크 프로파일: Private 필요 (WinRM 수신용)

## 전체 워크플로우

```
[1회 설정]
  호스트: .vm\setup-host-winrm.ps1 (관리자)
  VM:     launch.ps1 -Fresh -> setup-all.bat 실행 -> 셧다운

[드라이버 테스트 사이클]
  호스트: launch.ps1 -Reuse           # VM 시작
  호스트: vm-install.ps1              # 드라이버 설치 (또는 업데이트)
  호스트: vm-exec.ps1 "..."           # 원격 명령 실행
  VM:     (드라이버/앱 테스트)
  VM:     shutdown /s /t 0            # 셧다운 (리부트 아님!)
  호스트: launch.ps1 -Reuse           # 상태 유지하며 재시작

[앱 배포 사이클]
  호스트: pnpm tauri build            # 빌드
  호스트: (WinRM으로 MSI 복사 및 설치) # 위의 "앱 배포" 섹션 참조
```

## 트러블슈팅

### WinRM 연결 실패

```powershell
# 1. VM이 실행 중인지 확인
Get-Process qemu* | Select-Object Id, ProcessName

# 2. WinRM HTTP 응답 확인 (401 = 정상, 서비스 동작 중)
Invoke-WebRequest -Uri "http://127.0.0.1:15985/wsman" -Method POST -TimeoutSec 5

# 3. QMP로 VM 상태 확인
# TCP 127.0.0.1:14444에 연결하여 {"execute":"query-status"} 전송
```

### 드라이버 Code 52 에러

인증서가 올바른 스토어에 없을 때 발생:

```powershell
.vm\vm-exec.ps1 "powershell -File C:\CableAudio\import-cert.ps1"
.vm\vm-exec.ps1 "pnputil /scan-devices"
```

### BSOD 후 복구

1. VM 프로세스가 행 상태일 수 있음 - QMP 또는 프로세스 종료
2. `launch.ps1 -Reuse`로 재시작 (오버레이에 마지막 상태 보존)
3. BSOD 원인이 드라이버 코드라면, 수정 후 `devcon update`로 업데이트

### Windows Defender 파일 잠금

`.sys` 파일을 스테이징 디렉토리에 복사하면 Defender가 잠글 수 있음. `launch.ps1`은 매번 랜덤 서브디렉토리를 사용하여 이를 회피.
