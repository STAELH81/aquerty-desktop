# Aquerty Stop

Application Windows premium pour programmer l'arrêt, le redémarrage, la veille, l'hibernation ou le verrouillage — avec conditions intelligentes.

Repo : [STAELH81/aquerty-desktop](https://github.com/STAELH81/aquerty-desktop)

## Stack

- **Tauri 2** + Rust (actions système, tray, timer)
- **React** + TypeScript + Vite (UI)

## Prérequis

- Node.js 20+
- Rust (stable)
- Visual Studio Build Tools 2022 avec charge de travail **Desktop development with C++**

## Développement

```bash
npm install
# Dans un terminal "Developer Command Prompt for VS" (ou via build.bat pour la release) :
npm run tauri dev
```

Sous Windows, le linker MSVC doit être dans le PATH (`vcvars64.bat`).

## Build installateur

```bash
build.bat
```

Ou :

```bash
npm run tauri build
```

Les installateurs sont générés dans `D:\aquerty-cargo-target\release\bundle\` (et copiés dans `dist-installers/` après un build réussi) :

- `Aquerty Stop_1.0.0_x64-setup.exe` (NSIS)
- `Aquerty Stop_1.0.0_x64_en-US.msi`

## Licence freemium

- **Gratuit** : arrêt, redémarrage, 4 presets
- **Pro** : veille, hibernation, verrouillage, conditions intelligentes, presets illimités

Clé de démo : `AQUERTY-PRO-DEMO-2026`

## Legacy

Les anciens scripts batch/PowerShell sont dans `legacy/`.
