pub fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn is_variable_name(text: &str) -> bool {
    let Some(name) = text.strip_prefix('$') else {
        return false;
    };
    is_identifier(name)
}

pub fn is_constant_name(name: &str) -> bool {
    let mut has_alpha = false;
    for ch in name.chars() {
        if ch.is_ascii_alphabetic() {
            has_alpha = true;
            if !ch.is_ascii_uppercase() {
                return false;
            }
        } else if !(ch.is_ascii_digit() || ch == '_') {
            return false;
        }
    }
    has_alpha
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_name_rules() {
        assert!(is_constant_name("NAME"));
        assert!(is_constant_name("MAX_SIZE"));
        assert!(is_constant_name("API_V2"));
        assert!(is_constant_name("_TEMP"));
        assert!(!is_constant_name("name"));
        assert!(!is_constant_name("User"));
    }
}
