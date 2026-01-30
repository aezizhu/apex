"""Custom exception classes for the Apex SDK."""

from typing import Any


class ApexError(Exception):
    """Base exception for all Apex SDK errors."""

    def __init__(self, message: str, details: dict[str, Any] | None = None) -> None:
        super().__init__(message)
        self.message = message
        self.details = details or {}


class ApexAPIError(ApexError):
    """Exception raised when the API returns an error response."""

    def __init__(
        self,
        message: str,
        status_code: int,
        response_body: dict[str, Any] | None = None,
    ) -> None:
        super().__init__(message, details={"status_code": status_code, "response": response_body})
        self.status_code = status_code
        self.response_body = response_body


class ApexAuthenticationError(ApexAPIError):
    """Exception raised when authentication fails (401)."""

    def __init__(self, message: str = "Authentication failed", response_body: dict[str, Any] | None = None) -> None:
        super().__init__(message, status_code=401, response_body=response_body)


class ApexAuthorizationError(ApexAPIError):
    """Exception raised when authorization fails (403)."""

    def __init__(self, message: str = "Authorization failed", response_body: dict[str, Any] | None = None) -> None:
        super().__init__(message, status_code=403, response_body=response_body)


class ApexNotFoundError(ApexAPIError):
    """Exception raised when a resource is not found (404)."""

    def __init__(self, message: str = "Resource not found", response_body: dict[str, Any] | None = None) -> None:
        super().__init__(message, status_code=404, response_body=response_body)


class ApexValidationError(ApexAPIError):
    """Exception raised when request validation fails (422)."""

    def __init__(self, message: str = "Validation failed", response_body: dict[str, Any] | None = None) -> None:
        super().__init__(message, status_code=422, response_body=response_body)


class ApexRateLimitError(ApexAPIError):
    """Exception raised when rate limit is exceeded (429)."""

    def __init__(
        self,
        message: str = "Rate limit exceeded",
        response_body: dict[str, Any] | None = None,
        retry_after: int | None = None,
    ) -> None:
        super().__init__(message, status_code=429, response_body=response_body)
        self.retry_after = retry_after


class ApexServerError(ApexAPIError):
    """Exception raised when the server returns a 5xx error."""

    def __init__(
        self,
        message: str = "Server error",
        status_code: int = 500,
        response_body: dict[str, Any] | None = None,
    ) -> None:
        super().__init__(message, status_code=status_code, response_body=response_body)


class ApexConnectionError(ApexError):
    """Exception raised when a connection error occurs."""

    pass


class ApexTimeoutError(ApexError):
    """Exception raised when a request times out."""

    pass


class ApexWebSocketError(ApexError):
    """Exception raised when a WebSocket error occurs."""

    pass


class ApexWebSocketClosed(ApexWebSocketError):
    """Exception raised when the WebSocket connection is closed unexpectedly."""

    def __init__(self, message: str = "WebSocket connection closed", code: int | None = None) -> None:
        super().__init__(message, details={"close_code": code})
        self.code = code
