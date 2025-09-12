use std::{env};

use anyhow::anyhow;
use async_imap::Client;
use async_native_tls::TlsConnector;

use futures::StreamExt;
use tokio::net::TcpStream;

async fn read_starttls_emails(
    imap_addr: (&str, u16),
    login: &str,
    password: &str,
) -> anyhow::Result<()> {
    // Step 1: Connect to the IMAP server over plain TCP
    let tcp_stream = TcpStream::connect(imap_addr).await?;
    println!("Connected to {}:{} via plain TCP", imap_addr.0, imap_addr.1);

    let mut client = Client::new(tcp_stream);

    // Step 2: Send the STARTTLS command
    client.run_command_and_check_ok("STARTTLS", None).await?;
    println!("STARTTLS command sent successfully");

    use native_tls::TlsConnector as NativeTlsConnector;
    let mut native_tls_connector = NativeTlsConnector::builder();

    native_tls_connector
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true);

    let tls = TlsConnector::from(native_tls_connector);

    // Step 3: Upgrade to a secure TLS connection
    // let tls = TlsConnector::new();
    let tls_stream = tls.connect(imap_addr.0, client.into_inner()).await?;
    println!("Upgraded to a secure TLS connection");

    // Step 4: Rebuild the IMAP client using the TLS stream
    let client = Client::new(tls_stream);

    // Step 5: Log in with the provided credentials
    let mut imap_session = match client.login(login, password).await {
        Ok(session) => session,
        Err(e) => {
            println!("Error: {:?}", e.0);
            return Err(anyhow!("Login failed"));
        }
    };
    println!("Logged in as {}", login);

    // Step 6: Select the INBOX mailbox
    imap_session.select("INBOX").await?;
    println!("INBOX selected");

    // Step 7: Fetch messages from the mailbox
    let mut messages_stream = imap_session.fetch("1", "RFC822").await?;
    let mut messages = Vec::new();

    while let Some(result) = messages_stream.next().await {
        match result {
            Ok(message) => messages.push(message),
            Err(err) => return Err(anyhow::Error::new(err)), // Handle errors appropriately
        }
    }

    let message = if let Some(m) = messages.first() {
        m
    } else {
        return Ok(());
    };

    // Step 8: Extract the message body
    let body = message.body().expect("Message did not have a body!");
    let body = std::str::from_utf8(body)
        .expect("Message was not valid UTF-8")
        .to_string();
    println!("Message body: {}", body);

    // Step 9: Logout from the IMAP session
    //imap_session.logout().await?;
    // println!("Logged out");

    Ok(())
}

async fn read_ssl_emails(
    imap_addr: (&str, u16),
    login: &str,
    password: &str,
) -> anyhow::Result<()> {
    let tcp_stream = TcpStream::connect(imap_addr).await?;
    let tls = async_native_tls::TlsConnector::new();
    let tls_stream = tls.connect(imap_addr.0, tcp_stream).await?;

    println!("Build the client...");

    let mut client = async_imap::Client::new(tls_stream);

    // client.run_command_and_check_ok("STARTTLS", None).await?;

    println!("-- connected to {}:{}", imap_addr.0, imap_addr.1);

    // the client we have here is unauthenticated.
    // to do anything useful with the e-mails, we need to log in
    let mut imap_session = match client.login(login, password).await {
        Ok(session) => session,
        Err(e) => {
            println!("Error : {:?}", e.0);
            return Err(anyhow!("Error"));
        }
    };
    println!("-- logged in a {}", login);

    // we want to fetch the first email in the INBOX mailbox
    imap_session.select("INBOX").await?;
    println!("-- INBOX selected");

    // fetch message number 1 in this mailbox, along with its RFC822 field.
    // RFC 822 dictates the format of the body of e-mails
    let mut messages_stream = imap_session.fetch("1", "RFC822").await?;
    let mut messages = Vec::new();
    // let messages: Vec<_> = messages_stream.try_collect().await?;

    while let Some(result) = messages_stream.next().await {
        match result {
            Ok(message) => messages.push(message),
            Err(err) => return Err(anyhow::Error::new(err)), // Handle errors appropriately
        }
    }

    // dbg!(&messages);
    let message = if let Some(m) = messages.first() {
        m
    } else {
        return Ok(());
    };

    // extract the message's body
    let body = message.body().expect("message did not have a body!");
    let body = std::str::from_utf8(body)
        .expect("message was not valid utf-8")
        .to_string();
    println!("-- 1 message received, logging out");

    // imap_session.logout().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // here take login and password from args 0 and 1
    let args: Vec<String> = env::args().collect();
    let login = &args[1];
    let password = &args[2];

    println!("Hello Imap !!!");

    // Connect to the IMAP server

    //    let imap_addr = ("imap.free.fr", 993);
    //    let _ = read_ssl_emails(imap_addr, login, password).await;

    let proton_addr = ("127.0.0.1", 1143);
    let r = read_starttls_emails(proton_addr, login, password).await;

    dbg!(r);

    Ok(())
}
