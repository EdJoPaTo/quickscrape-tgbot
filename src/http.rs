use ureq::config::Config;
use ureq::http::header::USER_AGENT;
use ureq::http::{HeaderValue, Response};
use ureq::{Agent, Body};

/// Firefox ESR
const USER_AGENT_VALUE: HeaderValue = HeaderValue::from_static(
    "Mozilla/5.0 (X11; Fedora; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0",
);

pub fn get(url: &str) -> Result<Response<Body>, ureq::Error> {
    let config = Config::builder()
        .http_status_as_error(false)
        .save_redirect_history(true)
        .build();
    let agent = Agent::new_with_config(config);
    agent.get(url).header(USER_AGENT, USER_AGENT_VALUE).call()
}
