//! Кадрирование: 4 байта длины (big-endian) + JSON.
//!
//! Чистые функции без ввода-вывода, чтобы ими одинаково пользовались помощник
//! (tokio) и UI. Размер кадра ограничен: всё, что приходит от UI, недоверенное.

use serde::{Serialize, de::DeserializeOwned};

/// Предел размера одного кадра.
pub const MAX_FRAME_LEN: usize = 1024 * 1024;
/// Размер заголовка с длиной.
pub const HEADER_LEN: usize = 4;

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("кадр слишком большой: {0} байт")]
    TooLarge(usize),
    #[error("ошибка JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Сериализует сообщение в кадр (заголовок + тело).
pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, FrameError> {
    let body = serde_json::to_vec(message)?;
    if body.len() > MAX_FRAME_LEN {
        return Err(FrameError::TooLarge(body.len()));
    }
    let len = u32::try_from(body.len()).map_err(|_| FrameError::TooLarge(body.len()))?;
    let mut out = Vec::with_capacity(HEADER_LEN + body.len());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Разбирает заголовок и возвращает длину тела, проверив предел.
pub fn body_len(header: [u8; HEADER_LEN]) -> Result<usize, FrameError> {
    let len = u32::from_be_bytes(header) as usize;
    if len > MAX_FRAME_LEN {
        return Err(FrameError::TooLarge(len));
    }
    Ok(len)
}

/// Разбирает тело кадра.
pub fn decode<T: DeserializeOwned>(body: &[u8]) -> Result<T, FrameError> {
    Ok(serde_json::from_slice(body)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClientMessage, Request};

    #[test]
    fn round_trip() {
        let m = ClientMessage {
            id: 1,
            request: Request::GetStatus,
        };
        let frame = encode(&m).unwrap_or_default();
        let mut header = [0u8; HEADER_LEN];
        header.copy_from_slice(&frame[..HEADER_LEN]);
        let len = body_len(header).unwrap_or(0);
        assert_eq!(len, frame.len() - HEADER_LEN);
        let back: Result<ClientMessage, _> = decode(&frame[HEADER_LEN..]);
        assert_eq!(back.ok(), Some(m));
    }

    #[test]
    fn oversized_header_is_rejected() {
        assert!(matches!(
            body_len(u32::MAX.to_be_bytes()),
            Err(FrameError::TooLarge(_))
        ));
    }
}
