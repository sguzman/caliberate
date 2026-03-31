fn main() -> Result<(), Box<dyn std::error::Error>> {
    caliberate_gui::run().map_err(|err| Box::new(err) as Box<dyn std::error::Error>)
}
