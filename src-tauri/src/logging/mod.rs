pub fn event(name: &str, fields: &[(&str, &str)]) {
    let details = fields
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(" ");

    if details.is_empty() {
        log::info!("{name}");
    } else {
        log::info!("{name} {details}");
    }
}
