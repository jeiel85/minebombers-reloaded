# 💣 Mine Bombers: Reloaded (Native Windows Edition) ⛏️

> **1995년 고전 DOS 명작 아케이드 게임 *Mine Bombers (v3.11)*의 100% 순수 네이티브 Windows 리메이크 & 확장 엔진**  
> DOSBox 에뮬레이터 없이 최신 64비트 Windows에서 60 FPS 하드웨어 가속으로 구동되며, 혼자서도 즐길 수 있는 **인공지능(AI) 봇 시스템**, **자유로운 해상도 배율 & 4:3 레터박스 종횡비 보정**, **완전한 키 커스터마이징(`config.toml`)**을 지원합니다.

---

## ✨ 핵심 특징 (Key Features)

- ⚡ **순수 네이티브 64비트 Windows 엔진 (No DOSBox)**:
  - DOSBox 에뮬레이션 없이 Rust와 SDL2로 직접 구동되어 즉각적인 반응성과 60 FPS의 부드러운 화면을 제공합니다.
  - Windows GUI 서브시스템으로 컴파일되어 실행 시 **검은색 cmd 콘솔 창이 전혀 뜨지 않습니다.**
- 🤖 **자체 개발 AI 봇(Bot) 시스템**:
  - 원작의 한계(무조건 2~4명의 사람이 한 키보드로 플레이해야 함)를 극복하여 **혼자서도 컴퓨터와 대전**할 수 있습니다.
  - 폭탄 폭발 경로 회피(Danger Evasion), 자원 채굴(Mining), 근접 폭탄 매설/원거리 수류탄 투척(Combat), 상점 자동 구매(Auto-Shop) 탑재.
  - 캐릭터 선택 창에서 언제든 `Tab` 또는 `B` 키로 `[YOU] ↔ [CPU]` 슬롯을 자유롭게 변경 가능.
- 📺 **현대적인 디스플레이 & 화면비 설정**:
  - **4:3 고전 레트로 화면비 완벽 유지**: 임의의 모니터나 창 크기에서도 화면이 옆으로 늘어나지 않고 좌우 블랙 바(Pillarbox)로 깔끔하게 렌더링.
  - **1x / 2x / 3x 정수 배율 핫키**: 창 크기를 도트 깨짐 없이 선명하게 확대.
  - **전체화면(Fullscreen) / 테두리 없는 창모드(Borderless) / 창모드(Windowed)** 자유 전환.
  - **VSync 및 FPS 리미터** 지원.
- ⌨️ **완벽한 키 바인딩 & `config.toml` 환경설정**:
  - 1P부터 4P까지 모든 키를 메모장으로 쉽게 바꿀 수 있는 직관적인 `config.toml` 제공.
  - 인게임 `Options` → `Redefine Keys` 메뉴에서도 직접 키를 눌러 손쉽게 재설정 가능.
- ⏩ **실시간 인게임 게임 속도 조절**:
  - 게임 플레이 도중 `[` 및 `]` 키로 슬로우 모션부터 초고속 플레이까지 자유롭게 배속 변경 가능.
- 🎵 **1995년 오리지널 리소스 100% 보존 연동**:
  - 원본 46개 클래식 맵(`.MNE`), 12종 무기/도구, 사운드 블라스터 효과음(`.VOC`), Scream Tracker 3 BGM(`.S3M`)이 완벽하게 재생됩니다.

---

## 🚀 빠른 시작 (Quick Start)

### 1. 웹 브라우저에서 바로 플레이 (WebAssembly Edition) 🌐
설치 없이 크롬, 엣지, 사파리, 파이어폭스 브라우저에서 링크 클릭만으로 즉시 플레이할 수 있습니다:
- **온라인 데모 (GitHub Pages)**: `https://jeiel85.github.io/minebombers-reloaded/`
- **로컬 실행**:
  ```bash
  # 빌드 (WASM 아티팩트 생성)
  powershell .\scripts\build_web.ps1

  # 로컬 웹서버 실행
  npx serve web
  # 또는
  python -m http.server 8080 --directory web
  ```
  브라우저에서 `http://localhost:8080`으로 접속하여 즐기실 수 있습니다.
- **특징**: 웹에서도 **Xbox/PS 패드 조작 및 폭발 시 햅틱 럼블 진동(`dual-rumble`)**이 동일하게 작동합니다!

### 2. Windows 독립 실행 파일로 플레이 (Native Edition) 💻
1. 본 저장소를 다운로드하거나 클론합니다.
2. 루트 폴더의 **`MineBombers.exe`**를 더블클릭하면 즉시 게임이 시작됩니다!
   *(필요한 모든 라이브러리 `SDL2.dll`, `SDL2_mixer.dll` 및 오리지널 애셋 `res/minebomb`이 이미 포함되어 있습니다.)*

---

## 🕹️ 조작키 & 단축키 (Controls & Hotkeys)

### 인게임 디스플레이 및 편의 단축키
| 단축키 | 기능 |
|---|---|
| **`F11`** 또는 **`Alt + Enter`** | 전체화면 (Fullscreen) ↔ 창 모드 전환 |
| **`F1`** | 1x 배율 (640 × 480 픽셀) |
| **`F2`** | 2x 고해상도 배율 (1280 × 960 픽셀, 기본 권장) |
| **`F3`** | 3x 대형 배율 (1920 × 1440 픽셀) |
| **`F4`** | 4:3 레트로 화면비 고정 / 화면 채우기 토글 |
| **`[` 또는 `-`** | 게임 속도 감소 (0.75x, 0.50x ...) |
| **`]` 또는 `+`** | 게임 속도 증가 (1.25x, 1.50x, 2.00x ...) |
| **`Backspace` 또는 `0`** | 게임 속도 1.00x 정속으로 리셋 |
| **`F5`** | BGM 배경 음악 On / Off |
| **`F10`** | 라운드 강제 종료 / 게임 나가기 |

### 캐릭터 선택 화면
- **`Tab` 또는 `B`**: 선택된 플레이어 슬롯을 **`[YOU]`(인간) ↔ `[CPU]`(AI 봇)**으로 전환

### 기본 플레이어 조작키 (기본값)
> 모든 키는 `config.toml` 파일이나 인게임 `Redefine Keys` 메뉴에서 원하는 키로 언제든 변경할 수 있습니다.

| 동작 | Player 1 (화살표 키) | Player 2 (WASD 키) |
|---|---|---|
| **이동 / 굴착** | 방향키 (`↑`, `↓`, `←`, `→`) | `W`, `S`, `A`, `D` |
| **정지 (Stop)** | `Space` | `Q` |
| **폭탄 설치 / 구매** | `Enter` (Return) | `E` |
| **무기 선택 / 판매** | `Right Shift` | `Tab` |
| **원격 폭탄 기폭** | `Right Control` | `Left Control` |

---

## ⚙️ 설정 파일 (`config.toml`)

프로젝트 루트의 `config.toml`을 메모장으로 열어 각종 그래픽, 게임플레이, 플레이어별 키를 취향에 맞게 설정할 수 있습니다:

```toml
[display]
window_mode = "Windowed"      # "Windowed", "Fullscreen", "Borderless"
scale = 2                     # 1, 2, 3
width = 1280
height = 960
vsync = true                  # 수직동기화 활성화
target_fps = 60               # 목표 FPS (60, 120, 144 등)
keep_aspect_ratio = true      # 4:3 고전 화면비 유지 (레터박스)

[gameplay]
default_speed = 1.0           # 기본 게임 속도 (1.0 = 표준)
auto_bots = true              # 멀티플레이 시작 시 빈 슬롯에 AI 봇 자동 배치

[keys.player1]
left = "Left"
right = "Right"
up = "Up"
down = "Down"
stop = "Space"
bomb = "Return"
choose = "RightShift"
remote = "RightControl"
```

---

## 🛠️ 소스코드 직접 빌드하기 (Building from Source)

최신 64비트 Windows 환경에서 직접 빌드할 수 있습니다:

### 사전 준비
- **Rust Toolchain** (64-bit MSVC 타깃 권장): [rustup.rs](https://rustup.rs/)
- MSVC C++ Build Tools (Visual Studio 빌드 도구)

### 빌드 명령어
```powershell
# 개발 빌드 및 테스트
cargo test

# 최적화된 Release 바이너리 빌드
cargo build --release

# 생성된 실행 파일을 루트로 복사
Copy-Item target\release\MineBombers.exe .\MineBombers.exe -Force
```

---

## 📜 크레딧 & 라이선스 (Credits & Acknowledgements)

- **Original Game**: *Mine Bombers* (1995–1996) created by **Sami Lehtinen & Antti Lehtinen (Skhar)**.
- **Reverse Engineering Core**: DOS 바이너리 디코딩 및 타일 렌더링 프레임워크는 **Ivan Dubrov**의 오픈소스 프로젝트 `mb-reloaded`의 리버스 엔지니어링 분석을 기반으로 참고하였습니다.
- **Enhanced Engine & Features**:
  - 자체 AI 봇 시스템 (`src/world/bot.rs`)
  - Windows MSVC 완벽 포팅 및 콘솔 창 제거 (Windows GUI Subsystem)
  - `config.toml` 통합 환경설정 및 1~4P 키 매핑 시스템 (`src/config.rs`)
  - 4:3 레터박스 종횡비 보정 및 인게임 단축키 (`F1`~`F4`, `F11`)
  - 실시간 게임 배속 조절 시스템
