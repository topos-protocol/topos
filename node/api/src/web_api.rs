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
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::convert::Infallible;
use tokio::sync::{mpsc, oneshot};
use topos_core::uci::{Certificate, CertificateId, SubnetId};

/// Handler of [get]/health_check
async fn health_check() -> Result<Response<Body>, Infallible> {
    Ok(to_http_response(Ok::<(), ()>(())))
}

/// Output of [get]/tce_api_endpoints
#[derive(Serialize)]
struct ApiEndpoints {
    pub addrs: Vec<String>,
}

/// Handler of [get]/tce_api_endpoints
async fn tce_api_endpoints(_tx: mpsc::Sender<ApiRequests>) -> Result<Response<Body>, Infallible> {
    // let node_addrs = trbp.known_peers_api_addrs().await.expect("known_peers");
    Ok(to_http_response(ApiEndpoints { addrs: vec![] }))
}

/// Input of [post]/certs
#[derive(Serialize, Deserialize, Debug)]
pub struct SubmitCertReq {
    pub cert: Certificate,
}

/// Handler of [post]/certs
async fn post_cert(
    req: Request<Body>,
    tx: mpsc::Sender<ApiRequests>,
) -> Result<Response<Body>, Infallible> {
    if let Ok(deser_req) = from_post_params::<SubmitCertReq>(req).await {
        // todo: check transport-level signature
        let (snd, rcv) = oneshot::channel::<()>();
        tx.send(ApiRequests::SubmitCert {
            req: deser_req,
            resp_channel: snd,
        })
        .await
        .expect("send");
        rcv.await.expect("sync recv");
        return Ok(to_http_response(Ok::<(), ()>(())));
    }

    Ok(bad_request("failed".into()))
}

/// Input of [post]/delivered_certs
#[derive(Deserialize, Debug)]
pub struct DeliveredCertsReq {
    pub subnet_id: SubnetId,
    pub from_cert_id: CertificateId,
}

/// Output of [post]/delivered_certs
#[derive(Serialize)]
struct DeliveredCerts {
    certs: Vec<Certificate>,
}

/// Handler of POST /delivered_certs
async fn delivered_certs(
    req: Request<Body>,
    tx: mpsc::Sender<ApiRequests>,
) -> Result<Response<Body>, Infallible> {
    if let Ok(deser_req) = from_post_params::<DeliveredCertsReq>(req).await {
        let (snd, rcv) = oneshot::channel::<Vec<Certificate>>();
        tx.send(ApiRequests::DeliveredCerts {
            req: deser_req,
            resp_channel: snd,
        })
        .await
        .expect("send");
        let res = rcv.await;
        return match res {
            Ok(val) => {
                log::debug!("Request succeeded {:?}", val);
                Ok(to_http_response(DeliveredCerts { certs: val }))
            }
            Err(e) => {
                log::error!("Request failure {:?}", e);
                Ok(bad_request("failed".into()))
            }
        };
    }
    Ok(bad_request("failed".into()))
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
    tx: mpsc::Sender<ApiRequests>,
) -> Result<Response<Body>, Infallible> {
    log::debug!("dispatch_req: {:?}", req);
    match (req.method(), req.uri().path()) {
        // matches
        (&Method::GET, "/health_check") => health_check().await,
        (&Method::GET, "/tce_api_endpoints") => tce_api_endpoints(tx.clone()).await,
        (&Method::POST, "/certs") => post_cert(req, tx.clone()).await,
        (&Method::POST, "/delivered_certs") => delivered_certs(req, tx.clone()).await,
        // not found
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("Not Found".into())
            .unwrap()),
    }
}

/// Utility to read post params
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
                log::warn!("from_post_params failed due to: {:?}", &e);
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
fn bad_request(msg: String) -> Response<hyper::Body> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(msg.into())
        .unwrap()
}
