#[tracing::instrument(name = "Greeting", skip_all)]
pub async fn greet() -> String {
    "Hello, World!".into()
}
