use std::collections::BTreeSet;

pub fn compute_error_lines(lines: &[String]) -> BTreeSet<usize> {
    let mut errors = BTreeSet::new();
    let mut stack: Vec<(char, usize)> = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        let mut in_str = false;
        let mut prev = '\0';
        for ch in line.chars() {
            if ch == '"' && prev != '\\' {
                in_str = !in_str;
            }
            if in_str {
                prev = ch;
                continue;
            }
            match ch {
                '(' | '{' | '[' => stack.push((ch, idx)),
                ')' => pop_match(&mut stack, '(', idx, &mut errors),
                '}' => pop_match(&mut stack, '{', idx, &mut errors),
                ']' => pop_match(&mut stack, '[', idx, &mut errors),
                _ => {}
            }
            prev = ch;
        }
        if in_str {
            errors.insert(idx);
        }
    }

    for (_, idx) in stack {
        errors.insert(idx);
    }
    errors
}

fn pop_match(
    stack: &mut Vec<(char, usize)>,
    expected: char,
    idx: usize,
    errors: &mut BTreeSet<usize>,
) {
    match stack.pop() {
        Some((open, _)) if open == expected => {}
        _ => {
            errors.insert(idx);
        }
    }
}
