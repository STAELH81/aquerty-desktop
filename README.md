# Aquerty Stop

Timer Windows pour programmer l'arret, le redemarrage, la veille, l'hibernation ou le verrouillage, avec conditions optionnelles.

Repo : [STAELH81/aquerty-desktop](https://github.com/STAELH81/aquerty-desktop)

## Stack

- Tauri 2 + Rust
- React + TypeScript + Vite

## Prerequisites

- Node.js 20+
- Rust (stable)
- Visual Studio Build Tools 2022 (Desktop development with C++)

## Dev

```bash
npm install
npm run tauri dev
```

MSVC doit etre dans le PATH (`vcvars64.bat`).

## Build

```bash
build.bat
```

Ou :

```bash
npm run tauri build
```

## Updates

Updater GitHub Releases :

`https://github.com/STAELH81/aquerty-desktop/releases/latest/download/latest.json`

Details dans `docs/RELEASES.md`.

## Licence

- Gratuit : arret, redemarrage, 4 presets, 3 profils, 1 regle recurrente, delai de grace, FR/EN
- Pro : veille, hibernation, verrouillage, conditions, theme, wake PC, multi-recurrence, plus de profils
- Prix : **9,99 € a vie** ou **2,99 € / an** (Gumroad) - details dans [`docs/SELLING.md`](docs/SELLING.md)
- Landing : dossier [`landing/`](landing/) (GitHub Pages)

A partir de 1.1.2 la cle demo n'est plus acceptee.

## Legacy

Anciens scripts dans `legacy/`.
