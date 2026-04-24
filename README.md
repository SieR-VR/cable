# Cable

Windows용 실시간 오디오 라우팅 데스크탑 앱입니다. 노드 그래프 UI로 물리적·가상 오디오 장치를 연결하고, 가상 오디오 장치를 동적으로 생성·관리할 수 있습니다.

> **Warning**
> 현재 개발 초기 단계입니다. 가상 오디오 장치 기능은 테스트 서명(test signing)이 활성화된 Windows 환경에서만 동작합니다.

## 특징

- **노드 그래프 편집기** — React Flow 기반 UI로 오디오 소스와 출력 장치를 연결
- **가상 오디오 장치** — 커널 드라이버(CableAudio.sys)를 통해 가상 입출력 장치를 동적으로 생성
- **실시간 오디오 처리** — 서브밀리초 정밀도의 스핀루프 오디오 처리 스레드
- **장치 이름 변경** — 가상 장치의 표시 이름을 UAC 권한 상승을 통해 변경

## 요구 사항

- Windows 10 / 11
- 가상 오디오 장치 사용 시: 테스트 서명 모드 활성화

## 빌드

의존성: [Rust](https://rustup.rs), [Node.js](https://nodejs.org), [pnpm](https://pnpm.io), [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) (C++ 워크로드 포함), [WDK](https://learn.microsoft.com/en-us/windows-hardware/drivers/download-the-wdk) (드라이버 빌드 시)

```powershell
# 앱 전체 빌드 (드라이버 제외)
.\scripts\build.ps1 -Target App

# 드라이버만 빌드 (WDK 필요)
.\scripts\build.ps1 -Target Driver

# 전체 빌드 (드라이버 + 앱)
.\scripts\build.ps1

# 개발 서버 실행
pnpm tauri dev
```

## 아키텍처

```
Frontend (React + Zustand + React Flow)
        │  invoke() IPC
        ▼
Tauri Commands  (crates/tauri)
        ├── Runtime     — 위상 정렬 기반 오디오 그래프 처리 (스핀루프)
        ├── Nodes       — AudioInputDevice / AudioOutputDevice (cpal)
        │               VirtualAudioInput / VirtualAudioOutput (드라이버 링버퍼)
        └── DriverClient — Win32 SetupDi + DeviceIoControl → CableAudio.sys

crates/common  — #![no_std] 커널-유저 공유 ABI 타입
driver/        — CableAudio.sys Windows 커널 드라이버 (C++)
```

## 라이선스

이 프로젝트는 구성 요소에 따라 이중 라이선스로 배포됩니다:

| 구성 요소 | 라이선스 |
|-----------|----------|
| 앱 코드 (`src/`, `crates/`) | [GPL-3.0](./LICENSE-GPL) |
| 커널 드라이버 (`driver/`) | [MS-PL](./LICENSE-MS-PL) |

드라이버 코드는 Microsoft의 WDK 샘플 및 [VirtualDrivers/Virtual-Audio-Driver](https://github.com/VirtualDrivers/Virtual-Audio-Driver)를 기반으로 합니다.  
원본 저작권: Copyright (c) Microsoft Corporation. All Rights Reserved.

자세한 내용은 [LICENSE](./LICENSE)를 참고하세요.
