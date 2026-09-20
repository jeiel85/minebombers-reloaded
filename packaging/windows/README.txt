Mine Bombers: Reloaded (Windows, 64-bit)
=========================================

An unofficial engine that runs the 1995 DOS game Mine Bombers 3.11 (Skitso Productions)
natively on Windows, without DOSBox. Project page and source:
https://github.com/jeiel85/minebombers-reloaded

이 압축 파일에는 원작 게임 파일이 들어 있지 않습니다. 원작 3.11(프리웨어)을 직접 준비해야 합니다.
This zip does NOT contain the original game files. You need your own copy of Mine Bombers 3.11.


시작하기 / Getting started
--------------------------
1. 원작 Mine Bombers 3.11을 받아 압축을 풉니다.
   Download Mine Bombers 3.11 and unzip it, e.g. https://archive.org/details/mnb311fw
2. 이 압축을 아무 폴더에 풀고 MineBombers.exe를 실행합니다. (DLL 파일은 exe와 같은 폴더에 두세요.)
   Unzip this package anywhere and run MineBombers.exe. Keep the DLL files next to the exe.
3. 처음 실행하면 폴더 선택 창이 열립니다. TITLEBE.SPY가 들어 있는 폴더(원작을 푼 폴더)를 고르세요.
   한 번 고르면 기억합니다.
   On first run a folder chooser opens. Pick the folder that contains TITLEBE.SPY.
   The choice is remembered.

다른 폴더를 고르려면 / To pick another folder:
  MineBombers.exe --choose-game-folder

Windows의 "PC 보호" 창이 뜨면 / If Windows SmartScreen warns you
------------------------------------------------------------------
이 exe에는 코드 서명이 없어서 처음 실행할 때 "Windows의 PC 보호" 창이 뜰 수 있습니다.
"추가 정보"를 누른 뒤 "실행"을 선택하세요. 받은 파일이 맞는지는 함께 올린 .sha256 파일로
확인할 수 있습니다.
The exe is not code-signed, so SmartScreen may show a warning. Click "More info", then "Run anyway".
Compare the zip's SHA-256 with the .sha256 file published next to it if you want to verify it.
  PowerShell:  Get-FileHash .\MineBombers-*-windows-x64.zip -Algorithm SHA256


설정과 기록 / Settings and saved data
--------------------------------------
게임 폴더는 읽기만 하고 아무것도 쓰지 않습니다. 설정, 기록, 선수 명단은 여기에 저장됩니다:
The game folder is only read, never written. Settings, high scores and players are stored in:
  %APPDATA%\MineBombers

지우려면 이 압축을 푼 폴더와 위 폴더를 삭제하면 됩니다.
To remove everything, delete the folder you unzipped and the folder above.

명령행 옵션 / Command line
--------------------------
  MineBombers.exe <폴더>          이번 실행에만 그 폴더를 씁니다 / use that game folder for this run only
  MineBombers.exe --campaign      싱글플레이 캠페인 / single-player campaign
  MineBombers.exe --help          도움말 / help
  환경변수 MINEBOMBERS_GAME_DIR   게임 폴더 지정 / game folder
  환경변수 MINEBOMBERS_USER_DIR   설정과 기록 폴더 지정 / settings and data folder


알려진 한계 / Known limitations
-------------------------------
- 이 프로젝트의 코드 라이선스는 아직 정리되지 않았습니다. 자세한 내용은 NOTICE.txt와 저장소의 DECISIONS.md를 보세요.
  The code license of this project is not settled yet. See NOTICE.txt and DECISIONS.md in the repository.
- 시험용 릴리스(pre-release)입니다. 문제는 GitHub Issues에 알려 주세요.
  This is a pre-release. Please report problems on GitHub Issues.
