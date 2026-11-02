#[path = "../src/icon.rs"]
mod icon;
fn main() {
    let dir = std::path::Path::new("target/icon-preview");
    std::fs::create_dir_all(dir).unwrap();
    for value in [None, Some(0.), Some(7.), Some(37.), Some(100.), Some(999.), Some(1000.)] {
        let label = match value { None => "unknown".into(), Some(_) => icon::text(value) };
        std::fs::write(dir.join(format!("{label}.ico")), icon::ico(value)).unwrap();
    }
}
