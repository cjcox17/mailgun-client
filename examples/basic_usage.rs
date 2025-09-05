use dotenvy::dotenv;
use mailgun_client::MailgunClient;
use mailgun_client::client::Email;
use mailgun_client::client::Region;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let api_key = std::env::var("MAILGUN_API_KEY").expect("MAILGUN_API_KEY must be set");
    let domain = std::env::var("DOMAIN").expect("DOMAIN must be set");
    let email = std::env::var("EMAIL").expect("EMAIL must be set");

    println!("API_KEY: {api_key}");
    println!("DOMAIN: {domain}");
    println!("EMAIL: {email}");

    let client = MailgunClient::new(api_key, Region::US);

    let email = Email::new(
        &domain,
        format!("test@{}", &domain),
        vec![email],
        "This is a test email".to_string(),
    )
    .with_text("This is a test email".to_string());

    let response = client.send_email(email).await?;

    println!("Email sent! {response:?}");

    Ok(())
}
