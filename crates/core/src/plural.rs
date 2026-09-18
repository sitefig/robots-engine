//! CLDR cardinal plural categories for the 24 official EU languages, enough
//! for the dictionaries' `{ one, few, many, other }` forms without shipping
//! the full CLDR tables. Unknown languages use the English rule.

/// The CLDR category name for `n` in language `lang`.
pub fn category(lang: &str, n: f64) -> &'static str {
    let is_int = n.fract() == 0.0 && n.is_finite();
    let i = n.abs().trunc() as u64; // integer digits
    let v_nonzero = !is_int; // has visible fraction digits
    let m10 = i % 10;
    let m100 = i % 100;
    match lang {
        "fr" => {
            if i == 0 || i == 1 { "one" } else { "other" }
        }
        "cs" | "sk" => {
            if !is_int { "many" } else if i == 1 { "one" } else if (2..=4).contains(&i) { "few" } else { "other" }
        }
        "pl" => {
            if !is_int {
                "other"
            } else if i == 1 {
                "one"
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                "few"
            } else {
                "many"
            }
        }
        "hr" => {
            if !is_int {
                "other"
            } else if m10 == 1 && m100 != 11 {
                "one"
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                "few"
            } else {
                "other"
            }
        }
        "sl" => {
            if v_nonzero {
                "few"
            } else if m100 == 1 {
                "one"
            } else if m100 == 2 {
                "two"
            } else if m100 == 3 || m100 == 4 {
                "few"
            } else {
                "other"
            }
        }
        "lt" => {
            if !is_int {
                "many"
            } else if m10 == 1 && !(11..=19).contains(&m100) {
                "one"
            } else if (2..=9).contains(&m10) && !(11..=19).contains(&m100) {
                "few"
            } else {
                "other"
            }
        }
        "lv" => {
            if is_int && (m10 == 0 || (11..=19).contains(&m100)) {
                "zero"
            } else if is_int && m10 == 1 && m100 != 11 {
                "one"
            } else {
                "other"
            }
        }
        "ro" => {
            if is_int && i == 1 {
                "one"
            } else if v_nonzero || i == 0 || (2..=19).contains(&m100) {
                "few"
            } else {
                "other"
            }
        }
        "ga" => {
            if !is_int {
                "other"
            } else if i == 1 {
                "one"
            } else if i == 2 {
                "two"
            } else if (3..=6).contains(&i) {
                "few"
            } else if (7..=10).contains(&i) {
                "many"
            } else {
                "other"
            }
        }
        "mt" => {
            if !is_int {
                "other"
            } else if i == 1 {
                "one"
            } else if i == 2 {
                "two"
            } else if i == 0 || (3..=10).contains(&m100) {
                "few"
            } else if (11..=19).contains(&m100) {
                "many"
            } else {
                "other"
            }
        }
        // en de nl sv da fi et el hu bg it es pt and anything else: 1 is "one".
        _ => {
            if is_int && i == 1 { "one" } else { "other" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::category;

    #[test]
    fn rules() {
        assert_eq!(category("en", 1.0), "one");
        assert_eq!(category("en", 2.0), "other");
        assert_eq!(category("fr", 0.0), "one");
        assert_eq!(category("cs", 3.0), "few");
        assert_eq!(category("cs", 7.0), "other");
        assert_eq!(category("pl", 22.0), "few");
        assert_eq!(category("pl", 12.0), "many");
        assert_eq!(category("lv", 10.0), "zero");
        assert_eq!(category("ga", 5.0), "few");
        assert_eq!(category("sl", 102.0), "two");
        assert_eq!(category("ro", 5.0), "few");
        assert_eq!(category("ro", 25.0), "other");
    }
}
