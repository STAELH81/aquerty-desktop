$Host.UI.RawUI.WindowTitle = "Arret automatique du PC"

Clear-Host

Write-Host "========================================"
Write-Host "       ARRET AUTOMATIQUE DU PC"
Write-Host "========================================"
Write-Host ""
Write-Host "Formats acceptes :"
Write-Host "  30s"
Write-Host "  45m"
Write-Host "  2h"
Write-Host "  1h20m"
Write-Host "  2h30m15s"
Write-Host ""

$temps = (Read-Host "Temps avant l'arret").Trim().ToLower().Replace(" ", "")

# Analyse directement le texte sans utiliser $Matches
$regex = [regex]'^(?=\d)(?:(?<h>\d+)h)?(?:(?<m>\d+)m)?(?:(?<s>\d+)s)?$'
$resultat = $regex.Match($temps)

if (-not $resultat.Success) {
    Write-Host ""
    Write-Host "ERREUR : format invalide." -ForegroundColor Red
    Write-Host "Exemples : 30m, 1h20m, 2h30m15s"
    Write-Host ""
    Read-Host "Appuie sur Entree"
    exit
}

$heures = 0
$minutes = 0
$secondes = 0

if ($resultat.Groups["h"].Success) {
    $heures = [int]$resultat.Groups["h"].Value
}

if ($resultat.Groups["m"].Success) {
    $minutes = [int]$resultat.Groups["m"].Value
}

if ($resultat.Groups["s"].Success) {
    $secondes = [int]$resultat.Groups["s"].Value
}

$total = ($heures * 3600) + ($minutes * 60) + $secondes

if ($total -le 0) {
    Write-Host ""
    Write-Host "ERREUR : le temps doit etre superieur a 0." -ForegroundColor Red
    Write-Host ""
    Read-Host "Appuie sur Entree"
    exit
}

Write-Host ""
Write-Host "========================================"
Write-Host "           CONFIRMATION"
Write-Host "========================================"
Write-Host ""
Write-Host "Temps demande : $temps"
Write-Host ""
Write-Host "  Heures   : $heures"
Write-Host "  Minutes  : $minutes"
Write-Host "  Secondes : $secondes"
Write-Host ""
Write-Host "Le PC s'eteindra dans $temps."
Write-Host ""

$confirmation = Read-Host "Confirmer ? (O/N)"

if ($confirmation.ToLower() -ne "o") {
    Write-Host ""
    Write-Host "Arret annule." -ForegroundColor Yellow
    Write-Host ""
    Read-Host "Appuie sur Entree"
    exit
}

shutdown.exe /s /t $total

if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Green
    Write-Host "       ARRET PROGRAMME !" -ForegroundColor Green
    Write-Host "========================================" -ForegroundColor Green
    Write-Host ""
    Write-Host "Le PC s'eteindra dans $temps."
    Write-Host ""
    Write-Host "Pour ANNULER l'arret :"
    Write-Host "shutdown /a" -ForegroundColor Cyan
}
else {
    Write-Host ""
    Write-Host "ERREUR lors de la programmation de l'arret." -ForegroundColor Red
}

Write-Host ""
Read-Host "Appuie sur Entree pour fermer"