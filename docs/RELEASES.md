# Releases automatiques

## Où vont les clés ?

| Clé | Où | Commit GitHub ? |
|-----|-----|-----------------|
| **Publique** (`.pub`) | Déjà dans `src-tauri/tauri.conf.json` → `plugins.updater.pubkey` | Oui, safe |
| **Privée** (`.key`) | Secret GitHub uniquement | **NON jamais** |

## Configurer le secret (une seule fois)

1. Ouvre https://github.com/STAELH81/aquerty-desktop/settings/secrets/actions
2. **New repository secret**
3. Name : `TAURI_SIGNING_PRIVATE_KEY`
4. Value : colle **tout** le contenu de  
   `C:\Users\sacha\.tauri\aquerty.key`  
   (ouvre-le avec Notepad, Ctrl+A, Ctrl+C)
5. Si tu as mis un mot de passe à la génération, crée aussi  
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`  
   Sinon laisse vide / ne crée pas le secret (le workflow tolère l’absence)

## Sortir une nouvelle version (à chaque update)

1. Monte la version au **même numéro** dans :
   - `package.json`
   - `src-tauri/tauri.conf.json`
   - `src-tauri/Cargo.toml`  
   Exemple : `1.1.0` → `1.2.0`

2. Commit + push ton code :

```powershell
git add -A
git commit -m "Release 1.2.0"
git push
```

3. Crée le tag et pousse-le (c’est ça qui lance le build auto) :

```powershell
git tag v1.2.0
git push origin v1.2.0
```

4. Va sur **Actions** du repo → le job **Release** tourne (~10–20 min)
5. Quand c’est vert → onglet **Releases** : setup `.exe` + `latest.json` sont uploadés

L’app installée pointe déjà vers :

`https://github.com/STAELH81/aquerty-desktop/releases/latest/download/latest.json`

Donc **Réglages → Vérifier les mises à jour** récupère la dernière release.

## Checklist rapide

- [ ] Secret `TAURI_SIGNING_PRIVATE_KEY` ajouté
- [ ] Versions bumpées (3 fichiers)
- [ ] Code pushé
- [ ] Tag `vX.Y.Z` pushé
- [ ] Actions vert
- [ ] Release visible sur GitHub
