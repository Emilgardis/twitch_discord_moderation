fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    built::write_built_file()?;
    Ok(())
}
