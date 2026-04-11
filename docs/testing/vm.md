# VM Testing Environment

VMware Workstation 기반 tiny11 VM에서 CableAudio 드라이버 E2E 테스트를 수행하는 문서.

## 스크립트 구조

```
.vm/
├── test.ps1                         # 진입점: Pester로 모든 테스트 실행
├── exec.ps1                         # 수동 디버깅용 VM 내부 명령 실행
└── test/
    ├── common.ps1                   # 공통 헬퍼 (WinRM, vmrun, Reset-Vm 등)
    ├── driver-prepare.ps1           # VM에 드라이버만 설치하는 수동 준비 스크립트
    ├── ioctl.Tests.ps1              # Pester: IOCTL create/remove/rename 테스트
    └── pkey-friendlyname.Tests.ps1  # Pester: IPropertyStore FriendlyName 변경 테스트
```

## 테스트 실행

```powershell
# 전체 테스트
.vm\test.ps1

# 특정 테스트만 실행 (Pester FullName 필터)
.vm\test.ps1 -TestFilter "*IOCTL*"
.vm\test.ps1 -TestFilter "*PKEY*"
```

각 `Describe` 블록 실행 전 VM이 스냅샷으로 초기화되고 드라이버가 설치된다.
테스트 결과는 `.vm/vm-logs/test-results-<timestamp>.xml`에 저장된다.

## 테스트 목록

### ioctl.Tests.ps1

| Describe | It | 검증 내용 |
|----------|----|-----------|
| IOCTL: Capture device create/remove | creates a capture (mic) device and removes it cleanly | Create OK + Remove OK |
| IOCTL: Render device create/remove | creates a render (speaker) device and removes it cleanly | Create OK + Remove OK |
| IOCTL: Rename loop with UI reflection | renames a device via IOCTL and the new name is reflected in audio endpoints | VERIFY OK (PnP 엔드포인트 반영) |

### pkey-friendlyname.Tests.ps1

| Describe | It | 검증 내용 |
|----------|----|-----------|
| PKEY: rename audio endpoint via IPropertyStore | changes FriendlyName and the new name is visible in AudioEndpoint PnP devices | AudioEndpoint 이름 변경 반영 |

## 테스트 흐름 (각 Describe)

```
BeforeAll
  └─ Reset-Vm
       ├─ vmrun revertToSnapshot → start
       ├─ Wait-WinRM
       ├─ bcdedit testsigning 검증
       └─ Install-DriverInGuest

It (테스트 본체)

AfterAll
  └─ Remove-PSSession
```

## 선행 조건

- VMware Workstation + `vmrun`
- 호스트 WinRM 클라이언트 설정 완료 (1회)
- `.env`에 `VM_PASSWORD=...` 설정
- 드라이버 빌드 산출물:
  - `driver/x64/Debug/package/CableAudio.sys`
  - `driver/x64/Debug/package/CableAudio.inf`
  - `driver/x64/Debug/package/cableaudio.cat`
- Pester 5.x: `Install-Module Pester -Force -Scope CurrentUser`

## 주요 옵션

```powershell
# 스냅샷 이름 지정
.vm\test.ps1 -SnapshotName "my-snapshot"

# Rename 루프 횟수 조정
.vm\test.ps1 -RenameLoopCount 1

# VMware 창을 띄워서 진행 확인
.vm\test.ps1 -StartMode gui

# VM 암호 직접 전달
.vm\test.ps1 -VmPassword "secret"
```

## 개별 유틸리티

### exec.ps1

VM 내부에서 단일 명령을 실행하는 수동 디버깅 도구. VM이 이미 실행 중일 때 사용한다.

```powershell
.vm\exec.ps1 "hostname"
.vm\exec.ps1 "Get-PnpDevice -Class MEDIA"
.vm\exec.ps1 "Get-Content C:\Windows\INF\setupapi.dev.log -Tail 80"
```

### test/driver-prepare.ps1

VM을 스냅샷에서 복원하고 드라이버만 설치한 채로 대기 상태로 둔다. 수동 탐색이나 개별 테스트 준비에 사용한다.

```powershell
.vm\test\driver-prepare.ps1
.vm\test\driver-prepare.ps1 -SkipRevert  # 이미 실행 중인 VM에 드라이버만 재설치
```

## 로그

- Pester 결과 XML: `.vm/vm-logs/test-results-<timestamp>.xml`

## 참고

- VM 암호(`vmrun -vp`)는 다음 우선순위로 읽는다.
  1. `-VmPassword` 파라미터
  2. 환경변수 `VM_PASSWORD`
  3. 프로젝트 루트 `.env`의 `VM_PASSWORD=...`
- 호스트 WinRM 설정이 누락된 경우, 관리자 PowerShell에서 1회 실행:

```powershell
Start-Service WinRM
Set-Service WinRM -StartupType Manual
Set-Item WSMan:\localhost\Client\AllowUnencrypted $true -Force
Set-Item WSMan:\localhost\Client\TrustedHosts -Value "192.168.23.128" -Force
```
