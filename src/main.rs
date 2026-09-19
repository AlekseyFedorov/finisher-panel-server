//! Веб-сервер «Финишер — панель дня».
//! Отдаёт единственную страницу и сохраняет состояние в YAML-файл
//! в каталоге data/ внутри проекта.

use std::path::PathBuf;

use axum::{
    Router,
    http::{StatusCode, header},
    response::{Html, IntoResponse},
    routing::get,
};

const INDEX: &str = include_str!("../static/index.html");

/// Путь к YAML-файлу с данными.
/// По умолчанию — каталог data/ того дерева исходников, где шла сборка
/// (`CARGO_MANIFEST_DIR` подставляется при компиляции, поэтому бинарник
/// не перемещаем). Переменная FINISHER_DATA задаёт путь в рантайме —
/// для деплоя, где каталог сборки не должен быть хранилищем данных.
fn data_file() -> PathBuf {
    match std::env::var("FINISHER_DATA") {
        Ok(path) => PathBuf::from(path),
        Err(_) => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("finisher-data.yaml"),
    }
}

async fn index() -> Html<&'static str> {
    Html(INDEX)
}

/// GET /api/state — содержимое YAML-файла, 204 если данных ещё нет.
async fn get_state() -> impl IntoResponse {
    match tokio::fs::read_to_string(data_file()).await {
        Ok(yaml) => (
            [(header::CONTENT_TYPE, "text/yaml; charset=utf-8")],
            yaml,
        )
            .into_response(),
        Err(_) => StatusCode::NO_CONTENT.into_response(),
    }
}

/// PUT /api/state — валидирует YAML и атомарно пишет его в файл
/// (временный файл + rename, чтобы обрыв записи не портил данные).
async fn put_state(body: String) -> StatusCode {
    if body.trim().is_empty() || serde_yaml::from_str::<serde_yaml::Value>(&body).is_err() {
        return StatusCode::BAD_REQUEST;
    }
    let path = data_file();
    if let Some(dir) = path.parent() {
        let _ = tokio::fs::create_dir_all(dir).await;
    }
    let tmp = path.with_extension("tmp");
    let written = tokio::fs::write(&tmp, &body).await.is_ok()
        && tokio::fs::rename(&tmp, &path).await.is_ok();
    if written {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(index))
        .route("/api/state", get(get_state).put(put_state));

    let addr = std::env::var("FINISHER_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("не удалось занять {addr}: {e}"));
    println!("Финишер — панель дня: http://{addr}");
    axum::serve(listener, app).await.unwrap();
}
