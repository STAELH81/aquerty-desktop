use crate::settings::Locale;

pub fn action_label(locale: &Locale, action: crate::power::PowerAction) -> &'static str {
    use crate::power::PowerAction::*;
    match (locale, action) {
        (Locale::Fr, Shutdown) => "Arrêt",
        (Locale::Fr, Restart) => "Redémarrage",
        (Locale::Fr, Sleep) => "Veille",
        (Locale::Fr, Hibernate) => "Hibernation",
        (Locale::Fr, Lock) => "Verrouillage",
        (Locale::En, Shutdown) => "Shutdown",
        (Locale::En, Restart) => "Restart",
        (Locale::En, Sleep) => "Sleep",
        (Locale::En, Hibernate) => "Hibernate",
        (Locale::En, Lock) => "Lock",
    }
}

pub fn msg(locale: &Locale, key: &str) -> String {
    let en = locale.is_en();
    match key {
        "pro_power" => {
            if en {
                "Pro: sleep, hibernate and lock.".into()
            } else {
                "Fonction Pro : veille, hibernation et verrouillage.".into()
            }
        }
        "pro_conditions" => {
            if en {
                "Pro: conditions require a Pro license.".into()
            } else {
                "Fonction Pro : conditions réservées à la licence Pro.".into()
            }
        }
        "pro_recurring" => {
            if en {
                "Pro: more than one recurring rule.".into()
            } else {
                "Fonction Pro : plusieurs règles récurrentes.".into()
            }
        }
        "pro_wake" => {
            if en {
                "Pro: wake PC to run the action.".into()
            } else {
                "Fonction Pro : réveiller le PC pour l'action.".into()
            }
        }
        "invalid_license" => {
            if en {
                "Invalid license key.".into()
            } else {
                "Clé de licence invalide.".into()
            }
        }
        "delay_or_condition" => {
            if en {
                "Delay must be greater than 0, or add a condition.".into()
            } else {
                "Le délai doit être supérieur à 0, ou ajoutez une condition.".into()
            }
        }
        "alert_5m" => {
            if en {
                "5 minutes left before the action.".into()
            } else {
                "Plus que 5 minutes avant l'action.".into()
            }
        }
        "alert_1m" => {
            if en {
                "One minute left, action imminent.".into()
            } else {
                "Plus qu'une minute, action imminente.".into()
            }
        }
        "alert_soon" => {
            if en {
                "Action imminent.".into()
            } else {
                "Action imminente.".into()
            }
        }
        "alert_grace" => {
            if en {
                "Grace period: cancel now to abort.".into()
            } else {
                "Délai de grâce : annulez pour interrompre.".into()
            }
        }
        "waiting" => {
            if en {
                "Aquerty Stop - waiting".into()
            } else {
                "Aquerty Stop - en attente".into()
            }
        }
        "grace_title" => {
            if en {
                "Aquerty Stop - grace".into()
            } else {
                "Aquerty Stop - grâce".into()
            }
        }
        "empty_time" => {
            if en {
                "Empty time.".into()
            } else {
                "Temps vide.".into()
            }
        }
        "invalid_number" => {
            if en {
                "Invalid number.".into()
            } else {
                "Nombre invalide".into()
            }
        }
        "time_gt_zero" => {
            if en {
                "Time must be greater than 0.".into()
            } else {
                "Le temps doit être supérieur à 0.".into()
            }
        }
        "invalid_format" => {
            if en {
                "Invalid format. Examples: 30m, 1h20m, 2h30m15s".into()
            } else {
                "Format invalide. Exemples : 30m, 1h20m, 2h30m15s".into()
            }
        }
        _ => key.into(),
    }
}

pub fn action_in_progress(locale: &Locale, action: crate::power::PowerAction) -> String {
    let label = action_label(locale, action);
    if locale.is_en() {
        format!("{label} in progress…")
    } else {
        format!("{label} en cours…")
    }
}
