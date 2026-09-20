# 결정 기록

되돌리기 어려운 결정만 적습니다. 각 항목: 상태, 사실, 대안, 이유/결과.

## D1. 게임 데이터는 사용자가 준비한다 (OpenRCT2 방식) — 결정됨 (2026-09-20)

- **사실**: 원작 패키지의 문서가 서로 모순됩니다(`FILE_ID.DIZ`는 "MAY BE DISTRIBUTED", `MINEENG.TXT`는 "복사·배포는 불법"). 변환·내장(파생)에 대한 언급은 없습니다. 상위 `mb-reloaded`, ScummVM, OpenRA, OpenTyrian, OpenRCT2는 원본 데이터를 소스 저장소에 넣지 않습니다.
- **대안**: 원본 동봉 / 사이트가 원본을 제공 / 권리자 허락을 받고 동봉 / 대체 에셋 제작.
- **결과**: 저장소·배포 사이트·WASM에 원본이 없고, 사용자가 자기 사본을 고릅니다(PR #12). 웹 데모는 처음에 파일을 한 번 골라야 열립니다. git 이력에는 원본이 남아 있습니다(D4).

## D2. 제품 방향: 원작 3.11을 최신 Windows/Linux에서 돌리는 이식 — 제안됨 (2026-09-20)

- **내용**: 원작 데이터와 호환되는 엔진이 목표이고 원작 규칙(클래식)이 기준입니다. 봇, 신규 모드, 넷플레이, 웹판 같은 확장은 클래식과 분리된 선택 기능으로 둡니다.
- **이유**: 유지관리자가 제시한 방향이며, D1의 데이터 모델과 일관됩니다. 이 저장소는 짧은 기간에 방향이 여러 번 바뀌었고(TS 웹 멀티플레이 → JS-DOS → 네이티브 → WASM) 문서가 첫 방향에 머물러 있었습니다.
- **검증 상태**: Windows는 로컬에서 확인했습니다. **Linux는 아직 한 번도 빌드해 보지 않았고**, CI 매트릭스(`.github/workflows/ci.yml`)로 확인합니다. 확인 전에는 README에 Linux 지원을 적지 않습니다.
- **열린 문제 1, 확장이 원작 데이터를 오염시킴**: 신규 모드(`WinCondition::GoldRush` = 2, `Survival` = 3)가 원작 게임 자신의 `OPTIONS.CFG` 승리 조건 바이트에 저장됩니다(`crates/mb-core/src/options.rs`). 원작과 상위 저장소에는 0/1(돈/승수)뿐이라, 이 엔진을 실행하면 사용자의 원작 설정 파일에 원작이 모르는 값이 기록될 수 있습니다. 원작 DOS 판이 그 값을 읽을 때 어떻게 동작하는지는 확인하지 못했습니다. 확장 설정은 자체 설정 파일에 두고 `OPTIONS.CFG`에는 원작 값만 써야 합니다.
- **열린 문제 2, 쓰기 위치**: 설정(`config.toml`), 옵션(`OPTIONS.CFG`), 기록(`HIGHSCOR.DAT`)이 모두 게임 폴더에 쓰입니다. 설치 폴더가 읽기 전용인 Windows/Linux에서는 사용자 폴더로 옮겨야 하고, 원작 파일은 읽기 전용으로 다뤄야 합니다.

## D3. 엔진 코드의 출처 — 미결

- **사실** (2026-09-20 측정, 같은 경로 파일의 의미 있는 줄 일치율): `src/`의 73%, `crates/mb-core`의 75%가 상위 `mb-reloaded`(라이선스 없음)와 같은 줄입니다. `bitmaps.rs`, `roster.rs`는 100%. 새로 쓴 것은 `world/bot.rs`, `config.rs`, `crates/mb-wasm`입니다.
- **대안**: (a) 상위 저자가 라이선스를 부여([idubrov/mb-reloaded#1](https://github.com/idubrov/mb-reloaded/issues/1) 대기 중) / (b) 유지관리자가 독립적으로 다시 구현 / (c) 현 상태 유지(권장하지 않음).
- **유지관리자 의견**: (b) 지향("엔진을 직접 만든다").
- **(b)의 전제**: 규칙의 출처를 문서로 남긴다(원본 파일 포맷, DOS 원작 관찰). 모듈별 출처 표를 유지한다. 기존 코드를 보며 옮겨 적지 않는다. 이미 상위 코드를 읽은 사람이 쓰는 재구현은 독립성을 입증하기 어렵다는 점을 인지한다.

## D4. git 이력에 남은 원본 자료 — 미결

- **사실** (2026-09-20): 이력에 `res/minebomb/`(130개)와 `res/minebomb.zip`, JS-DOS 번들(`apps/client/public/minebomb.{jsdos,zip}`), 변환 오디오(`web/audio/`, `apps/client/public/assets/audio/`), 46개 맵을 변환한 `packages/shared/src/game/classicMapsData.json`, 원본이 내장된 과거 `web/pkg/mb_wasm.wasm`이 있습니다. 커밋 41개, `.git` 52 MB, 포크·스타·릴리스·태그 0개, `main` 보호 없음.
- **대안**: (A) `git filter-repo`로 이력 재작성 + force-push + GitHub 지원에 캐시/PR 참조 삭제 요청 / (B) 재작성한 이력을 새 저장소로 옮기고 기존 저장소 삭제 / (C) 비공개 전환만.
- **주의**: 브랜치를 고쳐도 GitHub의 `refs/pull/*` 참조와 SHA 직접 조회는 남습니다(A의 한계).
