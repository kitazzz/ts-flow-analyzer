use std::path::Path;

pub fn load_source(path: &Path) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path)
}
