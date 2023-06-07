pub async fn handle_doctor() -> Result<(), Box<dyn std::error::Error>> {
    println!("Topos CLI: version {}", env!("TOPOS_VERSION"));

    Ok(())
}
