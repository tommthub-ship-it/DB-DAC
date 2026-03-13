/// 간지-DAC Admin API HTTP 클라이언트

use anyhow::{Context, Result, bail};
use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub struct DacClient {
    http: Client,
    base_url: String,
    api_key: String,
}

impl DacClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Result<Self> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .context("HTTP 클라이언트 초기화 실패")?;

        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn auth(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    // ── GET ─────────────────────────────────────────────────

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .http
            .get(self.url(path))
            .header("Authorization", self.auth())
            .send()
            .await
            .with_context(|| format!("GET {} 요청 실패", path))?;

        self.parse(resp).await
    }

    pub async fn get_raw(&self, path: &str) -> Result<Value> {
        self.get(path).await
    }

    // ── POST ────────────────────────────────────────────────

    pub async fn post<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T> {
        let resp = self
            .http
            .post(self.url(path))
            .header("Authorization", self.auth())
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {} 요청 실패", path))?;

        self.parse(resp).await
    }

    pub async fn post_empty<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .http
            .post(self.url(path))
            .header("Authorization", self.auth())
            .header("Content-Length", "0")
            .send()
            .await
            .with_context(|| format!("POST {} 요청 실패", path))?;

        self.parse(resp).await
    }

    // ── PUT ─────────────────────────────────────────────────

    pub async fn put<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T> {
        let resp = self
            .http
            .put(self.url(path))
            .header("Authorization", self.auth())
            .json(body)
            .send()
            .await
            .with_context(|| format!("PUT {} 요청 실패", path))?;

        self.parse(resp).await
    }

    // ── DELETE ──────────────────────────────────────────────

    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .http
            .delete(self.url(path))
            .header("Authorization", self.auth())
            .send()
            .await
            .with_context(|| format!("DELETE {} 요청 실패", path))?;

        self.parse(resp).await
    }

    // ── 헬스체크 (인증 없음) ─────────────────────────────────

    pub async fn health(&self) -> Result<Value> {
        let resp = self
            .http
            .get(self.url("/health"))
            .send()
            .await
            .context("서버에 연결할 수 없습니다")?;

        self.parse(resp).await
    }

    // ── 응답 파싱 ────────────────────────────────────────────

    async fn parse<T: DeserializeOwned>(&self, resp: Response) -> Result<T> {
        let status = resp.status();
        let body = resp.text().await.context("응답 본문 읽기 실패")?;

        if status == StatusCode::UNAUTHORIZED {
            bail!("인증 실패 — API 키를 확인하세요 (--key 또는 DAC_API_KEY)");
        }

        if !status.is_success() {
            // 에러 메시지 파싱 시도
            let msg = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(|s| s.to_string()))
                .unwrap_or_else(|| body.clone());
            bail!("HTTP {} — {}", status, msg);
        }

        serde_json::from_str(&body)
            .with_context(|| format!("응답 파싱 실패: {}", body))
    }
}
