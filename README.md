# 💣 Mine Bombers: Reloaded ⛏️

[![GitHub Pages](https://img.shields.io/badge/Live%20Demo-GitHub%20Pages-brightgreen?logo=github)](https://jeiel85.github.io/minebombers-reloaded/)
[![Rust](https://img.shields.io/badge/Language-Rust%202018%2F2021-orange?logo=rust)](https://www.rust-lang.org/)
[![WebAssembly](https://img.shields.io/badge/Platform-WebAssembly%20%2F%20WASM-purple?logo=webassembly)](https://webassembly.org/)
[![Windows](https://img.shields.io/badge/Platform-Windows%2064--bit-blue?logo=windows)](https://www.microsoft.com/windows)
[![License](https://img.shields.io/badge/License-MIT%20%2F%20Freeware-yellow.svg)](#-크레딧--라이선스-credits--acknowledgements)

> **1995년 고전 MS-DOS 명작 아케이드 게임 *Mine Bombers (v3.11)*의 100% 순수 Rust 네이티브 & WebAssembly 리마스터 엔진**  
> DOSBox 에뮬레이터 없이 최신 64비트 Windows 및 웹 브라우저에서 60 FPS 하드웨어 가속으로 구동되며, 3단계 **인공지능(AI) 봇 시스템**, **레트로 CRT 셰이더 & 동적 조명**, **게임패드 햅틱 진동 피드백**, **4:3 레터박스 종횡비 보정**, **완전한 키/환경 커스터마이징(`config.toml`)**을 지원합니다.

---

## 🌐 라이브 데모 (Play Online)

별도 설치 없이 웹 브라우저에서 링크 클릭 한 번으로 오리지널 사운드와 게임패드 진동까지 그대로 즐기실 수 있습니다:

👉 **[https://jeiel85.github.io/minebombers-reloaded/](https://jeiel85.github.io/minebombers-reloaded/)**

---

## ✨ 핵심 특징 (Key Features)

- ⚡ **순수 네이티브 64비트 Windows 엔진 (No DOSBox)**:
  - DOSBox 에뮬레이션 없이 Rust와 SDL2로 직접 구동되어 초저지연 반응성과 60 FPS의 부드러운 화면을 제공합니다.
  - Windows GUI 서브시스템으로 컴파일되어 실행 시 **검은색 cmd 콘솔 창이 전혀 뜨지 않습니다.**
  - 전용 광부 & 다이너마이트 커스텀 멀티 해상도 아이콘 내장 (`MineBombers.exe`).
- 🌐 **온라인 & LAN P2P 멀티플레이 넷플레이 (WebRTC Netplay)**:
  - 서버리스 WebRTC DataChannel P2P 아키텍처 및 6자리 룸 코드(`mb-XXXXXX`)로 원거리 친구와 즉시 매칭.
  - 브라우저 `BroadcastChannel` 기반 로컬/LAN 방 자동 검색 기능 내장.
  - 60 FPS 결정론적 락스텝(Deterministic Lockstep) 입력 동기화로 쾌적한 4인 대전/협동 플레이 지원.
  - 레트로 CRT 스타일 넷플레이 모달 로비 UI 및 실시간 Ping HUD 표시.
- 🗺️ **절차적 동굴 맵 생성기 & 4대 지하 바이옴 (Procedural Biomes)**:
  - 셀룰러 오토마타 기반 무한 동굴 생성 시스템.
  - **흙/점토(Dirt)**, **화산 현무암/용암(Basalt)**, **빙하/얼음(Glacier)**, **심연 수정 동굴(Deep Crystal)**의 4대 지하 바이옴.
  - 각 바이옴의 고유 암석 강도, 광물 분포, 그리고 상점 미니맵 실시간 렌더링 반영.
- 💰 **골드 러시(Gold Rush) 채굴 경쟁 모드**:
  - 가장 먼저 **$5,000의 금을 채굴한 플레이어가 즉시 승리**하는 속도전 모드.
  - 모든 광부에게 기본 **드릴 속도 +15 보너스** 부여.
  - 사망 시 소지 자금의 50%가 필드에 보물 주머니로 흩뿌려지는 역전 메커니즘.
- 🧟 **서바이벌 호드(Survival Horde) 협동 PVE 모드**:
  - 플레이어와 아군 봇이 팀을 이루어 몰려오는 몬스터 웨이브를 섬멸하는 협동 모드.
  - 웨이브별 몬스터 수량(`8 + Wave*4`), 체력(+15%/웨이브), 굴착력 강화 및 동적 증원 스폰.
  - 몬스터 처치 시 골드 현상금 드롭, 아군 부활 시스템 및 웨이브 클리어 $1,500 보너스.
- 🌪️ **배틀로얄 / 서든데스 수축 광산 (Sudden Death Mine Collapse)**:
  - 라운드 종료 60초 전 붕괴 경고, 50초 전부터 5초마다 동심원 형태로 맵이 붕괴하며 화염/용암 타일 생성.
  - 위험 구역 잔류 시 초당 24 DPS 지속 환경 피해 및 지진 화면 진동 효과.
  - AI 봇의 안전 구역 탈출 지능 탑재.
- 💣 **신규 특수 무기 확장 (Arsenal Expansion)**:
  - 상점 2페이지(`Tab` 전환): **블랙홀 폭탄(Black Hole)**, **빙결 수류탄(Freeze Bomb)**, **드릴 드론(Drill Drone)** 등 추가.
- 🤖 **3단계 차별화 인공지능(AI) 봇 시스템**:
  - 원작의 한계(무조건 2~4명의 사람이 한 키보드로 플레이해야 함)를 극복하여 **혼자서도 컴퓨터와 박진감 넘치는 대전**이 가능합니다.
  - **Easy (`[EASY]`)**: 느린 반응, 우왕좌왕 위험 회피, 근접 기본 폭탄 위주, 소극적 상점 구매.
  - **Normal (`[NORM]`)**: 표준 2틱 반응 속도, 폭탄 감지 신속 회피, 수류탄 견제 투척, 균형 잡힌 무장 쇼핑.
  - **Hard (`[HARD]`)**: 매 틱 실시간 상황 판단, 막다른 골목(Dead-end) 감지 탈출, 도주로 차단 폭격, 지뢰 매설, 적극적 인간 추적 및 풀무장 쇼핑.
  - 캐릭터 선택 창에서 언제든 `Tab` 또는 `B` 키로 `[YOU]` ↔ `[EASY]` ↔ `[NORM]` ↔ `[HARD]` 순환 변경 가능.
- 📺 **레트로 CRT 셰이더 & 동적 동굴 라이팅**:
  - **CRT 스캔라인 & 비네팅 (`F6`)**: 90년대 오락실 브라운관 모니터의 수평 주사선과 곡면 음영을 실시간 합성.
  - **동적 동굴 조명 & 광부 랜턴 (`F7`)**: 칠흑 같은 지하 동굴 속에서 각 광부의 랜턴이 주위를 비추며, 폭탄 폭발 시 강력한 화염 광선이 순간적으로 주위를 밝히고 자연스럽게 감쇠.
- 🎮 **현대 게임패드 / 컨트롤러 완벽 지원**:
  - Xbox, PlayStation(DualShock/DualSense), 닌텐도 스위치 프로 컨트롤러 등 최대 4대 핫플러그(Hotplug) 자동 인식.
  - 폭발 발생 시 플레이어와의 거리를 계산하여 가까울수록 강렬한 **물리적 손끝 진동(Haptic Dual-Rumble)** 제공.
- 📺 **현대적인 디스플레이 & 화면비 설정**:
  - **4:3 고전 레트로 화면비 완벽 유지 (`F4`)**: 임의의 모니터나 창 크기에서도 화면 왜곡 없이 좌우 필러박스(Pillarbox)로 깔끔하게 렌더링.
  - **1x / 2x / 3x 정수 배율 핫키 (`F1`~`F3`)**: 창 크기를 도트 깨짐 없이 선명하게 확대.
  - **전체화면(Fullscreen) ↔ 창모드 전환 (`F11` / `Alt+Enter`)**.
  - **VSync 및 FPS 리미터** 지원.
- ⌨️ **완벽한 키 바인딩 & `config.toml` 환경설정**:
  - 1P부터 4P까지 모든 키와 그래픽/게임플레이 설정을 직관적인 `config.toml`로 제어.
  - 인게임 `Options` → `Redefine Keys` 메뉴에서도 직접 키를 눌러 손쉽게 재설정 가능.
- ⏩ **실시간 인게임 게임 속도 조절**:
  - 게임 플레이 도중 `[` 및 `]` 키로 슬로우 모션부터 초고속 플레이까지 자유롭게 배속 변경 가능 (`Backspace`로 리셋).
- 🎵 **1995년 오리지널 리소스 100% 보존 연동**:
  - 원본 46개 클래식 맵(`.MNE`), 12종 무기/도구, 효과음(`.VOC`), BGM(`.S3M`)이 완벽하게 재생됩니다.
- 🏆 **클래식 맵 선택 & 명예의 전당 (웹 에디션)**:
  - `Options` → `LOAD LEVELS`에서 `←`/`→`(또는 `Enter`)로 절차 생성 맵, 원작 클래식 맵(BATTLE, CASTLE, JAIL, LABYRINTH, BIOFARM, OLD MINE, CRUMBLE, ROCKS), 랜덤 클래식 중에서 고를 수 있습니다. 온라인 넷플레이는 피어 간 동기화를 위해 항상 절차 생성 맵을 사용합니다.
  - `Players`를 1로 설정하고 모든 라운드를 마치면 원작의 명예의 전당 화면이 열리고, 라운드 수 → 소지금 순으로 상위 10위에 기록됩니다. 기록은 브라우저 메모리에만 유지되므로 페이지를 새로고침하면 초기 순위표로 돌아갑니다.

---

## 🕹️ 조작키 & 단축키 (Controls & Hotkeys)

### 인게임 기능 및 그래픽 단축키
| 단축키 | 기능 |
|---|---|
| **`F6`** | **레트로 CRT 스캔라인 & 비네팅 필터 On / Off** |
| **`F7`** | **어두운 동굴 동적 라이팅 & 광부 랜턴 On / Off** |
| **`F11`** 또는 **`Alt + Enter`** | 전체화면 (Fullscreen) ↔ 창 모드 전환 |
| **`F1`** | 1x 배율 (640 × 480 픽셀) |
| **`F2`** | 2x 고해상도 배율 (1280 × 960 픽셀, 기본 권장) |
| **`F3`** | 3x 대형 배율 (1920 × 1440 픽셀) |
| **`F4`** | 4:3 레트로 화면비 고정 / 전체 채우기 토글 |
| **`[` 또는 `-`** | 게임 속도 감소 (0.75x, 0.50x ...) |
| **`]` 또는 `+`** | 게임 속도 증가 (1.25x, 1.50x, 2.00x ...) |
| **`Backspace` 또는 `0`** | 게임 속도 1.00x 정속으로 리셋 |
| **`F5`** | BGM 배경 음악 On / Off |
| **`F10`** 또는 **`ESC`** | 라운드 종료 / 메인 메뉴 나가기 |

### 캐릭터 선택 화면
- **`Tab` 또는 `B`**: 선택된 슬롯을 **`[YOU]`(인간) ↔ `[EASY]` ↔ `[NORM]` ↔ `[HARD]`(봇 난이도)**로 순환 전환

### 플레이어 기본 키보드 조작키
| 동작 | Player 1 (화살표 키) | Player 2 (WASD 키) |
|---|---|---|
| **이동 / 굴착** | 방향키 (`↑`, `↓`, `←`, `→`) | `W`, `S`, `A`, `D` |
| **자세 유지 (Stop)** | `Space` | `Q` |
| **폭탄 설치 / 구매** | `Enter` (Return) | `E` |
| **무기 선택 / 판매** | `Right Shift` | `Tab` |
| **원격 폭탄 기폭** | `Right Control` | `Left Control` |

### 게임패드 / 컨트롤러 조작키
| 패드 버튼 | 기능 |
|---|---|
| **D-Pad / 왼쪽 아날로그 스틱** | 이동 및 굴착 / 메뉴 이동 |
| **A 버튼 (남쪽)** | 폭탄 설치 / 메뉴 선택 (`Enter`) |
| **B 버튼 / LB (동쪽/좌측 숄더)** | 정지 및 자세 유지 (`Stop`) / 메뉴 취소 (`ESC`) |
| **X 버튼 (서쪽)** | 무기 선택 / 상점 판매 / 봇 난이도 변경 (`Tab`) |
| **Y 버튼 / RB (북쪽/우측 숄더)** | 원격 폭탄 기폭 (`Remote`) |
| **Back(View) 버튼** | 게임 즉시 중단 및 메뉴 복귀 |
| **Start 버튼** | 일시정지 (Pause) |

---

## ⚙️ 설정 파일 (`config.toml`)

프로젝트 루트의 `config.toml`을 메모장으로 열어 디스플레이, 봇 기본 난이도, 그래픽 필터, 플레이어별 키를 자유롭게 변경할 수 있습니다:

```toml
[display]
window_mode = "Windowed"      # "Windowed", "Fullscreen", "Borderless"
scale = 2                     # 1, 2, 3 배율
width = 1280
height = 960
vsync = true                  # 수직동기화 활성화
target_fps = 60               # 목표 FPS (60, 120, 144 등)
keep_aspect_ratio = true      # 4:3 고전 화면비 유지 (레터박스)

[gameplay]
default_speed = 1.0           # 기본 게임 속도 (1.0 = 표준)
auto_bots = true              # 멀티플레이 시작 시 빈 슬롯에 AI 봇 자동 참여
bot_difficulty = "Medium"     # 기본 AI 난이도 ("Easy", "Medium", "Hard")

[graphics]
crt_shader = false            # 레트로 CRT 브라운관 스캔라인 필터 (F6)
dynamic_lighting = false      # 어두운 동굴 동적 라이팅 및 랜턴 시야 (F7)

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

## 🚀 실행 및 빌드 가이드

### 1. Windows 데스크톱 실행 (Native Edition)
1. 저장소를 클론하거나 릴리즈 바이너리를 다운로드합니다.
2. 루트 폴더의 **`MineBombers.exe`**를 실행하면 즉시 플레이 가능합니다!
   *(필요한 모든 DLL 및 오리지널 에셋 `res/minebomb`이 동봉되어 있습니다.)*

### 2. 웹 에디션 로컬 실행 (WebAssembly Edition)
```powershell
# WebAssembly 바이너리 빌드 + 원본 게임 파일을 web/data로 복사
powershell .\scripts\build_web.ps1

# 로컬 웹 서버 실행 (npx serve 또는 python)
npx serve web
# 또는 python -m http.server 8080 --directory web
```
브라우저에서 `http://localhost:8080`에 접속하여 플레이합니다.

**원본 게임 파일은 WASM에 들어 있지 않습니다.** 페이지가 실행 시 `web/data/`에서 원본 그대로의 파일(화면·맵·효과음·BGM)을 불러오며, 이 폴더는 git에 포함되지 않습니다. `build_web.ps1`(또는 `node scripts/stage_web_data.mjs`)이 `res/minebomb`의 파일을 **변환 없이 복사**해 채웁니다. 파일이 없으면 페이지가 어떤 파일이 없는지 알려 줍니다. 배경음악은 `web/vendor/chiptune3`(MIT, libopenmpt는 BSD)가 원본 S3M을 직접 재생합니다.

### 3. 소스코드 빌드 (Rust Toolchain)
```powershell
# 단위 테스트 실행
cargo test

# 최적화된 Release 바이너리 컴파일
cargo build --release

# 실행 파일을 루트 디렉터리로 복사
Copy-Item target\release\MineBombers.exe .\MineBombers.exe -Force
```

---

## 📜 크레딧 & 라이선스 (Credits & Acknowledgements)

- **Original Game**: *Mine Bombers* (1995–1996) created by **Sami Lehtinen & Antti Lehtinen (Skhar)**.
- **Reverse Engineering Core**: DOS 바이너리 디코딩 및 타일 렌더링 프레임워크는 **Ivan Dubrov**의 오픈소스 프로젝트 `mb-reloaded`의 리버스 엔지니어링 분석을 기반으로 참고하였습니다.
- **Enhanced Remaster Engine**:
  - 3단계 반응형 AI 봇 시스템 (`src/world/bot.rs`)
  - WebAssembly 제로 카피 렌더링 & 웹 오디오/햅틱 브라우저 파이프라인 (`crates/mb-wasm`, `web/`)
  - 레트로 CRT 스캔라인 셰이더 및 감산형 동적 동굴 라이팅 시스템 (`src/context.rs`)
  - 4인 게임패드 자동 핫플러그 및 폭발 거리 기반 듀얼 럼블 진동 피드백 (`src/gamepad.rs`)
  - 4:3 레터박스 종횡비 보정 및 인게임 실시간 배율/속도 조절 시스템
  - Windows MSVC 완벽 포팅 및 GUI 서브시스템 (콘솔 창 제거, 고해상도 광부 아이콘 내장)
