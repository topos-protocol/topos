//! Web API
//!
//! Endpoints:
//! - [post] certs,
//! - [post] delivered_certs,
//! - [get] other_nodes
//! - [get] health_check
//!
use crate::ApiRequests;
use bytes::Buf;
use hyper::service::{make_service_fn, service_fn};
use hyper::{header, Body, Method, Request, Response, Server, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use std::convert::Infallible;
use tokio::sync::mpsc;
use tracing::{debug, warn};

/// Handler of [get]/health_check
async fn health_check() -> Result<Response<Body>, Infallible> {
    Ok(to_http_response(Ok::<(), ()>(())))
}

/// Output of [get]/tce_api_endpoints
#[derive(Serialize)]
struct ApiEndpoints {
    pub addrs: Vec<String>,
}

/// Runs hyper web server
pub async fn run(tx: mpsc::Sender<ApiRequests>, web_port: u16) {
    // todo:
    //  - TLS implementation, so the clients know they talk to trusted party
    //  - static routes for monitoring web app
    //  - advertise web endpoints with Kademlia

    let make_svc = make_service_fn(move |_| {
        let tx_mvd = tx.clone();
        async move { Ok::<_, Infallible>(service_fn(move |req| dispatch_req(req, tx_mvd.clone()))) }
    });
    let addr = ([0, 0, 0, 0], web_port).into();
    let svc = Server::bind(&addr).serve(make_svc);
    let _ = svc.await;
}

/// Redirect calls to handlers
async fn dispatch_req(
    req: Request<Body>,
    _tx: mpsc::Sender<ApiRequests>,
) -> Result<Response<Body>, Infallible> {
    debug!("dispatch_req: {:?}", req);
    match (req.method(), req.uri().path()) {
        // matches
        (&Method::GET, "/health_check") => health_check().await,
        // not found
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("Not Found".into())
            .unwrap()),
    }
}

/// Utility to read post params
#[allow(unused)]
async fn from_post_params<T>(req: Request<Body>) -> Result<T, ()>
where
    T: DeserializeOwned,
{
    // todo: check content-length
    if let Ok(full_body) = hyper::body::aggregate(req).await {
        match serde_json::from_reader::<_, T>(full_body.reader()) {
            Ok(deser_req) => {
                return Ok(deser_req);
            }
            Err(e) => {
                warn!("from_post_params failed due to: {:?}", &e);
            }
        }
    }

    Err(())
}

/// Utility to build up Response from output struct
fn to_http_response<T>(output_struct: T) -> Response<hyper::Body>
where
    T: Serialize,
{
    let json = serde_json::to_string(&output_struct).expect("serialize");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json))
        .unwrap()
}

/// Typified BAD_REQUEST
#[allow(unused)]
fn bad_request(msg: String) -> Response<hyper::Body> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(msg.into())
        .unwrap()
}
