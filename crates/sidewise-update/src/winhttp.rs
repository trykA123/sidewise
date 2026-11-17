//! Windows transport of the update check: one synchronous HTTPS GET through WinHTTP, system proxy and OS TLS.
//! Unsafe code is allowed function by function here and nowhere else in the crate.

use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{null, null_mut};
use std::time::Instant;

use windows_sys::Win32::Foundation::{ERROR_INVALID_PARAMETER, ERROR_TIMEOUT, GetLastError};
use windows_sys::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    WINHTTP_AUTOLOGON_SECURITY_LEVEL_HIGH, WINHTTP_DISABLE_COOKIES, WINHTTP_FLAG_SECURE,
    WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2, WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_3,
    WINHTTP_OPTION_AUTOLOGON_POLICY, WINHTTP_OPTION_DISABLE_FEATURE,
    WINHTTP_OPTION_REDIRECT_POLICY, WINHTTP_OPTION_REDIRECT_POLICY_NEVER,
    WINHTTP_OPTION_SECURE_PROTOCOLS, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WinHttpSetOption, WinHttpSetTimeouts,
};
use windows_sys::core::BOOL;

use super::{CHUNK_BYTES, FETCH_DEADLINE, HEADERS, TIMEOUTS_MS, USER_AGENT};
use crate::check::CheckError;
use crate::feed::MAX_FEED_BYTES;

/// An open WinHTTP handle (session, connection or request), closed once when dropped.
struct Handle(*mut c_void);

impl Handle {
    /// Wraps the result of a WinHTTP open call; a null handle becomes the thread's last Win32 error.
    fn new(raw: *mut c_void) -> Result<Self, CheckError> {
        if raw.is_null() {
            Err(last_error())
        } else {
            Ok(Self(raw))
        }
    }
}

impl Drop for Handle {
    /// Closes the handle; a close error has no one to report to and is dropped.
    #[allow(unsafe_code, reason = "closing a WinHTTP handle is an FFI call")]
    fn drop(&mut self) {
        // SAFETY: self.0 is a non-null handle from a WinHTTP open call, and only this drop closes it.
        unsafe { WinHttpCloseHandle(self.0) };
    }
}

/// Fetches `https://{host}{path}` and returns the body of a 200 answer, at most `MAX_FEED_BYTES`.
/// Fails with `Network(win32 code)`, `Status(http status)` or `TooLarge`.
pub(super) fn get(host: &str, path: &str) -> Result<Vec<u8>, CheckError> {
    let start = Instant::now();
    // Declared session, connection, request: they drop, and close, in reverse.
    let session = open_session()?;
    let connection = connect(&session, host)?;
    let request = open_request(&connection, path)?;
    send(&request)?;
    let status = status_code(&request)?;
    if status != 200 {
        return Err(CheckError::Status(
            u16::try_from(status).unwrap_or(u16::MAX),
        ));
    }
    read_body(&request, start)
}

/// Reads the calling thread's last Win32 error as a `Network` error.
#[allow(unsafe_code, reason = "GetLastError is an FFI call")]
fn last_error() -> CheckError {
    // SAFETY: GetLastError only reads the calling thread's error slot.
    CheckError::Network(unsafe { GetLastError() })
}

/// Turns a WinHTTP `BOOL` result into `Ok`, or the last Win32 error when it is zero.
fn check(ok: BOOL) -> Result<(), CheckError> {
    if ok == 0 { Err(last_error()) } else { Ok(()) }
}

/// Encodes `text` as NUL-terminated UTF-16, the string form WinHTTP takes.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Sets one `u32` option on an open handle; fails with the Win32 error.
#[allow(unsafe_code, reason = "WinHttpSetOption is an FFI call")]
fn set_u32(handle: &Handle, option: u32, value: u32) -> Result<(), CheckError> {
    // SAFETY: the handle is open; `value` lives through the call and the length says 4 bytes.
    check(unsafe {
        WinHttpSetOption(
            handle.0,
            option,
            (&raw const value).cast(),
            size_of::<u32>() as u32,
        )
    })
}

/// Opens a synchronous session with the system proxy, the four timeouts and TLS 1.2 or 1.3.
#[allow(
    unsafe_code,
    reason = "WinHttpOpen and WinHttpSetTimeouts are FFI calls"
)]
fn open_session() -> Result<Handle, CheckError> {
    let agent = wide(USER_AGENT);
    // SAFETY: `agent` is NUL-terminated and outlives the call; null proxy names go with AUTOMATIC_PROXY.
    let session = Handle::new(unsafe {
        WinHttpOpen(
            agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            null(),
            null(),
            0,
        )
    })?;
    let [resolve, connect, send, receive] = TIMEOUTS_MS;
    // SAFETY: `session` is an open session handle for the whole call.
    check(unsafe { WinHttpSetTimeouts(session.0, resolve, connect, send, receive) })?;
    let modern = WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2 | WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_3;
    // Windows 10 WinHTTP without TLS 1.3 refuses the pair; TLS 1.2 alone is the floor.
    if set_u32(&session, WINHTTP_OPTION_SECURE_PROTOCOLS, modern).is_err() {
        set_u32(
            &session,
            WINHTTP_OPTION_SECURE_PROTOCOLS,
            WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2,
        )?;
    }
    Ok(session)
}

/// Opens a connection to `host` on port 443; nothing is sent yet.
#[allow(unsafe_code, reason = "WinHttpConnect is an FFI call")]
fn connect(session: &Handle, host: &str) -> Result<Handle, CheckError> {
    let server = wide(host);
    // SAFETY: `session` is open and `server` is NUL-terminated; both outlive the call.
    Handle::new(unsafe {
        WinHttpConnect(session.0, server.as_ptr(), INTERNET_DEFAULT_HTTPS_PORT, 0)
    })
}

/// Opens a TLS `GET path` request with redirects, cookies and Windows logon credentials off.
#[allow(unsafe_code, reason = "WinHttpOpenRequest is an FFI call")]
fn open_request(connection: &Handle, path: &str) -> Result<Handle, CheckError> {
    let verb = wide("GET");
    let object = wide(path);
    // SAFETY: `connection` is open; `verb` and `object` are NUL-terminated and outlive the call;
    // null version, referrer and accept types are WinHTTP's documented defaults.
    let request = Handle::new(unsafe {
        WinHttpOpenRequest(
            connection.0,
            verb.as_ptr(),
            object.as_ptr(),
            null(),
            null(),
            null(),
            WINHTTP_FLAG_SECURE,
        )
    })?;
    set_u32(
        &request,
        WINHTTP_OPTION_REDIRECT_POLICY,
        WINHTTP_OPTION_REDIRECT_POLICY_NEVER,
    )?;
    set_u32(
        &request,
        WINHTTP_OPTION_DISABLE_FEATURE,
        WINHTTP_DISABLE_COOKIES,
    )?;
    set_u32(
        &request,
        WINHTTP_OPTION_AUTOLOGON_POLICY,
        WINHTTP_AUTOLOGON_SECURITY_LEVEL_HIGH,
    )?;
    Ok(request)
}

/// Sends the request with the fixed `HEADERS` and no body, then waits for the response headers.
#[allow(
    unsafe_code,
    reason = "WinHttpSendRequest and WinHttpReceiveResponse are FFI calls"
)]
fn send(request: &Handle) -> Result<(), CheckError> {
    let headers: Vec<u16> = HEADERS.encode_utf16().collect();
    let length =
        u32::try_from(headers.len()).map_err(|_| CheckError::Network(ERROR_INVALID_PARAMETER))?;
    // SAFETY: `request` is open; `headers` holds `length` UTF-16 units and outlives the call; no body.
    check(unsafe { WinHttpSendRequest(request.0, headers.as_ptr(), length, null(), 0, 0, 0) })?;
    // SAFETY: `request` is open and sent; the reserved pointer must be null.
    check(unsafe { WinHttpReceiveResponse(request.0, null_mut()) })
}

/// Reads the HTTP status code of a received response.
#[allow(unsafe_code, reason = "WinHttpQueryHeaders is an FFI call")]
fn status_code(request: &Handle) -> Result<u32, CheckError> {
    let mut code = 0_u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: `request` has a response; `code` and `size` are live locals and `size` is code's
    // byte length; a null name and index ask for the status line by number.
    check(unsafe {
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            null(),
            (&raw mut code).cast(),
            &raw mut size,
            null_mut(),
        )
    })?;
    Ok(code)
}

/// Reads the body in chunks of at most `CHUNK_BYTES`; fails with `TooLarge` before passing `MAX_FEED_BYTES`,
/// or with `Network(ERROR_TIMEOUT)` once `FETCH_DEADLINE` has passed since `start`.
#[allow(
    unsafe_code,
    reason = "WinHttpQueryDataAvailable and WinHttpReadData are FFI calls"
)]
fn read_body(request: &Handle, start: Instant) -> Result<Vec<u8>, CheckError> {
    let mut body = Vec::new();
    loop {
        if start.elapsed() >= FETCH_DEADLINE {
            return Err(CheckError::Network(ERROR_TIMEOUT));
        }
        let mut available = 0_u32;
        // SAFETY: `request` has a response and `available` is a live local.
        check(unsafe { WinHttpQueryDataAvailable(request.0, &raw mut available) })?;
        let want = usize::try_from(available).map_or(CHUNK_BYTES, |n| n.min(CHUNK_BYTES));
        if want == 0 {
            return Ok(body);
        }
        let start = body.len();
        if start.saturating_add(want) > MAX_FEED_BYTES {
            return Err(CheckError::TooLarge);
        }
        body.resize(start + want, 0);
        let mut read = 0_u32;
        // SAFETY: `body[start..]` is `want` initialised bytes (want <= CHUNK_BYTES fits u32),
        // borrowed mutably for the call only; `read` is a live local.
        check(unsafe {
            WinHttpReadData(
                request.0,
                body[start..].as_mut_ptr().cast(),
                want as u32,
                &raw mut read,
            )
        })?;
        let got = usize::try_from(read).map_or(0, |n| n.min(want));
        body.truncate(start + got);
        if got == 0 {
            return Ok(body);
        }
    }
}
