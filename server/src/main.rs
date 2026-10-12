// embedded ui + api, routed by suffix so any mount prefix works
use std::io::{Cursor, Read};
use std::{env, sync::Arc, sync::OnceLock, thread};

use tiny_http::{Header, Method, Request, Response, Server};

const MAX_BODY: usize = 1 << 20;
const FILES: [(&str, &str, &str); 5] = [
    ("app.js", "text/javascript", include_str!("../../ui/app.js")),
    ("editor.js", "text/javascript", include_str!("../../ui/editor.js")),
    ("graph.js", "text/javascript", include_str!("../../ui/graph.js")),
    ("app.css", "text/css", include_str!("../../ui/app.css")),
    ("index.html", "text/html", include_str!("../../ui/index.html")),
];

// asset urls stamped with the version: a redeploy changes every url,
// so no browser or CDN cache can serve the old ui under the new one
fn files() -> &'static [(&'static str, &'static str, String)] {
    static STAMPED: OnceLock<Vec<(&'static str, &'static str, String)>> = OnceLock::new();
    STAMPED.get_or_init(|| {
        let v = circuit::VERSION;
        let stamp = |body: &str| {
            FILES.iter().fold(body.to_string(), |b, (dep, ..)| {
                b.replace(&format!("\"{dep}\""), &format!("\"{dep}?v={v}\""))
                    .replace(&format!("'./{dep}'"), &format!("'./{dep}?v={v}'"))
            })
        };
        FILES.iter().map(|&(name, kind, body)| (name, kind, stamp(body))).collect()
    })
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct Job {
    parts: Vec<circuit::PartIn>,
    width: f32,
    height: f32,
}

type Res = Response<Cursor<Vec<u8>>>;

// no-cache: the proxy/CDN and browser revalidate, so a redeploy shows at once
fn reply(body: impl Into<Vec<u8>>, kind: &str, code: u16) -> Res {
    let kind = format!("{kind}; charset=utf-8");
    Response::from_data(body.into())
        .with_status_code(code)
        .with_header(Header::from_bytes("Content-Type", kind).unwrap())
        .with_header(Header::from_bytes("Cache-Control", "no-cache").unwrap())
}

fn json(body: impl serde::Serialize, code: u16) -> Res {
    reply(serde_json::to_vec(&body).unwrap(), "application/json", code)
}

fn body(req: &mut Request) -> Result<Vec<u8>, (u16, String)> {
    let mut body = Vec::new();
    match req.as_reader().take(MAX_BODY as u64 + 1).read_to_end(&mut body) {
        Ok(n) if n <= MAX_BODY => Ok(body),
        _ => Err((413, "body too large".into())),
    }
}

fn solve(req: &mut Request) -> Res {
    let out = body(req).and_then(|b| {
        serde_json::from_slice::<Job>(&b)
            .map_err(|e| (400, e.to_string()))
            .and_then(|j| circuit::run(&j.parts, j.width, j.height).map_err(|e| (422, e)))
    });
    match out {
        Ok(scene) => json(scene, 200),
        Err((code, e)) => json(serde_json::json!({ "error": e }), code),
    }
}

fn route(req: &mut Request) -> Res {
    let path = req.url().split('?').next().unwrap_or_default().to_string();
    match req.method() {
        Method::Post if path.ends_with("/api/solve") => solve(req),
        Method::Get | Method::Head if path.ends_with("/api/version") => {
            json(serde_json::json!({ "version": circuit::VERSION }), 200)
        }
        Method::Get | Method::Head => {
            let name = path.rsplit('/').next().unwrap_or_default();
            let files = files();
            let (_, kind, body) = files.iter().find(|f| f.0 == name).unwrap_or(&files[4]);
            reply(body.as_str(), kind, 200)
        }
        _ => reply("", "text/plain", 405),
    }
}

fn main() {
    let port: u16 = env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let server = Arc::new(Server::http(("0.0.0.0", port)).unwrap_or_else(|e| panic!("bind {port}: {e}")));
    let workers = thread::available_parallelism().map_or(8, |n| n.get() * 4);
    eprintln!("circuit v{} on http://0.0.0.0:{port}, {workers} workers", circuit::VERSION);
    let pool: Vec<_> = (0..workers)
        .map(|_| {
            let server = Arc::clone(&server);
            thread::spawn(move || {
                for mut req in server.incoming_requests() {
                    let res = route(&mut req);
                    let _ = req.respond(res);
                }
            })
        })
        .collect();
    pool.into_iter().for_each(|t| t.join().unwrap_or_default());
}
