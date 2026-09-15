use crate::{deep_link::parser::validate_game_id, games::catalog, logging, protocol::mime};
use http::{header, Request, Response, StatusCode};
use percent_encoding::percent_decode_str;
use tauri::{AppHandle, Runtime};

pub fn response_for_request<R: Runtime>(
    app: &AppHandle<R>,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    response_with_reader(request, |game_id, resource_path| {
        catalog::read_installed_resource(app, game_id, resource_path)
    })
}

fn response_with_reader<F>(request: Request<Vec<u8>>, read_installed: F) -> Response<Vec<u8>>
where
    F: FnOnce(&str, &str) -> Result<Option<Vec<u8>>, String>,
{
    match build_response(request, read_installed) {
        Ok(response) => response,
        Err((status, message)) => Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(message.into_bytes())
            .unwrap_or_else(|_| Response::new(Vec::new())),
    }
}

fn build_response<F>(
    request: Request<Vec<u8>>,
    read_installed: F,
) -> Result<Response<Vec<u8>>, (StatusCode, String)>
where
    F: FnOnce(&str, &str) -> Result<Option<Vec<u8>>, String>,
{
    let uri = request.uri();

    if uri.host() != Some("game") {
        return Err((StatusCode::NOT_FOUND, "Unknown ytasset host.".to_string()));
    }

    let decoded_path = percent_decode_str(uri.path().trim_start_matches('/'))
        .decode_utf8()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid asset path encoding.".to_string(),
            )
        })?;

    let mut parts = decoded_path.splitn(2, '/');
    let game_id = parts
        .next()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing game ID.".to_string()))?;
    let resource_path = parts.next().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "Missing game resource path.".to_string(),
        )
    })?;

    validate_game_id(game_id).map_err(|message| (StatusCode::BAD_REQUEST, message))?;
    validate_resource_path(resource_path)?;

    let bytes = read_installed(game_id, resource_path)
        .map_err(|message| (StatusCode::INTERNAL_SERVER_ERROR, message))?
        .ok_or_else(|| {
            logging::event(
                "game_resource_not_found",
                &[("game_id", game_id), ("path", resource_path)],
            );
            (
                StatusCode::NOT_FOUND,
                "Game resource was not found.".to_string(),
            )
        })?;
    let bytes = maybe_inject_game_runtime_shims(resource_path, bytes);

    let mut response_builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime::content_type(resource_path));

    if is_html_resource(resource_path) {
        response_builder = response_builder.header(
            "Content-Security-Policy",
            "default-src 'self' data: blob: ytasset:; connect-src 'self'; img-src 'self' data: blob: ytasset:; media-src 'self' data: blob: ytasset:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; object-src 'none'",
        );
    }

    response_builder.body(bytes).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to build response: {err}"),
        )
    })
}

fn is_html_resource(resource_path: &str) -> bool {
    resource_path.ends_with(".html") || resource_path.ends_with(".htm")
}

fn maybe_inject_game_runtime_shims(resource_path: &str, bytes: Vec<u8>) -> Vec<u8> {
    if !is_html_resource(resource_path) {
        return bytes;
    }

    let html = match String::from_utf8(bytes) {
        Ok(html) => html,
        Err(err) => return err.into_bytes(),
    };

    let shim = r#"<script>
(function () {
  function send(level, payload) {
    try {
      window.parent.postMessage(Object.assign({
        source: "yeutre-game-runtime",
        level: level,
        href: window.location.href,
        readyState: document.readyState
      }, payload || {}), "*");
    } catch (_) {}
  }

  window.addEventListener("error", function (event) {
    send("error", {
      message: event.message || "Script error",
      filename: event.filename,
      line: event.lineno,
      column: event.colno
    });
  });

  window.addEventListener("unhandledrejection", function (event) {
    var reason = event.reason;
    send("unhandledrejection", {
      message: reason && reason.message ? reason.message : String(reason || "Unhandled promise rejection")
    });
  });

  window.addEventListener("securitypolicyviolation", function (event) {
    send("securitypolicyviolation", {
      message: "CSP blocked " + event.violatedDirective,
      blockedURI: event.blockedURI,
      violatedDirective: event.violatedDirective
    });
  });

  window.addEventListener("DOMContentLoaded", function () {
    var activeCard = null;
    var ghost = null;
    var didMove = false;

    function isWordCard(node) {
      return node && node.closest ? node.closest(".word-card") : null;
    }

    function makeGhost(card, event) {
      ghost = card.cloneNode(true);
      ghost.style.position = "fixed";
      ghost.style.left = "0";
      ghost.style.top = "0";
      ghost.style.zIndex = "2147483647";
      ghost.style.pointerEvents = "none";
      ghost.style.opacity = "0.92";
      ghost.style.transform = "translate(" + event.clientX + "px, " + event.clientY + "px) translate(-50%, -50%)";
      ghost.style.boxShadow = "0 16px 36px rgba(0,0,0,.22)";
      document.body.appendChild(ghost);
    }

    function moveGhost(event) {
      if (ghost) {
        ghost.style.transform = "translate(" + event.clientX + "px, " + event.clientY + "px) translate(-50%, -50%)";
      }
    }

    function clearGhost() {
      if (ghost) ghost.remove();
      ghost = null;
    }

    document.addEventListener("pointerdown", function (event) {
      var card = isWordCard(event.target);
      if (!card || event.button > 0) return;
      activeCard = card;
      didMove = false;
      try { card.setPointerCapture && card.setPointerCapture(event.pointerId); } catch (_) {}
      makeGhost(card, event);
    }, true);

    document.addEventListener("pointermove", function (event) {
      if (!activeCard) return;
      didMove = true;
      moveGhost(event);
      event.preventDefault();
    }, true);

    document.addEventListener("pointerup", function (event) {
      if (!activeCard) return;
      var card = activeCard;
      activeCard = null;
      clearGhost();
      var target = document.elementFromPoint(event.clientX, event.clientY);
      var zone = target && target.closest ? target.closest(".dropzone") : null;
      if (!zone) return;
      event.preventDefault();
      try { card.click(); } catch (_) {}
      try { zone.click(); } catch (_) {}
      send("drag-drop", {
        message: "Pointer fallback placed card",
        word: card.dataset ? card.dataset.wordText : "",
        target: zone.dataset ? zone.dataset.target : "",
        moved: didMove
      });
    }, true);

    document.addEventListener("pointercancel", function () {
      activeCard = null;
      clearGhost();
    }, true);
  });

  var nativeStorage = null;
  try {
    nativeStorage = window.localStorage;
  } catch (_) {}

  var memory = Object.create(null);
  var memoryKeys = [];

  function rememberKey(key) {
    if (memoryKeys.indexOf(key) === -1) memoryKeys.push(key);
  }

  var storage = {
    get length() {
      var nativeLength = 0;
      try { nativeLength = nativeStorage ? nativeStorage.length : 0; } catch (_) {}
      return Math.max(nativeLength, memoryKeys.length);
    },
    key: function (index) {
      if (index < memoryKeys.length) return memoryKeys[index] || null;
      try { return nativeStorage ? nativeStorage.key(index - memoryKeys.length) : null; } catch (_) { return null; }
    },
    getItem: function (key) {
      key = String(key);
      if (Object.prototype.hasOwnProperty.call(memory, key)) return memory[key];
      try { return nativeStorage ? nativeStorage.getItem(key) : null; } catch (_) { return null; }
    },
    setItem: function (key, value) {
      key = String(key);
      value = String(value);
      memory[key] = value;
      rememberKey(key);
      try {
        if (nativeStorage) nativeStorage.setItem(key, value);
      } catch (error) {
        send("storage-fallback", {
          message: error && error.name ? error.name + ": " + error.message : String(error || "localStorage write failed"),
          key: key,
          bytes: value.length
        });
      }
    },
    removeItem: function (key) {
      key = String(key);
      delete memory[key];
      memoryKeys = memoryKeys.filter(function (item) { return item !== key; });
      try { if (nativeStorage) nativeStorage.removeItem(key); } catch (_) {}
    },
    clear: function () {
      memory = Object.create(null);
      memoryKeys = [];
      try { if (nativeStorage) nativeStorage.clear(); } catch (_) {}
    }
  };

  try {
    Object.defineProperty(window, "localStorage", {
      configurable: true,
      value: storage
    });
  } catch (error) {
    if (nativeStorage) {
      try {
        var originalSetItem = nativeStorage.setItem.bind(nativeStorage);
        nativeStorage.setItem = function (key, value) {
          key = String(key);
          value = String(value);
          memory[key] = value;
          rememberKey(key);
          try { originalSetItem(key, value); } catch (writeError) {
            send("storage-fallback", {
              message: writeError && writeError.name ? writeError.name + ": " + writeError.message : String(writeError || "localStorage write failed"),
              key: key,
              bytes: value.length
            });
          }
        };
      } catch (_) {}
    }
    window.__ytLocalStorageFallback = storage;
    send("storage-fallback", {
      message: error && error.message ? error.message : "Could not replace localStorage object"
    });
  }
})();
</script>"#;

    if let Some(index) = html.find("</head>") {
        let mut output = String::with_capacity(html.len() + shim.len());
        output.push_str(&html[..index]);
        output.push_str(shim);
        output.push_str(&html[index..]);
        return output.into_bytes();
    }

    if let Some(index) = html.find("<script") {
        let mut output = String::with_capacity(html.len() + shim.len());
        output.push_str(&html[..index]);
        output.push_str(shim);
        output.push_str(&html[index..]);
        return output.into_bytes();
    }

    html.into_bytes()
}

fn validate_resource_path(resource_path: &str) -> Result<(), (StatusCode, String)> {
    if resource_path.is_empty()
        || resource_path.starts_with('/')
        || resource_path.contains('\\')
        || resource_path
            .split('/')
            .any(|part| part == "." || part == ".." || part.is_empty())
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid game resource path.".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::response_with_reader;
    use http::{Request, StatusCode};

    #[test]
    fn serves_installed_game_resource() {
        let request = Request::builder()
            .uri("ytasset://game/toan-lop-4/index.html")
            .body(Vec::new())
            .unwrap();
        let response = response_with_reader(request, |_, _| Ok(Some(b"<html></html>".to_vec())));
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!response.body().is_empty());
    }

    #[test]
    fn injects_local_storage_fallback_into_html() {
        let request = Request::builder()
            .uri("ytasset://game/toan-lop-4/index.html")
            .body(Vec::new())
            .unwrap();
        let response = response_with_reader(request, |_, _| {
            Ok(Some(b"<html><head></head><body><script>localStorage.getItem('x')</script></body></html>".to_vec()))
        });
        let body = String::from_utf8(response.body().clone()).unwrap();
        assert!(body.contains("storage-fallback"));
        assert!(body.contains("</head>"));
    }

    #[test]
    fn rejects_path_traversal() {
        let request = Request::builder()
            .uri("ytasset://game/toan-lop-4/../secret.html")
            .body(Vec::new())
            .unwrap();
        let response = response_with_reader(request, |_, _| Ok(None));
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
