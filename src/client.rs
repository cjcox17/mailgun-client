use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Mailgun API regions
pub enum Region {
    /// United States region
    US,
    /// European Union region
    EU,
}

impl Region {
    /// Returns the base URL for this region
    pub fn base_url(&self) -> &'static str {
        match self {
            Region::US => "https://api.mailgun.net",
            Region::EU => "https://api.eu.mailgun.net",
        }
    }
}

/// A client for interacting with the Mailgun API
///
/// This client handles authentication, regional endpoints, and HTTP configuration
/// for sending emails through Mailgun's REST API.
pub struct MailgunClient {
    api_key: String,
    client: Client,
    pool: usize,
    timeout: usize,
    base_url: &'static str,
}

/// Errors that can occur when using the Mailgun client
#[derive(Error, Debug)]
pub enum MailgunClientError {
    #[error("non 200 status: {0} {1}")]
    MailgunError(String, String),

    #[error("email error: {0}")]
    Email(#[from] EmailError), // wraps your EmailError

    #[error("HTTP request error: {0}")]
    Reqwest(#[from] reqwest::Error), // wraps reqwest::Error
}

impl MailgunClient {
    /// Creates a new Mailgun client
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Mailgun API key
    /// * `region` - The Mailgun region to use (US or EU)
    ///
    /// # Examples
    ///
    /// ```
    /// let client = MailgunClient::new("key-your-api-key", Region::US);
    /// ```
    pub fn new(api_key: impl Into<String>, region: Region) -> Self {
        let client = Client::new();
        Self {
            api_key: api_key.into(),
            client,
            pool: 5,
            timeout: 30,
            base_url: region.base_url(),
        }
    }

    /// Sets the connection pool size for the HTTP client
    ///
    /// # Arguments
    ///
    /// * `pool` - The size of the connection pool
    ///
    /// # Returns
    ///
    /// The modified client instance for method chaining
    pub fn with_pool(mut self, pool: usize) -> Self {
        self.pool = pool;
        self
    }

    /// Sets the request timeout for the HTTP client
    ///
    /// # Arguments
    ///
    /// * `timeout` - The timeout in seconds
    ///
    /// # Returns
    ///
    /// The modified client instance for method chaining
    pub fn with_timeout(mut self, timeout: usize) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sends an email through the Mailgun API
    ///
    /// # Arguments
    ///
    /// * `email` - The email to send, constructed using the `Email` builder
    ///
    /// # Returns
    ///
    /// A `Result` containing either a `SendEmailResponse` on success or a `MailgunClientError` on failure
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The email is missing a body (text or HTML)
    /// - The HTTP request fails
    /// - The Mailgun API returns a non-200 status code
    ///
    /// # Examples
    ///
    /// ```
    /// let email = Email::new("example.com", "from@example.com", vec!["to@example.com"], "Subject")
    ///     .with_text("Hello, world!".to_string());
    /// let response = client.send_email(email).await?;
    /// ```
    pub async fn send_email(&self, email: Email) -> Result<SendEmailResponse, MailgunClientError> {
        let url = format!("{}/v3/{}/messages", self.base_url, email.domain);
        let mut form = HashMap::new();
        form.insert("from", email.from.clone());
        form.insert("to", email.to.join(","));
        form.insert("cc", email.cc.join(","));
        form.insert("bcc", email.bcc.join(","));
        form.insert("subject", email.subject.clone());
        let (content_type, body) = email.get_body()?;
        form.insert(content_type, body);

        let response = self
            .client
            .post(&url)
            .basic_auth("api", Some(&self.api_key))
            .form(&form)
            .send()
            .await?;

        println!("response {response:?}");
        if response.status() != StatusCode::OK {
            return Err(MailgunClientError::MailgunError(
                response.status().to_string(),
                response.json().await?,
            ));
        }

        let body = response.json::<SendEmailResponse>().await?;

        Ok(body)
    }
}

/// Response from the Mailgun API when sending an email
#[derive(Debug, Clone, Deserialize)]
pub struct SendEmailResponse {
    id: String,
    message: String,
}

/// An email message to be sent through Mailgun
///
/// This struct contains all the information needed to send an email,
/// including recipients, content, and optional settings.
#[derive(Debug, Clone)]
pub struct Email {
    domain: String,
    from: String,
    to: Vec<String>,
    cc: Vec<String>,
    bcc: Vec<String>,
    subject: String,
    text: Option<String>,
    html: Option<String>,
    template: Option<Template>,
    send_options: Option<SendOptions>,
    amp_html: Option<String>,
    attachment: Option<HashMap<String, Vec<u8>>>,
    inline: Option<HashMap<String, Vec<u8>>>,
    recipient_variables: Option<String>,
}

/// Errors that can occur when constructing or validating an email
#[derive(Error, Debug)]
pub enum EmailError {
    #[error("invalid email address: {0}")]
    InvalidAddress(String),

    #[error("this email contains no text or html body")]
    MissingBody,
}

impl Email {
    /// Creates a new email message
    ///
    /// # Arguments
    ///
    /// * `domain` - Your Mailgun domain (e.g., "example.com")
    /// * `from` - The sender's email address
    /// * `to` - An iterable of recipient email addresses
    /// * `subject` - The email subject line
    ///
    /// # Returns
    ///
    /// A new `Email` instance with the specified basic information
    ///
    /// # Examples
    ///
    /// ```
    /// let email = Email::new(
    ///     "example.com",
    ///     "sender@example.com",
    ///     vec!["recipient@example.com"],
    ///     "Hello World".to_string()
    /// );
    /// ```
    pub fn new(
        domain: impl Into<String>,
        from: impl Into<String>,
        to: impl IntoIterator<Item = impl Into<String>>,
        subject: String,
    ) -> Self {
        Email {
            domain: domain.into(),
            from: from.into(),
            to: to.into_iter().map(|s| s.into()).collect(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject,
            text: None,
            html: None,
            template: None,
            send_options: None,
            amp_html: None,
            attachment: None,
            inline: None,
            recipient_variables: None,
        }
    }

    /// Sets the plain text body of the email
    ///
    /// # Arguments
    ///
    /// * `text` - The plain text content of the email
    ///
    /// # Returns
    ///
    /// The modified email instance for method chaining
    pub fn with_text(mut self, text: String) -> Self {
        self.text = Some(text);
        self
    }

    /// Sets the HTML body of the email
    ///
    /// # Arguments
    ///
    /// * `html` - The HTML content of the email
    ///
    /// # Returns
    ///
    /// The modified email instance for method chaining
    pub fn with_html(mut self, html: String) -> Self {
        self.html = Some(html);
        self
    }

    /// Sets additional sending options for the email
    ///
    /// # Arguments
    ///
    /// * `send_options` - Configuration options for how the email should be sent
    ///
    /// # Returns
    ///
    /// The modified email instance for method chaining
    pub fn with_send_options(mut self, send_options: SendOptions) -> Self {
        self.send_options = Some(send_options);
        self
    }

    /// Extracts the email body content and determines its type
    ///
    /// This method prioritizes HTML content over plain text. If both are present,
    /// it returns the HTML content. If neither is present or both are empty,
    /// it returns an error.
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// - The content type ("html" or "text")
    /// - The actual content string
    ///
    /// # Errors
    ///
    /// Returns `EmailError::MissingBody` if no valid body content is found
    pub fn get_body(&self) -> Result<(&str, String), EmailError> {
        if let Some(html) = &self.html
            && !html.is_empty()
        {
            return Ok(("html", html.clone()));
        }

        if let Some(text) = &self.text
            && !text.is_empty()
        {
            return Ok(("text", text.clone()));
        }

        Err(EmailError::MissingBody)
    }
}

/// Template configuration for Mailgun email templates
#[derive(Debug, Clone)]
pub struct Template {
    template: Option<String>,
    template_version: Option<String>,
    template_text: Option<String>,
    template_variables: Option<String>,
}

/// Additional options for sending emails through Mailgun
///
/// These options control various aspects of email delivery, tracking,
/// and processing. All fields are optional and use Mailgun's default
/// behavior when not specified.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SendOptions {
    #[serde(rename = "o:tags", skip_serializing_if = "Option::is_none")]
    o_tags: Option<Vec<String>>,

    #[serde(rename = "o:dkim", skip_serializing_if = "Option::is_none")]
    o_dkim: Option<bool>,

    #[serde(rename = "o:dkim:secondary", skip_serializing_if = "Option::is_none")]
    o_dkim_secondary: Option<String>,

    #[serde(
        rename = "o:dkim:secondary:public",
        skip_serializing_if = "Option::is_none"
    )]
    o_dkim_secondary_public: Option<String>,

    #[serde(rename = "o:deliverytime", skip_serializing_if = "Option::is_none")]
    o_delivery_time: Option<DateTime<Utc>>,

    #[serde(rename = "o:timezonelocalize", skip_serializing_if = "Option::is_none")]
    o_time_zone_localize: Option<String>,

    #[serde(rename = "o:testmode", skip_serializing_if = "Option::is_none")]
    o_test_mode: Option<bool>,

    #[serde(rename = "o:tracking", skip_serializing_if = "Option::is_none")]
    o_tracking: Option<bool>,

    #[serde(rename = "o:tracking-clicks", skip_serializing_if = "Option::is_none")]
    o_tracking_clicks: Option<bool>,

    #[serde(rename = "o:tracking-opens", skip_serializing_if = "Option::is_none")]
    o_tracking_opens: Option<bool>,

    #[serde(rename = "o:require-tls", skip_serializing_if = "Option::is_none")]
    o_require_tls: Option<bool>,

    #[serde(rename = "o:skip-verification")]
    o_skip_verification: bool,

    #[serde(rename = "o:sending-ip", skip_serializing_if = "Option::is_none")]
    o_sending_ip: Option<String>,

    #[serde(rename = "o:sending-ip-pool", skip_serializing_if = "Option::is_none")]
    o_sending_ip_pool: Option<String>,

    #[serde(rename = "o:tracking-pixel-location-top")]
    o_tracking_pixel_location_top: bool,

    #[serde(rename = "o:archive-to", skip_serializing_if = "Option::is_none")]
    o_archive_to: Option<String>,

    #[serde(rename = "o:suppress-header", skip_serializing_if = "Option::is_none")]
    o_suppress_header: Option<String>,

    #[serde(rename = "h:X-My-Header", skip_serializing_if = "Option::is_none")]
    h_x_my_header: Option<String>,

    #[serde(rename = "v:my-var", skip_serializing_if = "Option::is_none")]
    v_my_var: Option<String>,
}

impl Default for SendOptions {
    /// Creates a default `SendOptions` configuration
    ///
    /// The default configuration sets:
    /// - `o_skip_verification` to `true` (skip email address verification)
    /// - `o_tracking_pixel_location_top` to `false` (place tracking pixel at bottom)
    /// - All other options to `None` (use Mailgun defaults)
    fn default() -> Self {
        SendOptions {
            o_tags: None,
            o_dkim: None,
            o_dkim_secondary: None,
            o_dkim_secondary_public: None,
            o_delivery_time: None,
            o_time_zone_localize: None,
            o_test_mode: None,
            o_tracking: None,
            o_tracking_clicks: None,
            o_tracking_opens: None,
            o_require_tls: None,
            o_skip_verification: true,
            o_sending_ip: None,
            o_sending_ip_pool: None,
            o_tracking_pixel_location_top: false,
            o_archive_to: None,
            o_suppress_header: None,
            h_x_my_header: None,
            v_my_var: None,
        }
    }
}
