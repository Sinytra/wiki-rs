use std::collections::HashMap;

/// Infer tab names as item variants in cases where repeated prefixes/suffixes appear in item names
/// For example: *Iron* Ingot, *Gold* Ingot, ...
pub fn infer_tab_names(names: &[String]) -> Vec<String> {
    if names.len() < 2 {
        return names.to_vec();
    }

    let words: Vec<Vec<&str>> = names
        .iter()
        .map(|n| n.split_whitespace().collect())
        .collect();
    let prefix = find_common_affix(names.len(), words.clone());
    let mut suffix = find_common_affix(
        names.len(),
        words
            .iter()
            .map(|w| w.iter().rev().copied().collect())
            .collect(),
    );
    suffix.reverse();

    names
        .iter()
        .zip(&words)
        .map(|(name, word)| {
            let start = if word.starts_with(&prefix) {
                prefix.len()
            } else {
                0
            };
            let end = if word[start..].ends_with(&suffix) {
                word.len() - suffix.len()
            } else {
                word.len()
            };
            if start == end {
                name.clone()
            } else {
                word[start..end].join(" ")
            }
        })
        .collect()
}

fn find_common_affix(total: usize, mut names: Vec<Vec<&str>>) -> Vec<&str> {
    let mut affix = Vec::new();

    loop {
        let position = affix.len();
        let mut word_count: HashMap<&str, usize> = HashMap::new();

        for name in &names {
            if name.len() > position + 1 {
                *word_count.entry(name[position]).or_default() += 1;
            }
        }

        let Some((most_common, count)) =
            word_count.into_iter().max_by_key(|(_, count)| *count)
        else {
            break;
        };

        if count * 2 <= total {
            break;
        }

        affix.push(most_common);

        names.retain(|word| {
            word.len() > position + 1 && word[position] == most_common
        });
    }

    affix
}