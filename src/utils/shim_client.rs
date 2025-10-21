use crate::config;
use anyhow::Result;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);

/// Performs an HTTP GET request to the shim monitor socket
pub async fn do_get(sandbox_id: &str, path: &str) -> Result<Vec<u8>> {
    do_get_with_timeout(sandbox_id, DEFAULT_TIMEOUT, path).await
}

/// Performs an HTTP GET request with custom timeout
pub async fn do_get_with_timeout(
    sandbox_id: &str,
    timeout: Duration,
    path: &str,
) -> Result<Vec<u8>> {
    let socket_address = config::client_socket_address(sandbox_id)?;

    // Parse the socket address to extract the path
    let socket_path = if socket_address.starts_with("unix://") {
        &socket_address[7..]
    } else {
        &socket_address
    };

    // Create a URI for the HTTP request
    let uri = format!("http://shim{}", path);

    // Use Unix socket connector
    let response = do_http_get_unix_socket(socket_path, &uri, timeout).await?;

    Ok(response)
}

/// Perform HTTP GET over Unix socket
async fn do_http_get_unix_socket(
    socket_path: &str,
    uri: &str,
    timeout: Duration,
) -> Result<Vec<u8>> {
    use tokio::net::UnixStream;

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: shim\r\nConnection: close\r\n\r\n",
        uri
    );

    // Connect to Unix socket with timeout
    let mut stream = tokio::time::timeout(timeout, UnixStream::connect(socket_path)).await??;

    // Send request
    stream.write_all(request.as_bytes()).await?;

    // Read response
    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer).await?;

    // Parse HTTP response to extract body
    let response_str = String::from_utf8_lossy(&buffer);

    if !response_str.contains("200") {
        let status_line = response_str.lines().next().unwrap_or("Unknown");
        return Err(anyhow::anyhow!(
            "unexpected status from {}: {}",
            uri,
            status_line
        ));
    }

    // Find the body (after empty line)
    if let Some(body_start) = response_str.find("\r\n\r\n") {
        Ok(response_str[body_start + 4..].as_bytes().to_vec())
    } else {
        Ok(vec![])
    }
}
