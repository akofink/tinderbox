//! Player-facing names. Rules and saves keep snake_case ids.

pub fn display_name(name: &str) -> String {
    let pretty = name.replace('_', " ");
    let mut chars = pretty.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub fn group(name: &str) -> &'static str {
    match name {
        "air" | "sand" | "soil" | "wet_soil" | "fertile_soil" | "stone" | "humus" => "Ground",
        "water" | "ice" | "steam" | "cloud" | "lava" | "oil" => "Fluids",
        "tree" | "plant" | "seed" | "deadwood" | "leaf_litter" | "fungus" | "beetle"
        | "termite" => "Life",
        "fire" | "oil_fire" | "ash" | "smoke" => "Fire",
        _ => "Other",
    }
}

pub const GROUPS: [&str; 5] = ["Ground", "Fluids", "Life", "Fire", "Other"];

#[cfg(test)]
mod tests {
    use super::*;
    use tinderbox_sim::{DEFAULT_RULES, Rules};

    #[test]
    fn default_elements_have_readable_grouped_names() {
        let rules = Rules::load(DEFAULT_RULES).unwrap();
        for element in &rules.elements {
            let label = display_name(&element.name);
            assert!(!label.contains('_'), "{label}");
            assert!(GROUPS.contains(&group(&element.name)));
        }
        assert_eq!(display_name("wet_soil"), "Wet soil");
        assert_eq!(display_name("oil_fire"), "Oil fire");
    }
}
