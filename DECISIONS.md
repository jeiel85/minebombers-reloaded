# 결정 기록

되돌리기 어려운 결정만 적습니다. 각 항목: 상태, 사실, 대안, 이유/결과.

## D1. 게임 데이터는 사용자가 준비한다 (OpenRCT2 방식) — 결정됨 (2026-09-20)

- **사실**: 원작 패키지의 문서가 서로 모순됩니다(`FILE_ID.DIZ`는 "MAY BE DISTRIBUTED", `MINEENG.TXT`는 "복사·배포는 불법"). 변환·내장(파생)에 대한 언급은 없습니다. 상위 `mb-reloaded`, ScummVM, OpenRA, OpenTyrian, OpenRCT2는 원본 데이터를 소스 저장소에 넣지 않습니다.
- **대안**: 원본 동봉 / 사이트가 원본을 제공 / 권리자 허락을 받고 동봉 / 대체 에셋 제작.
- **결과**: 저장소·배포 사이트·WASM에 원본이 없고, 사용자가 자기 사본을 고릅니다(PR #12). 웹 데모는 처음에 파일을 한 번 골라야 열립니다. git 이력의 원본은 D4에서 제거했습니다.

## D2. 제품 방향: 원작 3.11을 최신 Windows/Linux에서 돌리는 이식 — 제안됨 (2026-09-20)

- **내용**: 원작 데이터와 호환되는 엔진이 목표이고 원작 규칙(클래식)이 기준입니다. 봇, 신규 모드, 넷플레이, 웹판 같은 확장은 클래식과 분리된 선택 기능으로 둡니다.
- **이유**: 유지관리자가 제시한 방향이며, D1의 데이터 모델과 일관됩니다. 이 저장소는 짧은 기간에 방향이 여러 번 바뀌었고(TS 웹 멀티플레이 → JS-DOS → 네이티브 → WASM) 문서가 첫 방향에 머물러 있었습니다.
- **우선순위**: 로컬(네이티브) 게임이 가장 중요하고, 웹판은 부가 에디션입니다.
- **검증 상태**: Windows 네이티브는 실제로 실행해 확인했습니다(창, 첫 실행 대화상자 흐름 포함). Linux는 CI에서 빌드와 단위 테스트가 통과했지만 **실제 실행은 아직 확인하지 못했습니다.** 확인 전에는 README에 Linux 지원을 적지 않습니다.
- **해결됨, 확장이 원작 데이터를 오염시키던 문제**: 신규 모드(`WinCondition::GoldRush` = 2, `Survival` = 3)가 원작 게임 자신의 `OPTIONS.CFG` 승리 조건 바이트에 저장되고 있었습니다(원작에는 0/1뿐). 엔진이 쓰는 파일을 모두 사용자 폴더로 옮겼으므로 원작 폴더의 `OPTIONS.CFG`에는 더 이상 아무것도 쓰이지 않습니다(D6).
- **해결됨, 쓰기 위치**: 설정(`config.toml`, `keysrel.cfg`), 옵션(`OPTIONS.CFG`), 기록(`HIGHSCOR.DAT`, `PLAYERS.DAT`, `IDENTIFY.DAT`)이 모두 게임 폴더에 쓰이던 문제. 게임 폴더는 이제 읽기만 합니다(D6).

## D5. 네이티브 게임은 첫 실행 때 폴더 선택 창으로 원작 파일을 찾는다 (OpenRCT2 방식) — 결정됨 (2026-09-20)

- **내용**: 처음 실행하면 안내창과 폴더 선택 창이 열리고, 고른 폴더는 `TITLEBE.SPY` 존재로 검증해 사용자 폴더의 `game_path.txt`에 기억합니다. 우선순위는 명령행 폴더 → `MINEBOMBERS_GAME_DIR` → 기억한 폴더 → exe 주변 자동 탐색 → 선택 창입니다. `--choose-game-folder`로 다시 고를 수 있습니다.
- **대안**: 명령행 인자와 exe를 게임 폴더에 복사하는 방식(기존) / 설치 관리자 / 웹처럼 파일 업로드.
- **이유**: 기존 방식은 첫 사용자에게 아무 반응 없이 종료되거나 파일을 복사해야 해서 불편했습니다.
- **한계**: Linux에서는 `xdg-desktop-portal`이 있어야 선택 창이 열립니다(`rfd` 기본 백엔드). 포털이 없으면 `MINEBOMBERS_GAME_DIR`나 명령행 인자를 써야 합니다. Linux에서 파일 이름이 소문자로 풀린 사본은 인식하지 못합니다(원작 파일은 대문자).

## D6. 엔진이 쓰는 파일은 사용자 폴더에 둔다 — 결정됨 (2026-09-20)

- **내용**: Windows `%APPDATA%\MineBombers`, macOS `~/Library/Application Support/MineBombers`, 그 외 `$XDG_CONFIG_HOME/minebombers`(없으면 `~/.config/minebombers`). `MINEBOMBERS_USER_DIR`로 바꿀 수 있습니다. 처음 실행 때 게임 폴더에 이미 있던 엔진 관련 파일 6종을 **없는 것만 복사**하고, 게임 폴더는 읽기만 합니다.
- **이유**: 원작 파일을 건드리지 않고 원작 DOS 판과 공존하며, 설치 폴더가 읽기 전용이어도 동작합니다.
- **검증**: 실제 실행으로 게임 폴더의 추가·수정·삭제 0건, 기존 기록의 동일 복사를 확인했습니다.

## D3. 엔진 코드의 출처 — 미결

- **사실** (2026-09-20 측정, 같은 경로 파일의 의미 있는 줄 일치율): `src/`의 73%, `crates/mb-core`의 75%가 상위 `mb-reloaded`(라이선스 없음)와 같은 줄입니다. `bitmaps.rs`, `roster.rs`는 100%. 새로 쓴 것은 `world/bot.rs`, `config.rs`, `crates/mb-wasm`입니다.
- **대안**: (a) 상위 저자가 라이선스를 부여([idubrov/mb-reloaded#1](https://github.com/idubrov/mb-reloaded/issues/1) 대기 중) / (b) 유지관리자가 독립적으로 다시 구현 / (c) 현 상태 유지(권장하지 않음).
- **유지관리자 의견**: (b) 지향("엔진을 직접 만든다").
- **(b)의 전제**: 규칙의 출처를 문서로 남긴다(원본 파일 포맷, DOS 원작 관찰). 모듈별 출처 표를 유지한다. 기존 코드를 보며 옮겨 적지 않는다. 이미 상위 코드를 읽은 사람이 쓰는 재구현은 독립성을 입증하기 어렵다는 점을 인지한다.

## D4. git 이력의 원본 자료: B, 이력을 재작성해 새 저장소로 이전 — 결정됨 (2026-09-20)

- **사실**: 이력에 `res/minebomb/`(130개)와 `res/minebomb.zip`, JS-DOS 번들(`apps/client/public/minebomb.{jsdos,zip}`), 변환 오디오(`web/audio/`, `apps/client/public/assets/audio/`), 46개 맵을 변환한 `packages/shared/src/game/classicMapsData.json`, 원본이 내장된 과거 `web/pkg/mb_wasm.wasm`이 있었습니다. 재작성 리허설 중에 **앱 아이콘(`res/minebombers.ico`, `web/favicon.ico`)이 원작 타이틀 화면을 잘라 축소한 이미지**이고 이 아이콘이 `MineBombers.exe`에도 내장돼 있음을 추가로 확인했습니다(바이트 검색은 축소된 파생물을 못 잡습니다). 포크·스타·릴리스·태그 0개.
- **대안**: (A) 같은 저장소에서 재작성 + force-push + GitHub 지원에 캐시/PR 참조 삭제 요청 / (B) 재작성한 이력을 새 저장소로 옮기고 기존 저장소 삭제 / (C) 비공개 전환만.
- **이유**: 기존 저장소에서는 브랜치를 고쳐도 `refs/pull/*`와 SHA 직접 조회에 옛 객체가 남고, 사용자가 이를 지울 수 없습니다. 포크·스타·릴리스가 없어 이전 비용이 작습니다.
- **조치**: 위 경로에 `MineBombers.exe`, 두 아이콘, 쓰지 않는 JS-DOS 에뮬레이터(`apps/client/public/js-dos/`)를 더해 이력 전체에서 제거하고, 아이콘은 도형으로 직접 그린 것으로 교체했습니다.
- **검증**: 삭제 경로를 건드리는 커밋 0개, 삭제 경로 blob 261개 중 잔존 0개, 원본 바이트가 든 객체 613개 중 156개 → 360개 중 0개, 커밋 48개 → 46개, 최상단 브랜치에서 테스트 통과, wasm 재빌드 결과가 삭제 전과 바이트 동일.
- **한계**: 압축·재인코딩·축소된 형태(ZIP, JS-DOS 번들, JSON, MP3, 아이콘 같은 축소 이미지)는 바이트 검색에 걸리지 않아 경로, blob ID, 눈으로 본 출처 확인에만 의존했습니다. 코드로 생성한 과거 스프라이트(`scripts/generate_retro_art.cjs` 산출물)가 원작을 모사했는지는 판정하지 못했습니다. 이미 다른 사람이 받아 간 사본은 회수할 수 없습니다.

## D7. Windows 릴리스는 시험용(pre-release) zip으로, 코드 서명 없이 배포한다 — 결정됨 (2026-09-20)

- **내용**: `v*` 태그를 푸시하면 `release.yml`이 windows-latest에서 빌드·테스트하고 `scripts/package_windows.ps1`로 zip(exe, SDL2 계열 DLL 8개, README.txt, NOTICE.txt)과 `.sha256`을 만들어 pre-release로 올립니다. 원작 게임 파일은 넣지 않습니다(D1). exe는 정적 CRT(`+crt-static`)로 빌드해 Visual C++ 재배포 패키지가 필요 없습니다.
- **대안**: 코드 서명(인증서 비용과 발급 절차가 필요함) / 설치 관리자 / 정식(latest) 릴리스.
- **이유**: 서명 없이도 zip은 SmartScreen 경고 한 번으로 실행되고, 코드 라이선스(D3)가 정리되지 않았으므로 정식 릴리스로 표시하지 않습니다.
- **한계**: 서명이 없어 SmartScreen 경고가 남습니다. DLL 8개 중 `SDL2.dll`을 뺀 7개는 공식 SDL2_mixer 2.8.0 Windows 패키지와 바이트가 같음을 확인했고, 그 패키지의 라이선스 원문(SDL_mixer, gme, xmp, ogg, opus, opusfile, wavpack)을 `packaging/windows/licenses/`에 그대로 넣어 zip에 동봉합니다(v0.1.0 zip에는 없었고 v0.1.1부터 들어갑니다). `SDL2.dll`(2.30.8)도 공식 SDL2-2.30.8-win32-x64.zip과 바이트가 같음을 확인했고(2026-09-24), 그 패키지의 README-SDL.txt와 같은 태그 소스의 LICENSE.txt(`LICENSE.SDL2.txt`)를 함께 동봉합니다(v0.1.3부터). `libgme.dll`은 LGPL-2.1이라 NOTICE.txt에 소스 위치(libsdl-org/game-music-emu, SDL_mixer 2.8.0이 고정한 커밋)를 적었습니다.
