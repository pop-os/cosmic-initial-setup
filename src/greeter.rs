use zbus::proxy;

#[proxy(
    interface = "com.system76.CosmicGreeter",
    default_service = "com.system76.CosmicGreeter",
    default_path = "/com/system76/CosmicGreeter"
)]
pub trait Greeter {
    async fn initial_setup_end(&mut self, new_user: String) -> Result<(), zbus::Error>;
}
