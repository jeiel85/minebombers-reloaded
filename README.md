# 💣 Mine Bombers: Reloaded

[![CI](https://github.com/jeiel85/minebombers-reloaded/actions/workflows/ci.yml/badge.svg)](https://github.com/jeiel85/minebombers-reloaded/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Language-Rust-orange?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-Unresolved%20(see%20notes)-lightgrey.svg)](assets/LICENSE-NOTES.md)

1995년 MS-DOS 아케이드 게임 **Mine Bombers 3.11**(Skitso Productions)을 최신 Windows와 Linux에서 DOSBox 없이 실행하기 위한 Rust 엔진입니다. [OpenRCT2](https://github.com/OpenRCT2/OpenRCT2)와 같은 방식으로, 이 프로젝트는 엔진이고 **게임 데이터는 사용자가 자기 사본을 준비**합니다. 원작 3.11은 무료(프리웨어)입니다. 브라우저에서 실행하는 부가 에디션도 있습니다.

| | 상태 |
|---|---|
| **Windows** (네이티브) | 실제로 실행해 확인했습니다. [릴리스 zip](https://github.com/jeiel85/minebombers-reloaded/releases) 제공(시험용) |
| **Linux** (네이티브) | CI(ubuntu-latest)에서 빌드·테스트에 더해 실제로 실행해 메뉴까지 뜨는지 확인합니다. 사람이 원작 파일로 직접 플레이해 확인한 적은 아직 없고, 배포용 패키지(tar.gz/AppImage)도 없습니다 — 소스 빌드만 가능합니다. |
| **웹** | [라이브 데모](https://jeiel85.github.io/minebombers-reloaded/) (부가 에디션) |

---

## 📦 원작 게임 파일

이 저장소와 배포 사이트에는 원작 게임 파일이 **들어 있지 않습니다.**

1. 원작 3.11을 받아 압축을 풉니다. 예: [Internet Archive의 Mine Bombers 3.11](https://archive.org/details/mnb311fw)
2. 게임을 처음 실행하면 **폴더 선택 창**이 열립니다. `TITLEBE.SPY`가 들어 있는 폴더(압축을 푼 게임 폴더)를 고르세요. 한 번만 고르면 기억합니다.

---

## 🖥️ 실행 (Windows / Linux)

**Windows (릴리스 zip)**: [Releases](https://github.com/jeiel85/minebombers-reloaded/releases)에서 `MineBombers-<버전>-windows-x64.zip`을 받아 풀고 `MineBombers.exe`를 실행합니다. 시험용(pre-release)이며 exe에 코드 서명이 없어 처음에 Windows의 "PC 보호" 창이 뜰 수 있습니다("추가 정보" → "실행"). 함께 올라오는 `.sha256` 파일로 받은 파일을 확인할 수 있습니다.

소스에서 빌드하려면 [Rust](https://rustup.rs/) 툴체인이 필요합니다.

**Windows (소스)**
```powershell
cargo run --release
```
SDL2 DLL은 저장소에 들어 있고, 빌드할 때 `build.rs`가 `lib/`의 DLL을 exe 옆(`target/debug`, `target/release`)으로 복사합니다. 그래서 `cargo run --release`는 물론 빌드된 exe를 직접 실행하는 것도 그대로 동작합니다. exe를 다른 곳으로 옮길 때는 같은 폴더의 DLL도 함께 옮기세요. 릴리스 zip 자체는 `pwsh scripts/package_windows.ps1`이 만듭니다.

**Linux**
```bash
sudo apt install libsdl2-dev libsdl2-mixer-dev
cargo run --release
```
폴더 선택 창은 `xdg-desktop-portal`이 있는 데스크톱에서 열립니다. 없으면 아래 방법으로 폴더를 직접 지정하세요. 파일 이름이 **대문자**(`TITLEBE.SPY`)여야 하므로, 소문자로 풀린 사본은 인식하지 못합니다.

### 실행 옵션

| | |
|---|---|
| `MineBombers` | 기억한 폴더로 시작합니다. 없으면 폴더 선택 창이 열립니다. |
| `MineBombers <폴더>` | 이번 실행에만 그 폴더를 씁니다(기억하지 않음). |
| `--choose-game-folder` | 폴더 선택 창을 다시 열어 다른 폴더를 고릅니다. |
| `--campaign` | 싱글플레이 캠페인 모드로 시작합니다. |
| 환경변수 `MINEBOMBERS_GAME_DIR` | 게임 폴더를 지정합니다. |
| 환경변수 `MINEBOMBERS_USER_DIR` | 설정과 기록을 저장할 폴더를 바꿉니다. |

### 설정과 기록은 어디에 저장되나

게임 폴더는 **읽기만** 합니다. 원작 파일과 원작 DOS 판은 이 엔진이 쓰는 어떤 것도 보지 않습니다. 엔진이 쓰는 파일은 사용자 폴더에 저장됩니다.

| OS | 사용자 폴더 |
|---|---|
| Windows | `%APPDATA%\MineBombers` |
| Linux | `$XDG_CONFIG_HOME/minebombers` (없으면 `~/.config/minebombers`) |

처음 실행할 때 게임 폴더에 이미 있던 `HIGHSCOR.DAT`, `PLAYERS.DAT`, `IDENTIFY.DAT`, `OPTIONS.CFG`, `config.toml`, `keysrel.cfg`는 **없는 것만 복사**해서 기존 기록을 이어받습니다.

`config.toml`로 화면과 키를 바꿀 수 있습니다.

```toml
[display]
window_mode = "Windowed"      # "Windowed", "Fullscreen", "Borderless"
scale = 2                     # 1, 2, 3
width = 1280
height = 960
vsync = true
target_fps = 60
keep_aspect_ratio = true      # 4:3 유지 (검은 여백)

[gameplay]
default_speed = 1.0
auto_bots = true              # 빈 슬롯에 AI 봇 배치
bot_difficulty = "Medium"     # "Easy", "Medium", "Hard"

[graphics]
crt_shader = false            # F6
dynamic_lighting = false      # F7

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

## 🌐 웹 에디션 (부가)

[라이브 데모](https://jeiel85.github.io/minebombers-reloaded/)를 열면 처음 한 번 원작 파일(ZIP, 압축을 푼 폴더, 또는 파일들)을 고르라고 합니다. 필요한 약 30개 파일만 이 브라우저에 보관되고 어디로도 전송되지 않으며, 메뉴(☰)의 "Forget game files"로 지울 수 있습니다. 모바일에서는 온스크린 컨트롤을 켤 수 있습니다.

로컬에서 실행하려면:

```powershell
powershell .\scripts\build_web.ps1
npx serve web
```

`MB_GAME_DIR`에 내 게임 폴더를 지정하고 `build_web.ps1`을 실행하면 사본이 `web/data/`(git 미추적)에 복사되어 개발 중에는 선택 창이 뜨지 않습니다.

---

## ✨ 원작에 없는 확장 기능

원작 규칙과 별개인 선택 기능입니다. 원작과 동작이 같은지는 아직 자동으로 검증하지 않습니다.

- **AI 봇**: 캐릭터 선택 화면에서 `Tab` 또는 `B`로 `[YOU]` → `[EASY]` → `[NORM]` → `[HARD]`를 순환합니다. 혼자서도 대전할 수 있습니다.
  봇은 사람과 같은 방식으로 플레이합니다. 바위를 파서 묻힌 보물까지 길을 뚫고(다익스트라 경로 탐색, 비용 단위는 틱),
  폭발 패턴을 정확히 계산해 폭발 범위 밖으로 피하고, 도망갈 자리가 있을 때만 폭탄을 놓습니다.
  난이도는 이 공통 두뇌를 **얼마나 잘 쓰는지**를 바꿉니다 — 판단 주기(1/2/5틱), 계획 거리, 단단한 바위를 팔 수 있는지,
  폭탄을 굴착에 쓰는지, 원거리 무기·원격 폭탄·지뢰를 쓰는지, 실수 확률(HARD 0% / NORM 7% / EASY 22%).
- **추가 게임 모드**: 골드 러시(먼저 $5,000를 캐면 승리), 서바이벌 호드(몬스터 웨이브 협동), 서든데스(라운드 후반 맵 붕괴)
- **절차 생성 동굴 맵과 4가지 바이옴**, 상점 2페이지의 **추가 특수 무기**(블랙홀 폭탄, 빙결 수류탄, 드릴 드론)
- **화면**: CRT 스캔라인(`F6`), 동적 동굴 조명(`F7`), 4:3 유지/전체 채우기(`F4`), 창 배율(`F1`~`F3`)
- **게임패드**: Xbox, PlayStation, 스위치 프로 컨트롤러 등 최대 4대, 폭발 거리에 따른 진동
- **게임 속도 조절**, **클래식 맵 선택과 명예의 전당**(웹 에디션)
- **P2P 온라인 대전**(웹 에디션 전용): WebRTC, 룸 코드, 결정론적 락스텝

---

## 🕹️ 조작키

| 동작 | Player 1 | Player 2 |
|---|---|---|
| 이동 / 굴착 | 방향키 | `W` `A` `S` `D` |
| 정지 | `Space` | `Q` |
| 폭탄 설치 / 구매 | `Enter` | `E` |
| 무기 선택 / 판매 | `Right Shift` | `Tab` |
| 원격 폭탄 기폭 | `Right Ctrl` | `Left Ctrl` |

키는 게임의 `Options` → `Redefine Keys` 또는 `config.toml`에서 바꿉니다.

| 단축키 | 기능 |
|---|---|
| `F1` `F2` `F3` | 창 배율 1x / 2x / 3x |
| `F4` | 4:3 유지 ↔ 전체 채우기 |
| `F5` | 배경 음악 켜기/끄기 |
| `F6` / `F7` | CRT 필터 / 동적 조명 |
| `F11` 또는 `Alt+Enter` | 전체화면 |
| `[` `]` (또는 `-` `+`) | 게임 속도 감소 / 증가 (`Backspace`로 1.00x) |
| `Esc` 또는 `F10` | 라운드 종료 / 메인 메뉴 |
| `Pause` 또는 `P` | 일시정지 (`P`는 어느 플레이어 키에도 쓰이지 않을 때만) |

라운드 중 속도·음악·CRT·조명·화면 비율을 바꾸거나 일시정지하면 화면 위쪽에 현재 상태가 잠깐 표시됩니다.

| 게임패드 | 기능 |
|---|---|
| D-Pad / 왼쪽 스틱 | 이동, 메뉴 이동 |
| A | 폭탄 / 선택 |
| B, LB | 정지 / 취소 |
| X | 무기 선택 / 판매 / 봇 난이도 |
| Y, RB | 원격 기폭 |
| Start / Back | 일시정지 / 메뉴로 |

---

## 🛠️ 개발

```bash
cargo test --workspace                          # 원작 게임 파일 없이 실행됩니다
node --test web/test/*.test.mjs
```

테스트는 원작 파일 대신 테스트가 직접 만든 대체 파일로 돕니다. 내 사본으로 실제 파일 검증까지 하려면 `MB_GAME_DIR`(게임 폴더)와 필요하면 `MB_GAME_ZIP`(원작 ZIP)을 지정하세요.

```bash
MB_GAME_DIR=/path/to/mb311 cargo test -p mb-wasm -- --include-ignored
```

CI는 Windows와 Linux에서 네이티브 엔진을, 그리고 웹 에디션을 빌드하고 테스트합니다.

문서: [결정 기록](DECISIONS.md) · [에셋과 라이선스 노트](assets/LICENSE-NOTES.md) · [이전 설계 문서(보관)](docs/README.md)

---

## 📜 크레딧 & 라이선스

- **Original Game**: *Mine Bombers* 3.11 by **Skitso Productions** (Finland). 원작의 저작권은 Skitso Productions에 있으며, 이 저장소는 원작과 제휴하거나 승인받은 프로젝트가 아닙니다.
- **Engine base**: 게임 엔진의 뼈대(DOS 데이터 디코딩, 타일 렌더링, 월드 시뮬레이션)는 **Ivan Dubrov**의 [`mb-reloaded`](https://github.com/idubrov/mb-reloaded)를 기반으로 합니다. 해당 저장소에는 라이선스가 명시되어 있지 않아 [라이선스를 요청하는 이슈](https://github.com/idubrov/mb-reloaded/issues/1)를 올려 두었습니다.
- **라이선스 상태**: 이 프로젝트의 코드 라이선스는 **아직 정리되지 않았습니다**. 원작 게임 파일은 이 프로젝트에 포함되지 않으며 사용자가 자기 사본을 준비합니다(이전 저장소의 이력에 있던 원본 파일은 이력을 재작성해 모두 제거했습니다). 근거와 현황은 [assets/LICENSE-NOTES.md](assets/LICENSE-NOTES.md)에 있으며, 권리자께서 요청하시면 해당 자료를 바로 제거하겠습니다.
- **이 저장소에서 새로 만든 부분**: AI 봇(`src/world/bot.rs`), 설정 파일(`src/config.rs`), 폴더 선택과 사용자 폴더(`src/gamedir.rs`, `src/userdata.rs`), 웹 에디션(`crates/mb-wasm`, `web/`), 앱 아이콘(직접 그린 폭탄 그림)
