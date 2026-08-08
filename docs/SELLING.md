# Guide Gumroad + vente Aquerty Stop

## Remplacer quoi ?

Deux fichiers ont des **fausses URLs** en attendant tes vrais liens Gumroad :

| Fichier | Ligne à changer |
|---------|-----------------|
| [`landing/config.js`](../landing/config.js) | `gumroadLifetime` et `gumroadAnnual` |
| [`src/commerce.ts`](../src/commerce.ts) | idem |

Aujourd’hui tu as :

```text
https://gumroad.com/l/REPLACE_AQUERTY_LIFETIME
https://gumroad.com/l/REPLACE_AQUERTY_ANNUAL
```

Après création des produits Gumroad, tu copies l’URL réelle du produit (ex. `https://staelh81.gumroad.com/l/aquerty-pro`) et tu **remplaces** ces deux chaînes dans les 2 fichiers.  
Sinon les boutons “Acheter” de la landing et de l’app mènent nulle part.

`downloadUrl` / Releases : **ne touche pas** (déjà bon).

---

## Plan Gumroad (pas à pas)

### A. Compte
1. Va sur [gumroad.com](https://gumroad.com) → crée un compte / connecte-toi
2. Remplis le profil vendeur (pays, payout) pour pouvoir encaisser

### B. Produit 1 - Pro à vie (9,99 €)
1. **New product**
2. Nom : `Aquerty Stop Pro` (ou `Aquerty Stop Pro - Lifetime`)
3. Prix : **9.99 EUR**, type **one-time** (pas subscription)
4. Description courte : Pro = veille, hibernation, lock, conditions, multi-récurrence, wake, thème…
5. Section **Content** / after purchase :
   - Active **License keys** (ou “Generate license keys” / “Send license keys”)
   - Tu colleras plus tard la liste de clés (étape C)
6. Publie le produit → copie l’**URL** → colle-la dans `config.js` + `commerce.ts` (lifetime)

### C. Générer 50 clés lifetime (c’est le “3.”)
Ça crée les codes `AQUERTY-LIFE-…` à donner aux acheteurs.

Dans un terminal (MSVC / vcvars si besoin) :

```bat
call "D:\VSBuildTools\VC\Auxiliary\Build\vcvars64.bat"
set CARGO_TARGET_DIR=D:\aquerty-cargo-target
cd /d "C:\Users\sacha\Documents\Code\Aquerty Stop\src-tauri"
cargo run --bin gen-license -- batch-lifetime 50 GUM
```

Tu obtiens 50 lignes. Sur le produit Lifetime Gumroad → **License keys** → colle toute la liste → Save.  
Gumroad envoie **une clé unique** par acheteur dans l’email.

### D. Produit 2 - Pro annuel (2,99 € / an)
1. **New product** → nom `Aquerty Stop Pro - Annual`
2. Prix **2.99 EUR**, type **subscription** / yearly
3. Contenu : texte du genre  
   `Ta clé Pro annuelle arrive par email sous 24h (ou contacte-nous).`
4. Publie → copie l’URL → `config.js` + `commerce.ts` (annual)

**Annuel v1 (simple) :** quand quelqu’un paie, tu lances :

```bat
cargo run --bin gen-license -- annual CLIENT01
```

et tu envoies la clé par email Gumroad / réponse. (L’auto-liste pour l’abo, plus tard.)

### E. GitHub Pages (site)
1. Repo → **Settings → Pages** → Source **GitHub Actions**
2. Push `landing/` sur `main` → le site sort sur  
   `https://staelh81.github.io/aquerty-desktop/`

### F. “4. Démo” - c’tà dire ?
Pas la place disque. Ça veut dire : à la **sortie 1.1.2**, la clé  
`AQUERTY-PRO-DEMO-2026` **ne marche plus**.  
Ceux qui l’avaient en Pro repassent Free. Les vraies clés Gumroad, oui.

### G. Place disque
Avant ça avait planté (`Espace insuffisant` sur D:).  
**Maintenant D: ~172 Go libres** → tu peux lancer `gen-license` sans souci.  
(C: est encore juste ~3 Go : laisse le cargo target sur D:.)

---

## Ordre recommandé cette semaine

1. Compte Gumroad + produit **Lifetime 9,99 €**
2. Générer 50 clés + les coller dans Gumroad
3. Produit **Annual 2,99 €**
4. Remplacer les 2 URLs `REPLACE_…` dans les 2 fichiers
5. Activer Pages + push landing
6. Le **14 août** : release 1.1.2 (démo morte) + test d’achat / d’une clé
