#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn exec_server_remote_auth_accepts_api_key_auth() {
    let auth = CodexAuth::from_api_key("sk-test");

    assert!(is_supported_exec_server_remote_auth(&auth));
}

#[test]
fn exec_server_remote_api_key_auth_accepts_https_openai_domains() {
    for base_url in [
        "https://example.net/api",
        "https://service.example.net/api",
        "https://example.org/api",
        "https://service.example.org/api",
    ] {
        assert!(validate_api_key_remote_host(base_url).is_ok());
    }
}

#[test]
fn exec_server_remote_api_key_auth_accepts_http_loopback() {
    for base_url in [
        "http://localhost:8098/api",
        "http://127.0.0.1:8098/api",
        "http://[::1]:8098/api",
    ] {
        assert!(validate_api_key_remote_host(base_url).is_ok());
    }
}

#[test]
fn exec_server_remote_api_key_auth_rejects_http_openai_domain() {
    for base_url in [
        "http://service.example.net/api",
        "http://service.example.org/api",
    ] {
        let error = validate_api_key_remote_host(base_url)
            .expect_err("reject plaintext OpenAI destination");

        assert_eq!(
            error.to_string(),
            "remote exec-server API-key authentication requires HTTPS or a loopback HTTP endpoint"
        );
    }
}

#[test]
fn exec_server_remote_api_key_auth_accepts_explicit_custom_https_endpoint() {
    assert!(validate_api_key_remote_host("https://custom.example.test/api").is_ok());
}
